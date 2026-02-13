// Copyright 2022 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Types implementing the [flyweight pattern](https://en.wikipedia.org/wiki/Flyweight_pattern)
//! for reusing string allocations.
//!
//! # Features
//!
//! * Supports UTF-8 strings with [`FlyStr`] and UTF-8-ish bytestrings with [`FlyByteStr`].
//! * Easy drop-in replacement for immutable strings without wiring up any additional function args.
//! * Accessing the underlying string values has overhead similar to a `Box<str>`.
//! * Cheap to clone.
//! * Strings in the cache are freed when the last reference to them is dropped.
//! * Heap allocations are avoided when possible with small string optimizations (SSO).
//! * Hashing a flyweight and comparing it for equality against another flyweight are both O(1)
//!   operations.
//!
//! # Tradeoffs
//!
//! This was originally written for [Fuchsia](https://fuchsia.dev) at a time when popular options
//! didn't fit the needs of adding caching to an existing long-running multithreaded system service
//! that holds an unbounded number of user-controlled strings. The above features are suited to this
//! use case, but there are many (many) alternative [string caching crates] to choose from if you
//! have different constraints.
//!
//! [string caching crates]: https://crates.io/search?q=intern

#![warn(missing_docs, clippy::all)]

mod flybytestr;
mod flycstr;
mod flystr;
mod raw;

pub use flybytestr::FlyByteStr;
pub use flycstr::FlyCStr;
pub use flystr::FlyStr;

use foldhash::fast::RandomState;
use hashbrown::{hash_table::Entry, hash_table::VacantEntry, HashTable};
use std::borrow::Borrow;
use std::hash::{BuildHasher as _, Hash, Hasher};
use std::ptr::NonNull;
use std::sync::Mutex;

/// The global string cache for `FlyStr`.
///
/// If a live `FlyStr` contains an `Storage`, the `Storage` must also be in this cache and it must
/// have a refcount of >= 2.
pub(crate) static CACHE: std::sync::LazyLock<Cache> = std::sync::LazyLock::new(Cache::default);

#[derive(Default)]
pub(crate) struct Cache {
    pub(crate) hasher: RandomState,
    pub(crate) table: Mutex<HashTable<Storage>>,
}

/// Wrapper type for stored strings that lets us query the cache without an owned value.
#[repr(transparent)]
pub(crate) struct Storage(NonNull<raw::Payload>);

// SAFETY: FlyStr storage is always safe to send across threads.
unsafe impl Send for Storage {}
// SAFETY: FlyStr storage is always safe to share across threads.
unsafe impl Sync for Storage {}

impl PartialEq for Storage {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl Eq for Storage {}

impl Hash for Storage {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_bytes().hash(state)
    }
}

impl Storage {
    #[inline]
    fn inc_ref(&self) -> usize {
        // SAFETY: `Storage` always points to a valid `Payload`.
        unsafe { raw::Payload::inc_ref(self.0.as_ptr()) }
    }

    #[inline]
    fn as_bytes(&self) -> &[u8] {
        // SAFETY: `Storage` always points to a valid `Payload`.
        unsafe { &*raw::Payload::bytes(self.0.as_ptr()) }
    }
}

impl Borrow<[u8]> for Storage {
    #[inline]
    fn borrow(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[repr(C)] // Guarantee predictable field ordering.
pub(crate) union RawRepr {
    /// Strings longer than MAX_INLINE_SIZE are allocated using `raw::Payload`, which have a thin
    /// pointer representation. This means that `heap` variants of `Storage` will always have the
    /// pointer contents aligned and this variant will never have its least significant bit set.
    ///
    /// We store a `NonNull` so we can have guaranteed pointer layout.
    heap: NonNull<raw::Payload>,

    /// Strings shorter than or equal in length to MAX_INLINE_SIZE are stored in this union variant.
    /// The first byte is reserved for the size of the inline string, and the remaining bytes are
    /// used for the string itself. The first byte has its least significant bit set to 1 to
    /// distinguish inline strings from heap-allocated ones, and the size is stored in the remaining
    /// 7 bits.
    inline: InlineRepr,
}

// The inline variant should not cause us to occupy more space than the heap variant alone.
static_assertions::assert_eq_size!(NonNull<raw::Payload>, RawRepr);

// Alignment of the payload pointers must be >1 in order to have space for the mask bit at the
// bottom.
static_assertions::const_assert!(std::mem::align_of::<raw::Payload>() > 1);

// The short string optimization makes little-endian layout assumptions with the first byte being
// the least significant.
static_assertions::assert_type_eq_all!(byteorder::NativeEndian, byteorder::LittleEndian);

/// An enum with an actual discriminant that allows us to limit the reach of unsafe code in the
/// implementation without affecting the stored size of `RawRepr`.
pub(crate) enum SafeRepr<'a> {
    Heap(NonNull<raw::Payload>),
    Inline(&'a InlineRepr),
}

// SAFETY: FlyStr can be dropped from any thread.
unsafe impl Send for RawRepr {}
// SAFETY: FlyStr has an immutable public API.
unsafe impl Sync for RawRepr {}

impl RawRepr {
    #[inline]
    pub(crate) fn new(s: &[u8]) -> Self {
        if s.len() <= MAX_INLINE_SIZE {
            RawRepr::new_inline(s)
        } else {
            let cache = &*CACHE;
            let mut table = cache.table.lock().unwrap();

            match table.entry(
                cache.hasher.hash_one(s),
                |storage: &Storage| s == storage.as_bytes(),
                |storage: &Storage| cache.hasher.hash_one(storage.as_bytes()),
            ) {
                Entry::Occupied(entry) => RawRepr::from_storage(entry.get()),
                Entry::Vacant(entry) => RawRepr::new_for_storage(entry, s),
            }
        }
    }

    #[inline]
    fn new_inline(s: &[u8]) -> Self {
        assert!(s.len() <= MAX_INLINE_SIZE);
        let new = Self {
            inline: InlineRepr::new(s),
        };
        assert!(
            new.is_inline(),
            "least significant bit must be 1 for inline strings"
        );
        new
    }

    #[inline]
    fn from_storage(storage: &Storage) -> Self {
        if storage.inc_ref() == 0 {
            // Another thread is trying to lock the cache and free this string. They already
            // released their refcount, so give it back to them. This will prevent this thread
            // and other threads from attempting to free the string if they drop the refcount back
            // down.
            storage.inc_ref();
        }
        Self { heap: storage.0 }
    }

    #[inline]
    fn new_for_storage(entry: VacantEntry<'_, Storage>, bytes: &[u8]) -> Self {
        assert!(bytes.len() > MAX_INLINE_SIZE);
        // `Payload::alloc` starts the refcount at 1.
        let new_storage = raw::Payload::alloc(bytes);

        let for_cache = Storage(new_storage);
        let new = Self { heap: new_storage };
        assert!(
            !new.is_inline(),
            "least significant bit must be 0 for heap strings"
        );
        entry.insert(for_cache);
        new
    }

    #[inline]
    fn is_inline(&self) -> bool {
        // SAFETY: it is always OK to interpret a pointer as byte array as long as we don't expect
        // to retain provenance.
        (unsafe { self.inline.masked_len } & 1) == 1
    }

    #[inline]
    pub(crate) fn project(&self) -> SafeRepr<'_> {
        if self.is_inline() {
            // SAFETY: Just checked that this is the inline variant.
            SafeRepr::Inline(unsafe { &self.inline })
        } else {
            // SAFETY: Just checked that this is the heap variant.
            SafeRepr::Heap(unsafe { self.heap })
        }
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        match self.project() {
            // SAFETY: FlyStr owns the payload stored as a NonNull, it is live as long as `FlyStr`.
            SafeRepr::Heap(ptr) => unsafe { &*raw::Payload::bytes(ptr.as_ptr()) },
            SafeRepr::Inline(i) => i.as_bytes(),
        }
    }

    #[cfg(test)]
    pub(crate) fn refcount(&self) -> Option<usize> {
        match self.project() {
            SafeRepr::Heap(ptr) => {
                // SAFETY: The payload is live as long as the repr is live.
                let count = unsafe { raw::Payload::refcount(ptr.as_ptr()) };
                Some(count)
            }
            SafeRepr::Inline(_) => None,
        }
    }
}

impl PartialEq for RawRepr {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // SAFETY: it is always OK to interpret a pointer as a byte array as long as we don't expect
        // to retain provenance.
        let lhs = unsafe { &self.inline };
        // SAFETY: it is always OK to interpret a pointer as a byte array as long as we don't expect
        // to retain provenance.
        let rhs = unsafe { &other.inline };
        lhs.eq(rhs)
    }
}
impl Eq for RawRepr {}

impl Hash for RawRepr {
    fn hash<H: Hasher>(&self, h: &mut H) {
        // SAFETY: it is always OK to interpret a pointer as a byte array as long as we don't expect
        // to retain provenance.
        let this = unsafe { &self.inline };
        this.hash(h);
    }
}

impl Clone for RawRepr {
    fn clone(&self) -> Self {
        match self.project() {
            SafeRepr::Heap(ptr) => {
                // SAFETY: We own this payload, we know it's live because we are.
                unsafe {
                    raw::Payload::inc_ref(ptr.as_ptr());
                }
                Self { heap: ptr }
            }
            SafeRepr::Inline(&inline) => Self { inline },
        }
    }
}

impl Drop for RawRepr {
    fn drop(&mut self) {
        if !self.is_inline() {
            // SAFETY: We checked above that this is the heap repr.
            let heap = unsafe { self.heap };

            // Decrementing the refcount before locking the cache causes the following failure mode:
            //
            // 1. We drop the refcount to 0.
            // 2. Another thread finds the string we're about to drop and increments the refcount
            //    back up to 1.
            // 3. That thread drops its refcount, and also sees the refcount drop to 0.
            // 4. That thread locks the cache, removes the value, and drops the payload. This leaves
            //    us with a dangling pointer to the dropped payload.
            // 5. We lock the cache and go to look up our value in the cache. Our payload pointer is
            //    dangling now, but we don't know that. If we try to read through our dangling
            //    pointer, we cause UB.
            //
            // To account for this failure mode and still optimistically drop our refcount, we
            // modify the procedure slightly:
            //
            // 1. We drop the refcount to 0.
            // 2. Another thread finds the string we're about to drop and increments the refcount
            //    back up to 1. It notices that the refcount incremented from 0 to 1, and so knows
            //    that our thread will try to drop it. While still holding the cache lock, that
            //    thread increments the refcount again from 1 to 2. This "gives back" the refcount
            //    to our thread.
            // 3. That thread drops its refcount, and sees the refcount drop to 1. It won't try to
            //    drop the payload this time.
            // 4. We lock the cache, and decrement the refcount a second time. If it decremented
            //    from 0 or 1, then we know that no other threads are currently holding references
            //    to it and we can safely drop it ourselves.

            // SAFETY: The payload is live.
            let prev_refcount = unsafe { raw::Payload::dec_ref(heap.as_ptr()) };

            // If we held the final refcount outside of the cache, try to remove the string.
            if prev_refcount == 1 {
                let cache = &*CACHE;
                let mut table = cache.table.lock().unwrap();

                let current_refcount = unsafe { raw::Payload::dec_ref(heap.as_ptr()) };
                if current_refcount <= 1 {
                    // If the refcount was still 0 after acquiring the cache lock, no other thread
                    // looked up this payload between optimistically decrementing the refcount and
                    // now. If the refcount was 1, then another thread did, but dropped its refcount
                    // before we got the cache lock. Either way, we can safely remove the string
                    // from the cache and free it.

                    let bytes = unsafe { &*raw::Payload::bytes(heap.as_ptr()) };

                    if let Ok(entry) = table
                        .find_entry(cache.hasher.hash_one(bytes), |storage: &Storage| {
                            self.as_bytes() == storage.as_bytes()
                        })
                    {
                        entry.remove();
                    } else {
                        panic!(
                            "cache did not contain bytes, but this thread didn't remove them yet"
                        )
                    }

                    // Get out of the critical section as soon as possible
                    drop(table);

                    // SAFETY: The payload is live.
                    unsafe { raw::Payload::dealloc(heap.as_ptr()) };
                } else {
                    // Another thread looked up this payload, made a reference to it, and gave our
                    // refcount back to us for a minmium refcount of 2. We re-removed our refcount,
                    // giving a minimum of one. This means it's no longer our responsibility to
                    // deallocate the string, so we ended up needlessly locking the cache.
                }
            }
        }
    }
}

#[derive(Clone, Copy, Hash, PartialEq)]
#[repr(C)] // Preserve field ordering.
pub(crate) struct InlineRepr {
    /// The first byte, which corresponds to the LSB of a pointer in the other variant.
    ///
    /// When the first bit is `1` the rest of this byte stores the length of the inline string.
    masked_len: u8,
    /// Inline string contents.
    contents: [u8; MAX_INLINE_SIZE],
}

/// We can store small strings up to 1 byte less than the size of the pointer to the heap-allocated
/// string.
pub(crate) const MAX_INLINE_SIZE: usize = std::mem::size_of::<NonNull<raw::Payload>>() - 1;

// Guard rail to make sure we never end up with an incorrect inline size encoding. Ensure that
// MAX_INLINE_SIZE is always smaller than the maximum size we can represent in a byte with the LSB
// reserved.
static_assertions::const_assert!((u8::MAX >> 1) as usize >= MAX_INLINE_SIZE);

impl InlineRepr {
    #[inline]
    pub(crate) fn new(s: &[u8]) -> Self {
        assert!(s.len() <= MAX_INLINE_SIZE);

        // Set the first byte to the length of the inline string with LSB masked to 1.
        let masked_len = ((s.len() as u8) << 1) | 1;

        let mut contents = [0u8; MAX_INLINE_SIZE];
        contents[..s.len()].copy_from_slice(s);

        Self {
            masked_len,
            contents,
        }
    }

    #[inline]
    pub(crate) fn as_bytes(&self) -> &[u8] {
        let len = self.masked_len >> 1;
        &self.contents[..len as usize]
    }
}

#[cfg(test)]
pub(crate) mod test_utils {
    use super::*;

    pub(crate) fn reset_global_cache() {
        // We still want subsequent tests to be able to run if one in the same process panics.
        match CACHE.table.lock() {
            Ok(mut c) => *c = HashTable::new(),
            Err(e) => *e.into_inner() = HashTable::new(),
        }
    }
    pub(crate) fn num_strings_in_global_cache() -> usize {
        CACHE.table.lock().unwrap().len()
    }
}

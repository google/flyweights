// Copyright 2022 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::RawRepr;
use std::borrow::Borrow;
use std::ffi::{CStr, CString};
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::hash::Hash;
use std::ops::Deref;

#[cfg(feature = "serde")]
use serde::de::{Deserializer, Visitor};
#[cfg(feature = "serde")]
use serde::ser::Serializer;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// An immutable C-string type which only stores a single copy of each string allocated.
/// Internally represented as a shared pointer to the backing allocation. Occupies a single pointer width.
///
/// # Small strings
///
/// Very short strings are stored inline in the pointer with bit-tagging, so no allocations are
/// performed.
///
/// # Performance
///
/// It's slower to construct than a regular `CString` but trades that for reduced standing memory
/// usage by deduplicating strings. `PartialEq` and `Hash` are implemented on the underlying pointer
/// value rather than the pointed-to data for faster equality comparisons and indexing, which is
/// sound by virtue of the type guaranteeing that only one `FlyCStr` pointer value will exist at
/// any time for a given string's contents.
///
/// As with any performance optimization, you should only use this type if you can measure the
/// benefit it provides to your program. Pay careful attention to creating `FlyCStr`s in hot paths
/// as it may regress runtime performance.
///
/// # Allocation lifecycle
///
/// Intended for long-running system services with user-provided values, `FlyCStr`s are removed from
/// the global cache when the last reference to them is dropped. While this incurs some overhead
/// it is important to prevent the value cache from becoming a denial-of-service vector.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct FlyCStr(pub(crate) RawRepr);

static_assertions::assert_eq_size!(FlyCStr, usize);

// `Borrow` requires that:
//
// > Eq, Ord and Hash must be equivalent for borrowed and owned values: x.borrow() == y.borrow()
// > should give the same result as x == y.
//
// The current implementation of `FlyCStr` cannot satisfy `Borrow<CStr>` because the O(1) hashing
// feature means that `s.hash() != s.borrow().hash()`.
static_assertions::assert_not_impl_any!(FlyCStr: Borrow<CStr>);

impl FlyCStr {
    /// Create a `FlyCStr`, allocating it in the cache if the value is not already cached.
    ///
    /// # Performance
    ///
    /// Creating an instance of this type for strings longer than the maximum inline size requires
    /// accessing the global cache of strings, which involves taking a lock. When multiple threads
    /// are allocating lots of strings there may be contention. Each string allocated is hashed for
    /// lookup in the cache.
    #[inline]
    pub fn new(s: impl AsRef<CStr>) -> Self {
        Self(RawRepr::new(s.as_ref().to_bytes_with_nul()))
    }

    /// Returns the underlying C-string slice.
    #[inline]
    pub fn as_c_str(&self) -> &CStr {
        // SAFETY: Every FlyCStr is constructed from a valid CStr (bytes with nul).
        unsafe { CStr::from_bytes_with_nul_unchecked(self.0.as_bytes()) }
    }
}

impl Default for FlyCStr {
    #[inline]
    fn default() -> Self {
        Self::new(c"")
    }
}

impl From<&'_ CStr> for FlyCStr {
    #[inline]
    fn from(s: &CStr) -> Self {
        Self::new(s)
    }
}

impl From<CString> for FlyCStr {
    #[inline]
    fn from(s: CString) -> Self {
        Self::new(s)
    }
}

impl From<&'_ CString> for FlyCStr {
    #[inline]
    fn from(s: &CString) -> Self {
        Self::new(s)
    }
}

impl From<Box<CStr>> for FlyCStr {
    #[inline]
    fn from(s: Box<CStr>) -> Self {
        Self::new(s)
    }
}

impl From<&Box<CStr>> for FlyCStr {
    #[inline]
    fn from(s: &Box<CStr>) -> Self {
        Self::new(&**s)
    }
}

impl From<FlyCStr> for CString {
    #[inline]
    fn from(s: FlyCStr) -> CString {
        s.as_c_str().to_owned()
    }
}

impl From<&'_ FlyCStr> for CString {
    #[inline]
    fn from(s: &FlyCStr) -> CString {
        s.as_c_str().to_owned()
    }
}

impl Deref for FlyCStr {
    type Target = CStr;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_c_str()
    }
}

impl AsRef<CStr> for FlyCStr {
    #[inline]
    fn as_ref(&self) -> &CStr {
        self.as_c_str()
    }
}

impl PartialOrd for FlyCStr {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FlyCStr {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_c_str().cmp(other.as_c_str())
    }
}

impl PartialEq<CStr> for FlyCStr {
    #[inline]
    fn eq(&self, other: &CStr) -> bool {
        self.as_c_str() == other
    }
}

impl PartialEq<&'_ CStr> for FlyCStr {
    #[inline]
    fn eq(&self, other: &&CStr) -> bool {
        self.as_c_str() == *other
    }
}

impl PartialEq<CString> for FlyCStr {
    #[inline]
    fn eq(&self, other: &CString) -> bool {
        self.as_c_str() == other.as_c_str()
    }
}

impl PartialOrd<CStr> for FlyCStr {
    #[inline]
    fn partial_cmp(&self, other: &CStr) -> Option<std::cmp::Ordering> {
        self.as_c_str().partial_cmp(other)
    }
}

impl PartialOrd<&CStr> for FlyCStr {
    #[inline]
    fn partial_cmp(&self, other: &&CStr) -> Option<std::cmp::Ordering> {
        self.as_c_str().partial_cmp(*other)
    }
}

impl PartialOrd<CString> for FlyCStr {
    #[inline]
    fn partial_cmp(&self, other: &CString) -> Option<std::cmp::Ordering> {
        self.as_c_str().partial_cmp(other.as_c_str())
    }
}

impl Debug for FlyCStr {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self.as_c_str(), f)
    }
}

#[cfg(feature = "serde")]
impl Serialize for FlyCStr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Serialize as bytes since C-string might not be valid UTF-8. We're copying serde which
        // serializes `CStr` without the trailing nul.
        serializer.serialize_bytes(self.to_bytes())
    }
}

#[cfg(feature = "serde")]
impl<'d> Deserialize<'d> for FlyCStr {
    fn deserialize<D: Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_bytes(FlyCStrVisitor)
    }
}

#[cfg(feature = "serde")]
struct FlyCStrVisitor;

#[cfg(feature = "serde")]
impl<'de> Visitor<'de> for FlyCStrVisitor {
    type Value = FlyCStr;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str("a byte array containing a C-string")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let capacity = seq.size_hint().unwrap_or(0);
        let mut bytes = Vec::with_capacity(capacity);

        while let Some(b) = seq.next_element()? {
            bytes.push(b);
        }

        CString::new(bytes)
            .map(FlyCStr::new)
            .map_err(serde::de::Error::custom)
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        CString::new(v)
            .map(FlyCStr::new)
            .map_err(serde::de::Error::custom)
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        CString::new(v)
            .map(FlyCStr::new)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use crate::{FlyByteStr, MAX_INLINE_SIZE};
    use static_assertions::{const_assert, const_assert_eq};
    use std::collections::{BTreeSet, HashSet};
    use test_case::test_case;

    // These tests all manipulate the process-global cache in the parent module. On target devices
    // we run each test case in its own process, so the test cases can't pollute each other. On
    // host, we run tests with a process for each suite (which is the Rust upstream default), and
    // we need to manually isolate the tests from each other.
    #[cfg(not(target_os = "fuchsia"))]
    use serial_test::serial;

    // `test-case` doesn't support the `c"..."` syntax, so use raw byte strings instead.
    const SHORT_CSTRING: &[u8] = b"hello\0";
    const_assert!(SHORT_CSTRING.len() < MAX_INLINE_SIZE);

    const MAX_LEN_SHORT_CSTRING: &[u8] = b"hello!\0";
    const_assert_eq!(MAX_LEN_SHORT_CSTRING.len(), MAX_INLINE_SIZE);

    const MIN_LEN_LONG_CSTRING: &[u8] = b"hello!!!\0";
    // const_assert_eq!(MIN_LEN_LONG_CSTRING.len(), MAX_INLINE_SIZE + 1);

    const LONG_CSTRING: &[u8] = b"hello, world!!!!!!!!!!!!!!!!!!!!\0";
    const_assert!(LONG_CSTRING.len() > MAX_INLINE_SIZE);

    fn cstr(bytes: &[u8]) -> &CStr {
        CStr::from_bytes_with_nul(bytes).unwrap()
    }

    #[test_case(b"\0" ; "empty cstring")]
    #[test_case(SHORT_CSTRING ; "short cstring")]
    #[test_case(MAX_LEN_SHORT_CSTRING ; "max len short cstrings")]
    #[test_case(MIN_LEN_LONG_CSTRING ; "barely long cstrings")]
    #[test_case(LONG_CSTRING ; "long cstrings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn cstring_formatting_is_equivalent_to_cstr(original: &[u8]) {
        reset_global_cache();

        let original = cstr(original);
        let cached = FlyCStr::new(original);
        assert_eq!(format!("{original:?}"), format!("{cached:?}"));
    }

    #[test_case(b"\0" ; "empty cstring")]
    #[test_case(SHORT_CSTRING ; "short cstring")]
    #[test_case(MAX_LEN_SHORT_CSTRING ; "max len short cstrings")]
    #[test_case(MIN_LEN_LONG_CSTRING ; "barely long cstrings")]
    #[test_case(LONG_CSTRING ; "long cstrings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn cstring_equality_works(contents: &[u8]) {
        reset_global_cache();

        let contents = cstr(contents);
        let cached = FlyCStr::new(contents);
        assert_eq!(cached, cached.clone(), "must be equal to itself");
        assert_eq!(cached, contents, "must be equal to the original");
        assert_eq!(
            cached,
            contents.to_owned(),
            "must be equal to an owned copy of the original"
        );

        // test inequality too
        assert_ne!(cached, c"goodbye");
    }

    #[test_case(b"\0", SHORT_CSTRING ; "empty and short cstring")]
    #[test_case(SHORT_CSTRING, MAX_LEN_SHORT_CSTRING ; "two short cstrings")]
    #[test_case(MAX_LEN_SHORT_CSTRING, MIN_LEN_LONG_CSTRING ; "short and long cstrings")]
    #[test_case(MIN_LEN_LONG_CSTRING, LONG_CSTRING ; "barely long and long cstrings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn cstring_comparison_works(lesser_contents: &[u8], greater_contents: &[u8]) {
        reset_global_cache();

        let lesser = FlyCStr::new(cstr(lesser_contents));
        let greater = FlyCStr::new(cstr(greater_contents));

        // lesser as method receiver
        assert!(lesser < greater);
        assert!(lesser <= greater);

        // greater as method receiver
        assert!(greater > lesser);
        assert!(greater >= lesser);
    }

    #[test_case(b"\0" ; "empty cstring")]
    #[test_case(SHORT_CSTRING ; "short cstrings")]
    #[test_case(MAX_LEN_SHORT_CSTRING ; "max len short cstrings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn no_allocations_for_short_cstrings(contents: &[u8]) {
        reset_global_cache();
        assert_eq!(num_strings_in_global_cache(), 0);

        let contents = cstr(contents);
        let original = FlyCStr::new(contents);
        assert_eq!(num_strings_in_global_cache(), 0);
        assert_eq!(original.0.refcount(), None);

        let cloned = original.clone();
        assert_eq!(num_strings_in_global_cache(), 0);
        assert_eq!(cloned.0.refcount(), None);

        let deduped = FlyCStr::new(contents);
        assert_eq!(num_strings_in_global_cache(), 0);
        assert_eq!(deduped.0.refcount(), None);
    }

    #[test_case(MIN_LEN_LONG_CSTRING ; "barely long cstrings")]
    #[test_case(LONG_CSTRING ; "long cstrings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn only_one_copy_allocated_for_long_cstrings(contents: &[u8]) {
        reset_global_cache();

        assert_eq!(num_strings_in_global_cache(), 0);

        let contents = cstr(contents);
        let original = FlyCStr::new(contents);
        assert_eq!(
            num_strings_in_global_cache(),
            1,
            "only one string allocated"
        );
        assert_eq!(original.0.refcount(), Some(1), "one copy on stack");

        let cloned = original.clone();
        assert_eq!(
            num_strings_in_global_cache(),
            1,
            "cloning just incremented refcount"
        );
        assert_eq!(cloned.0.refcount(), Some(2), "two copies on stack");

        let deduped = FlyCStr::new(contents);
        assert_eq!(num_strings_in_global_cache(), 1, "new string was deduped");
        assert_eq!(deduped.0.refcount(), Some(3), "three copies on stack");
    }

    #[test]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn cstring_and_bytestrings_share_the_cache() {
        reset_global_cache();

        assert_eq!(num_strings_in_global_cache(), 0, "cache is empty");

        let _cstr = FlyCStr::new(cstr(MIN_LEN_LONG_CSTRING));
        assert_eq!(num_strings_in_global_cache(), 1, "string was allocated");

        let _bytes = FlyByteStr::from(MIN_LEN_LONG_CSTRING);
        assert_eq!(
            num_strings_in_global_cache(),
            1,
            "bytestring was pulled from cache"
        );
    }

    #[test_case(MIN_LEN_LONG_CSTRING ; "barely long cstrings")]
    #[test_case(LONG_CSTRING ; "long cstrings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn cached_cstrings_dropped_when_refs_dropped(contents: &[u8]) {
        reset_global_cache();

        let contents = cstr(contents);
        let alloced = FlyCStr::new(contents);
        assert_eq!(
            num_strings_in_global_cache(),
            1,
            "only one string allocated"
        );
        drop(alloced);
        assert_eq!(num_strings_in_global_cache(), 0, "last reference dropped");
    }

    #[test_case(b"\0", SHORT_CSTRING ; "empty and short cstring")]
    #[test_case(SHORT_CSTRING, MAX_LEN_SHORT_CSTRING ; "two short cstrings")]
    #[test_case(SHORT_CSTRING, LONG_CSTRING ; "short and long cstrings")]
    #[test_case(LONG_CSTRING, MAX_LEN_SHORT_CSTRING ; "long and max-len-short cstrings")]
    #[test_case(MIN_LEN_LONG_CSTRING, LONG_CSTRING ; "barely long and long cstrings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn equality_and_hashing_works(first: &[u8], second: &[u8]) {
        reset_global_cache();

        let first = FlyCStr::new(cstr(first));
        let second = FlyCStr::new(cstr(second));

        let mut set = HashSet::new();
        set.insert(first.clone());
        assert!(set.contains(&first));
        assert!(!set.contains(&second));

        // re-insert the same string
        set.insert(first);
        assert_eq!(
            set.len(),
            1,
            "set did not grow because the same string was inserted as before"
        );

        set.insert(second.clone());
        assert_eq!(
            set.len(),
            2,
            "inserting a different string must mutate the set"
        );
        assert!(set.contains(&second));

        // re-insert the second string
        set.insert(second);
        assert_eq!(set.len(), 2);
    }

    #[test_case(b"\0", SHORT_CSTRING ; "empty and short cstring")]
    #[test_case(SHORT_CSTRING, MAX_LEN_SHORT_CSTRING ; "two short cstrings")]
    #[test_case(SHORT_CSTRING, LONG_CSTRING ; "short and long cstrings")]
    #[test_case(LONG_CSTRING, MAX_LEN_SHORT_CSTRING ; "long and max-len-short cstrings")]
    #[test_case(MIN_LEN_LONG_CSTRING, LONG_CSTRING ; "barely long and long cstrings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn comparison_for_btree_storage_works(first: &[u8], second: &[u8]) {
        reset_global_cache();

        let first = FlyCStr::new(cstr(first));
        let second = FlyCStr::new(cstr(second));

        let mut set = BTreeSet::new();
        set.insert(first.clone());
        assert!(set.contains(&first));
        assert!(!set.contains(&second));

        // re-insert the same string
        set.insert(first);
        assert_eq!(
            set.len(),
            1,
            "set did not grow because the same string was inserted as before"
        );

        set.insert(second.clone());
        assert_eq!(
            set.len(),
            2,
            "inserting a different string must mutate the set"
        );
        assert!(set.contains(&second));

        // re-insert the second string
        set.insert(second);
        assert_eq!(set.len(), 2);
    }

    #[cfg(feature = "serde")]
    #[test_case(b"\0" ; "empty cstring")]
    #[test_case(SHORT_CSTRING ; "short cstrings")]
    #[test_case(MAX_LEN_SHORT_CSTRING ; "max len short cstrings")]
    #[test_case(MIN_LEN_LONG_CSTRING ; "min len long cstrings")]
    #[test_case(LONG_CSTRING ; "long cstrings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn serde_works(contents: &[u8]) {
        reset_global_cache();

        let contents = cstr(contents);
        let s = FlyCStr::new(contents);
        let as_json = serde_json::to_string(&s).unwrap();
        let expected_json = serde_json::to_string(contents.to_bytes()).unwrap();
        assert_eq!(as_json, expected_json);

        let deserialized: FlyCStr = serde_json::from_str(&as_json).unwrap();
        assert_eq!(s, deserialized);
    }
}

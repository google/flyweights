// Copyright 2022 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::{FlyStr, RawRepr};
use bstr::{BStr, BString};
use std::borrow::Borrow;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::hash::Hash;
use std::ops::Deref;

#[cfg(feature = "serde")]
use serde::de::{Deserializer, Visitor};
#[cfg(feature = "serde")]
use serde::ser::Serializer;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// An immutable bytestring type which only stores a single copy of each string allocated.
/// Internally represented as a shared pointer to the backing allocation. Occupies a single pointer width.
///
/// # Small strings
///
/// Very short strings are stored inline in the pointer with bit-tagging, so no allocations are
/// performed.
///
/// # Performance
///
/// It's slower to construct than a regular `BString` but trades that for reduced standing memory
/// usage by deduplicating strings. `PartialEq` and `Hash` are implemented on the underlying pointer
/// value rather than the pointed-to data for faster equality comparisons and indexing, which is
/// sound by virtue of the type guaranteeing that only one `FlyByteStr` pointer value will exist at
/// any time for a given string's contents.
///
/// As with any performance optimization, you should only use this type if you can measure the
/// benefit it provides to your program. Pay careful attention to creating `FlyByteStr`s in hot
/// paths as it may regress runtime performance.
///
/// # Allocation lifecycle
///
/// Intended for long-running system services with user-provided values, `FlyByteStr`s are removed
/// from the global cache when the last reference to them is dropped. While this incurs some
/// overhead it is important to prevent the value cache from becoming a denial-of-service vector.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct FlyByteStr(pub(crate) RawRepr);

static_assertions::assert_eq_size!(FlyByteStr, usize);

// `Borrow` requires that:
//
// > Eq, Ord and Hash must be equivalent for borrowed and owned values: x.borrow() == y.borrow()
// > should give the same result as x == y.
//
// The current implementation of `FlyByteStr` cannot satisfy `Borrow<str>` because the O(1) hashing
// feature means that `s.hash() != s.borrow().hash()`.
static_assertions::assert_not_impl_any!(FlyByteStr: Borrow<str>);

impl FlyByteStr {
    /// Create a `FlyByteStr`, allocating it in the cache if the value is not already cached.
    ///
    /// # Performance
    ///
    /// Creating an instance of this type for strings longer than the maximum inline size requires
    /// accessing the global cache of strings, which involves taking a lock. When multiple threads
    /// are allocating lots of strings there may be contention. Each string allocated is hashed for
    /// lookup in the cache.
    #[inline]
    pub fn new(s: impl AsRef<[u8]>) -> Self {
        Self(RawRepr::new(s.as_ref()))
    }

    /// Returns the underlying bytestring slice.
    #[inline]
    pub fn as_bstr(&self) -> &BStr {
        BStr::new(self.0.as_bytes())
    }

    /// Returns the underlying byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl Default for FlyByteStr {
    #[inline]
    fn default() -> Self {
        Self::new(b"")
    }
}

impl From<&'_ [u8]> for FlyByteStr {
    #[inline]
    fn from(s: &[u8]) -> Self {
        Self::new(s)
    }
}

impl<const N: usize> From<[u8; N]> for FlyByteStr {
    #[inline]
    fn from(s: [u8; N]) -> Self {
        Self::new(s)
    }
}

impl From<&'_ BStr> for FlyByteStr {
    #[inline]
    fn from(s: &BStr) -> Self {
        Self(RawRepr::new(s))
    }
}

impl From<&'_ str> for FlyByteStr {
    #[inline]
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<&'_ Vec<u8>> for FlyByteStr {
    #[inline]
    fn from(s: &Vec<u8>) -> Self {
        Self(RawRepr::new(s))
    }
}

impl From<&'_ String> for FlyByteStr {
    #[inline]
    fn from(s: &String) -> Self {
        Self::new(&**s)
    }
}

impl From<Vec<u8>> for FlyByteStr {
    #[inline]
    fn from(s: Vec<u8>) -> Self {
        Self::new(s)
    }
}

impl From<String> for FlyByteStr {
    #[inline]
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<BString> for FlyByteStr {
    #[inline]
    fn from(s: BString) -> Self {
        Self::new(s)
    }
}

impl From<Box<[u8]>> for FlyByteStr {
    #[inline]
    fn from(s: Box<[u8]>) -> Self {
        Self::new(s)
    }
}

impl From<Box<str>> for FlyByteStr {
    #[inline]
    fn from(s: Box<str>) -> Self {
        Self(RawRepr::new(s.as_bytes()))
    }
}

impl From<&'_ Box<[u8]>> for FlyByteStr {
    #[inline]
    fn from(s: &'_ Box<[u8]>) -> Self {
        Self(RawRepr::new(s))
    }
}

impl From<&Box<str>> for FlyByteStr {
    #[inline]
    fn from(s: &Box<str>) -> Self {
        Self::new(&**s)
    }
}

impl From<FlyByteStr> for BString {
    #[inline]
    fn from(s: FlyByteStr) -> BString {
        s.as_bstr().to_owned()
    }
}

impl From<FlyByteStr> for Vec<u8> {
    #[inline]
    fn from(s: FlyByteStr) -> Vec<u8> {
        s.as_bytes().to_owned()
    }
}

impl From<FlyStr> for FlyByteStr {
    #[inline]
    fn from(s: FlyStr) -> FlyByteStr {
        Self(s.0)
    }
}

impl TryInto<String> for FlyByteStr {
    type Error = std::string::FromUtf8Error;

    #[inline]
    fn try_into(self) -> Result<String, Self::Error> {
        String::from_utf8(self.into())
    }
}

impl Deref for FlyByteStr {
    type Target = BStr;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_bstr()
    }
}

impl AsRef<BStr> for FlyByteStr {
    #[inline]
    fn as_ref(&self) -> &BStr {
        self.as_bstr()
    }
}

impl PartialOrd for FlyByteStr {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for FlyByteStr {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_bstr().cmp(other.as_bstr())
    }
}

impl PartialEq<[u8]> for FlyByteStr {
    #[inline]
    fn eq(&self, other: &[u8]) -> bool {
        self.as_bytes() == other
    }
}

impl PartialEq<BStr> for FlyByteStr {
    #[inline]
    fn eq(&self, other: &BStr) -> bool {
        self.as_bytes() == other
    }
}

impl PartialEq<str> for FlyByteStr {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl PartialEq<&'_ [u8]> for FlyByteStr {
    #[inline]
    fn eq(&self, other: &&[u8]) -> bool {
        self.as_bytes() == *other
    }
}

impl PartialEq<&'_ BStr> for FlyByteStr {
    #[inline]
    fn eq(&self, other: &&BStr) -> bool {
        self.as_bstr() == *other
    }
}

impl PartialEq<&'_ str> for FlyByteStr {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl PartialEq<String> for FlyByteStr {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

impl PartialEq<FlyStr> for FlyByteStr {
    #[inline]
    fn eq(&self, other: &FlyStr) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<&'_ FlyStr> for FlyByteStr {
    #[inline]
    fn eq(&self, other: &&FlyStr) -> bool {
        self.0 == other.0
    }
}

impl PartialOrd<str> for FlyByteStr {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<std::cmp::Ordering> {
        self.as_bstr().partial_cmp(other)
    }
}

impl PartialOrd<&str> for FlyByteStr {
    #[inline]
    fn partial_cmp(&self, other: &&str) -> Option<std::cmp::Ordering> {
        self.as_bstr().partial_cmp(other)
    }
}

impl PartialOrd<FlyStr> for FlyByteStr {
    #[inline]
    fn partial_cmp(&self, other: &FlyStr) -> Option<std::cmp::Ordering> {
        self.as_bstr().partial_cmp(other.as_str())
    }
}

impl Debug for FlyByteStr {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self.as_bstr(), f)
    }
}

impl Display for FlyByteStr {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self.as_bstr(), f)
    }
}

#[cfg(feature = "serde")]
impl Serialize for FlyByteStr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.as_bytes())
    }
}

#[cfg(feature = "serde")]
impl<'d> Deserialize<'d> for FlyByteStr {
    fn deserialize<D: Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_bytes(FlyByteStrVisitor)
    }
}

#[cfg(feature = "serde")]
struct FlyByteStrVisitor;

#[cfg(feature = "serde")]
impl<'de> Visitor<'de> for FlyByteStrVisitor {
    type Value = FlyByteStr;
    fn expecting(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str("a string, a bytestring, or a sequence of bytes")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(FlyByteStr::from(v))
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(FlyByteStr::from(v))
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(FlyByteStr::from(v))
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut bytes = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(b) = seq.next_element::<u8>()? {
            bytes.push(b);
        }
        Ok(FlyByteStr::from(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::*;
    use crate::MAX_INLINE_SIZE;
    use static_assertions::{const_assert, const_assert_eq};
    use std::collections::{BTreeSet, HashSet};
    use test_case::test_case;

    // These tests all manipulate the process-global cache in the parent module. On target devices
    // we run each test case in its own process, so the test cases can't pollute each other. On
    // host, we run tests with a process for each suite (which is the Rust upstream default), and
    // we need to manually isolate the tests from each other.
    #[cfg(not(target_os = "fuchsia"))]
    use serial_test::serial;

    const SHORT_STRING: &str = "hello";
    const_assert!(SHORT_STRING.len() < MAX_INLINE_SIZE);

    const MAX_LEN_SHORT_STRING: &str = "hello!!";
    const_assert_eq!(MAX_LEN_SHORT_STRING.len(), MAX_INLINE_SIZE);

    const MIN_LEN_LONG_STRING: &str = "hello!!!";
    const_assert_eq!(MIN_LEN_LONG_STRING.len(), MAX_INLINE_SIZE + 1);

    const LONG_STRING: &str = "hello, world!!!!!!!!!!!!!!!!!!!!";
    const_assert!(LONG_STRING.len() > MAX_INLINE_SIZE);

    const SHORT_NON_UTF8: &[u8] = b"\xF0\x28\x8C\x28";
    const_assert!(SHORT_NON_UTF8.len() < MAX_INLINE_SIZE);

    const LONG_NON_UTF8: &[u8] = b"\xF0\x28\x8C\x28\xF0\x28\x8C\x28";
    const_assert!(LONG_NON_UTF8.len() > MAX_INLINE_SIZE);

    #[test_case("" ; "empty string")]
    #[test_case(SHORT_STRING ; "short strings")]
    #[test_case(MAX_LEN_SHORT_STRING ; "max len short strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn no_allocations_for_short_bytestrings(contents: &str) {
        reset_global_cache();
        assert_eq!(num_strings_in_global_cache(), 0);

        let original = FlyByteStr::new(contents);
        assert_eq!(num_strings_in_global_cache(), 0);
        assert_eq!(original.0.refcount(), None);

        let cloned = original.clone();
        assert_eq!(num_strings_in_global_cache(), 0);
        assert_eq!(cloned.0.refcount(), None);

        let deduped = FlyByteStr::new(contents);
        assert_eq!(num_strings_in_global_cache(), 0);
        assert_eq!(deduped.0.refcount(), None);
    }

    #[test_case(MIN_LEN_LONG_STRING ; "barely long strings")]
    #[test_case(LONG_STRING ; "long strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn only_one_copy_allocated_for_long_bytestrings(contents: &str) {
        reset_global_cache();

        assert_eq!(num_strings_in_global_cache(), 0);

        let original = FlyByteStr::new(contents);
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

        let deduped = FlyByteStr::new(contents);
        assert_eq!(num_strings_in_global_cache(), 1, "new string was deduped");
        assert_eq!(deduped.0.refcount(), Some(3), "three copies on stack");
    }

    #[test_case(MIN_LEN_LONG_STRING ; "barely long strings")]
    #[test_case(LONG_STRING ; "long strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn cached_bytestrings_dropped_when_refs_dropped(contents: &str) {
        reset_global_cache();

        let alloced = FlyByteStr::new(contents);
        assert_eq!(
            num_strings_in_global_cache(),
            1,
            "only one string allocated"
        );
        drop(alloced);
        assert_eq!(num_strings_in_global_cache(), 0, "last reference dropped");
    }

    #[test_case("", SHORT_STRING ; "empty and short string")]
    #[test_case(SHORT_STRING, MAX_LEN_SHORT_STRING ; "two short strings")]
    #[test_case(SHORT_STRING, LONG_STRING ; "short and long strings")]
    #[test_case(LONG_STRING, MAX_LEN_SHORT_STRING ; "long and max-len-short strings")]
    #[test_case(MIN_LEN_LONG_STRING, LONG_STRING ; "barely long and long strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn byte_equality_and_hashing_with_pointer_value_works_correctly(first: &str, second: &str) {
        reset_global_cache();

        let first = FlyByteStr::new(first);
        let second = FlyByteStr::new(second);

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

    #[test_case("", SHORT_STRING ; "empty and short string")]
    #[test_case(SHORT_STRING, MAX_LEN_SHORT_STRING ; "two short strings")]
    #[test_case(SHORT_STRING, LONG_STRING ; "short and long strings")]
    #[test_case(LONG_STRING, MAX_LEN_SHORT_STRING ; "long and max-len-short strings")]
    #[test_case(MIN_LEN_LONG_STRING, LONG_STRING ; "barely long and long strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn byte_comparison_for_btree_storage_works(first: &str, second: &str) {
        reset_global_cache();

        let first = FlyByteStr::new(first);
        let second = FlyByteStr::new(second);

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
    #[test_case("" ; "empty string")]
    #[test_case(SHORT_STRING ; "short strings")]
    #[test_case(MAX_LEN_SHORT_STRING ; "max len short strings")]
    #[test_case(MIN_LEN_LONG_STRING ; "min len long strings")]
    #[test_case(LONG_STRING ; "long strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn serde_works_bytestring(contents: &str) {
        reset_global_cache();

        let s = FlyByteStr::new(contents);
        let as_json = serde_json::to_string(&s).unwrap();
        assert_eq!(s, serde_json::from_str::<FlyByteStr>(&as_json).unwrap());
    }

    #[test_case(SHORT_NON_UTF8 ; "short non-utf8 bytestring")]
    #[test_case(LONG_NON_UTF8 ; "long non-utf8 bytestring")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn non_utf8_works(contents: &[u8]) {
        reset_global_cache();

        let res: Result<FlyStr, _> = FlyByteStr::from(contents).try_into();
        res.unwrap_err();
    }
}

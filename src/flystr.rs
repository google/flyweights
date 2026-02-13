// Copyright 2022 The Fuchsia Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::{FlyByteStr, RawRepr};
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

/// An immutable string type which only stores a single copy of each string allocated. Internally
/// represented as a shared pointer to the backing allocation. Occupies a single pointer width.
///
/// # Small strings
///
/// Very short strings are stored inline in the pointer with bit-tagging, so no allocations are
/// performed.
///
/// # Performance
///
/// It's slower to construct than a regular `String` but trades that for reduced standing memory
/// usage by deduplicating strings. `PartialEq` and `Hash` are implemented on the underlying pointer
/// value rather than the pointed-to data for faster equality comparisons and indexing, which is
/// sound by virtue of the type guaranteeing that only one `FlyStr` pointer value will exist at
/// any time for a given string's contents.
///
/// As with any performance optimization, you should only use this type if you can measure the
/// benefit it provides to your program. Pay careful attention to creating `FlyStr`s in hot paths
/// as it may regress runtime performance.
///
/// # Allocation lifecycle
///
/// Intended for long-running system services with user-provided values, `FlyStr`s are removed from
/// the global cache when the last reference to them is dropped. While this incurs some overhead
/// it is important to prevent the value cache from becoming a denial-of-service vector.
#[derive(Clone, Eq, Hash, PartialEq)]
pub struct FlyStr(pub(crate) RawRepr);

#[cfg(feature = "json_schema")]
impl schemars::JsonSchema for FlyStr {
    fn schema_name() -> String {
        str::schema_name()
    }

    fn json_schema(generator: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        str::json_schema(generator)
    }

    fn is_referenceable() -> bool {
        false
    }
}

static_assertions::assert_eq_size!(FlyStr, usize);

// `Borrow` requires that:
//
// > Eq, Ord and Hash must be equivalent for borrowed and owned values: x.borrow() == y.borrow()
// > should give the same result as x == y.
//
// The current implementation of `FlyStr` cannot satisfy `Borrow<str>` because the O(1) hashing
// feature means that `s.hash() != s.borrow().hash()`.
static_assertions::assert_not_impl_any!(FlyStr: Borrow<str>);

impl FlyStr {
    /// Create a `FlyStr`, allocating it in the cache if the value is not already cached.
    ///
    /// # Performance
    ///
    /// Creating an instance of this type for strings longer than the maximum inline size requires
    /// accessing the global cache of strings, which involves taking a lock. When multiple threads
    /// are allocating lots of strings there may be contention. Each string allocated is hashed for
    /// lookup in the cache.
    #[inline]
    pub fn new(s: impl AsRef<str>) -> Self {
        Self(RawRepr::new(s.as_ref().as_bytes()))
    }

    /// Returns the underlying string slice.
    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: Every FlyStr is constructed from valid UTF-8 bytes.
        unsafe { std::str::from_utf8_unchecked(self.0.as_bytes()) }
    }
}

impl Default for FlyStr {
    #[inline]
    fn default() -> Self {
        Self::new("")
    }
}

impl From<&'_ str> for FlyStr {
    #[inline]
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<&'_ String> for FlyStr {
    #[inline]
    fn from(s: &String) -> Self {
        Self::new(&**s)
    }
}

impl From<String> for FlyStr {
    #[inline]
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<Box<str>> for FlyStr {
    #[inline]
    fn from(s: Box<str>) -> Self {
        Self::new(s)
    }
}

impl From<&Box<str>> for FlyStr {
    #[inline]
    fn from(s: &Box<str>) -> Self {
        Self::new(&**s)
    }
}

impl TryFrom<FlyByteStr> for FlyStr {
    type Error = std::str::Utf8Error;

    #[inline]
    fn try_from(b: FlyByteStr) -> Result<FlyStr, Self::Error> {
        // The internals of both FlyStr and FlyByteStr are the same, but it's only sound to return
        // a FlyStr if the RawRepr contains/points to valid UTF-8.
        std::str::from_utf8(b.as_bytes())?;
        Ok(FlyStr(b.0))
    }
}

impl From<FlyStr> for String {
    #[inline]
    fn from(s: FlyStr) -> String {
        s.as_str().to_owned()
    }
}

impl From<&'_ FlyStr> for String {
    #[inline]
    fn from(s: &FlyStr) -> String {
        s.as_str().to_owned()
    }
}

impl Deref for FlyStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for FlyStr {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl PartialOrd for FlyStr {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for FlyStr {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialEq<str> for FlyStr {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&'_ str> for FlyStr {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for FlyStr {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str() == &**other
    }
}

impl PartialEq<FlyByteStr> for FlyStr {
    #[inline]
    fn eq(&self, other: &FlyByteStr) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<&'_ FlyByteStr> for FlyStr {
    #[inline]
    fn eq(&self, other: &&FlyByteStr) -> bool {
        self.0 == other.0
    }
}

impl PartialOrd<str> for FlyStr {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<std::cmp::Ordering> {
        self.as_str().partial_cmp(other)
    }
}

impl PartialOrd<&str> for FlyStr {
    #[inline]
    fn partial_cmp(&self, other: &&str) -> Option<std::cmp::Ordering> {
        self.as_str().partial_cmp(*other)
    }
}

impl PartialOrd<FlyByteStr> for FlyStr {
    #[inline]
    fn partial_cmp(&self, other: &FlyByteStr) -> Option<std::cmp::Ordering> {
        bstr::BStr::new(self.as_str()).partial_cmp(other.as_bstr())
    }
}

impl PartialOrd<&'_ FlyByteStr> for FlyStr {
    #[inline]
    fn partial_cmp(&self, other: &&FlyByteStr) -> Option<std::cmp::Ordering> {
        bstr::BStr::new(self.as_str()).partial_cmp(other.as_bstr())
    }
}

impl Debug for FlyStr {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Debug::fmt(self.as_str(), f)
    }
}

impl Display for FlyStr {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        Display::fmt(self.as_str(), f)
    }
}

#[cfg(feature = "serde")]
impl Serialize for FlyStr {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'d> Deserialize<'d> for FlyStr {
    fn deserialize<D: Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(FlyStrVisitor)
    }
}

#[cfg(feature = "serde")]
struct FlyStrVisitor;

#[cfg(feature = "serde")]
impl Visitor<'_> for FlyStrVisitor {
    type Value = FlyStr;
    fn expecting(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str("a string")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(v.into())
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match str::from_utf8(v) {
            Ok(s) => Ok(FlyStr::new(s)),
            Err(_) => Err(serde::de::Error::invalid_value(
                serde::de::Unexpected::Bytes(v),
                &self,
            )),
        }
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

    #[test_case("" ; "empty string")]
    #[test_case(SHORT_STRING ; "short strings")]
    #[test_case(MAX_LEN_SHORT_STRING ; "max len short strings")]
    #[test_case(MIN_LEN_LONG_STRING ; "barely long strings")]
    #[test_case(LONG_STRING ; "long strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn string_formatting_is_equivalent_to_str(original: &str) {
        reset_global_cache();

        let cached = FlyStr::new(original);
        assert_eq!(format!("{original}"), format!("{cached}"));
        assert_eq!(format!("{original:?}"), format!("{cached:?}"));

        let cached = FlyByteStr::new(original);
        assert_eq!(format!("{original}"), format!("{cached}"));
        assert_eq!(format!("{original:?}"), format!("{cached:?}"));
    }

    #[test_case("" ; "empty string")]
    #[test_case(SHORT_STRING ; "short strings")]
    #[test_case(MAX_LEN_SHORT_STRING ; "max len short strings")]
    #[test_case(MIN_LEN_LONG_STRING ; "barely long strings")]
    #[test_case(LONG_STRING ; "long strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn string_equality_works(contents: &str) {
        reset_global_cache();

        let cached = FlyStr::new(contents);
        let bytes_cached = FlyByteStr::new(contents);
        assert_eq!(cached, cached.clone(), "must be equal to itself");
        assert_eq!(cached, contents, "must be equal to the original");
        assert_eq!(
            cached,
            contents.to_owned(),
            "must be equal to an owned copy of the original"
        );
        assert_eq!(cached, bytes_cached);

        // test inequality too
        assert_ne!(cached, "goodbye");
        assert_ne!(bytes_cached, "goodbye");
    }

    #[test_case("", SHORT_STRING ; "empty and short string")]
    #[test_case(SHORT_STRING, MAX_LEN_SHORT_STRING ; "two short strings")]
    #[test_case(MAX_LEN_SHORT_STRING, MIN_LEN_LONG_STRING ; "short and long strings")]
    #[test_case(MIN_LEN_LONG_STRING, LONG_STRING ; "barely long and long strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn string_comparison_works(lesser_contents: &str, greater_contents: &str) {
        reset_global_cache();

        let lesser = FlyStr::new(lesser_contents);
        let lesser_bytes = FlyByteStr::from(lesser_contents);
        let greater = FlyStr::new(greater_contents);
        let greater_bytes = FlyByteStr::from(greater_contents);

        // lesser as method receiver
        assert!(lesser < greater);
        assert!(lesser < greater_bytes);
        assert!(lesser_bytes < greater);
        assert!(lesser_bytes < greater_bytes);
        assert!(lesser <= greater);
        assert!(lesser <= greater_bytes);
        assert!(lesser_bytes <= greater);
        assert!(lesser_bytes <= greater_bytes);

        // greater as method receiver
        assert!(greater > lesser);
        assert!(greater > lesser_bytes);
        assert!(greater_bytes > lesser);
        assert!(greater >= lesser);
        assert!(greater >= lesser_bytes);
        assert!(greater_bytes >= lesser);
        assert!(greater_bytes >= lesser_bytes);
    }

    #[test_case("" ; "empty string")]
    #[test_case(SHORT_STRING ; "short strings")]
    #[test_case(MAX_LEN_SHORT_STRING ; "max len short strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn no_allocations_for_short_strings(contents: &str) {
        reset_global_cache();
        assert_eq!(num_strings_in_global_cache(), 0);

        let original = FlyStr::new(contents);
        assert_eq!(num_strings_in_global_cache(), 0);
        assert_eq!(original.0.refcount(), None);

        let cloned = original.clone();
        assert_eq!(num_strings_in_global_cache(), 0);
        assert_eq!(cloned.0.refcount(), None);

        let deduped = FlyStr::new(contents);
        assert_eq!(num_strings_in_global_cache(), 0);
        assert_eq!(deduped.0.refcount(), None);
    }

    #[test_case(MIN_LEN_LONG_STRING ; "barely long strings")]
    #[test_case(LONG_STRING ; "long strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn only_one_copy_allocated_for_long_strings(contents: &str) {
        reset_global_cache();

        assert_eq!(num_strings_in_global_cache(), 0);

        let original = FlyStr::new(contents);
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

        let deduped = FlyStr::new(contents);
        assert_eq!(num_strings_in_global_cache(), 1, "new string was deduped");
        assert_eq!(deduped.0.refcount(), Some(3), "three copies on stack");
    }

    #[test]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn utf8_and_bytestrings_share_the_cache() {
        reset_global_cache();

        assert_eq!(num_strings_in_global_cache(), 0, "cache is empty");

        let _utf8 = FlyStr::from(MIN_LEN_LONG_STRING);
        assert_eq!(num_strings_in_global_cache(), 1, "string was allocated");

        let _bytes = FlyByteStr::from(MIN_LEN_LONG_STRING);
        assert_eq!(
            num_strings_in_global_cache(),
            1,
            "bytestring was pulled from cache"
        );
    }

    #[test_case(MIN_LEN_LONG_STRING ; "barely long strings")]
    #[test_case(LONG_STRING ; "long strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn cached_strings_dropped_when_refs_dropped(contents: &str) {
        reset_global_cache();

        let alloced = FlyStr::new(contents);
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
    fn equality_and_hashing_with_pointer_value_works_correctly(first: &str, second: &str) {
        reset_global_cache();

        let first = FlyStr::new(first);
        let second = FlyStr::new(second);

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
    fn comparison_for_btree_storage_works(first: &str, second: &str) {
        reset_global_cache();

        let first = FlyStr::new(first);
        let second = FlyStr::new(second);

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
    fn serde_works(contents: &str) {
        reset_global_cache();

        let s = FlyStr::new(contents);
        let as_json = serde_json::to_string(&s).unwrap();
        assert_eq!(as_json, format!("\"{contents}\""));
        assert_eq!(s, serde_json::from_str::<FlyStr>(&as_json).unwrap());
    }

    #[test_case("" ; "empty string")]
    #[test_case(SHORT_STRING ; "short strings")]
    #[test_case(MAX_LEN_SHORT_STRING ; "max len short strings")]
    #[test_case(MIN_LEN_LONG_STRING ; "min len long strings")]
    #[test_case(LONG_STRING ; "long strings")]
    #[cfg_attr(not(target_os = "fuchsia"), serial)]
    fn flystr_to_flybytestr_and_back(contents: &str) {
        reset_global_cache();

        let bytestr = FlyByteStr::from(contents);
        let flystr = FlyStr::try_from(bytestr.clone()).unwrap();
        assert_eq!(bytestr, flystr);
        let bytestr2 = FlyByteStr::from(flystr.clone());
        assert_eq!(bytestr, bytestr2);
    }
}

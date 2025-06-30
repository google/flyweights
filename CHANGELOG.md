# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.4](https://github.com/google/flyweights/compare/v0.1.3...v0.1.4) - 2025-06-30

### Other

- Add auto-releasing configuration.
- Fix typo in nightly toolchain pin.
- Verify serde feature without schemars.
- Pin nightly toolchain version for miri checks.
- Fix clippy lints in benchmark.
- Remove unneeded actions badge.
- Remove cargo bench from workflow config.
- Fix clippy lints.
- Update MSRV to 1.81.0.
- Add GitHub Actions workflow.
- Add breadcrumb about Borrow<str> implementation.
- Disable bstr default features.
- Bump version number to allow publishing with correct repository.
- Bump patch version to allow new publish.
- Tweak docs after reviewing rendered output.
- Optimize memory usage
- Move design tradeoff discussion to crate docs.
- Convert crate comparisons into design tradeoffs.
- Fix clippy::all lints on latest toolchain.
- Fix up cargo packaging.
- Plot results on log scales.
- Support running under miri.
- Set a minimum Rust version.
- Port benchmark to regular criterion.
- Move benchmark to cargo-friendly location.
- Run cargo fmt with default config.
- Sort dev-dependencies.
- Make serde dependency optional.
- Set correct SPDX identifier in Cargo.toml.
- Add Google contributing and CoC docs.
- Set up build with cargo.
- Add LICENSE file.
- Use FlyByteStr in MappingName.
- Add json schema support.
- Avoid locking cache in drop
- Inline string representation
- Address flakes on host.
- Add FlyByteStr type.
- Use raw bytes for storage.
- Fix non_std_lazy_statics with fx shush
- Remove ManuallyDrop.
- Add inline attribute to small functions.
- Remove branch from hashing.
- Remove branches from equality comparisons.
- Use AHashSet for storage.
- Add basic benchmarks.
- Format imports using rustfmt
- Fix clippy::incorrect_partial_ord_impl_on_ord_type
- Short string optimization.
- Use FlyStr for component identity related strings
- Use Arc<Box<str>> instead of Arc<String>
- Define FlyStr interned string type.

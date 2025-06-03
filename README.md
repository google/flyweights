# flyweights

Types implementing the [flyweight pattern] for interning object allocations. Supports UTF-8 strings
and bytestrings.

# Goals & Tradeoffs

There are many (many) crates for interning strings in Rust. This was originally written for
[Fuchsia](https://fuchsia.dev) when the existing options didn't seem to fit the needs of a
long-running system service. These are some of the ways in which it differs from some other
approaches.

## Easy to retrofit into multithreading

To avoid large refactors when adopting in existing programs with many threads and lots of strings,
`flyweights` doesn't need any additional arguments to create, read, or drop a string and the types
all implement `Send` and `Sync`.

This requires that the string cache is global and that users have less control over the lifecycle of
underlying allocations compared to some other approaches.

## Uses memory proportional to live strings

To be suitable for long-running processes, memory usage is limited to currently live strings.
Strings are reference-counted, and the cached values are dropped when the last reference is dropped.

As a result there is more runtime overhead required to keep track of string lifecycle when compared
with some other approaches.

## Avoids latency spikes

Aside from the latency of reallocating the cache storage, there are no large batch operations like
explicit garbage collection of the cache.

This comes with some bookkeeping overhead that could potentially be avoided in a library that's
able to restrict cleaning up the cache to specific points in the program.

## Minimal memory usage

`flyweights`' string types all have a small string optimization (SSO) that stores very short strings
inline in the pointer instead of heap allocating in the cache. This can save a significant amount
of memory depending on the workload.

This does create a significant amount of `unsafe` which has been thoroughly reviewed and is tested
with `miri`.

## O(1) equality and hashing

Because `flyweights` uses global storage, there is only a single pointer value for any given cached
string at any given time. Because the pointer value uniquely identifies the string, we can implement
equality and hashing on the pointer value instead of the string contents, making a typically O(n)
operation into O(1) and avoiding a pointer chase.

The main cost of this feature is that the strings in this crate don't implement `Borrow<str>`.

# License

See [LICENSE](LICENSE).

# Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

# Code of Conduct

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

[flyweight pattern]: https://en.wikipedia.org/wiki/Flyweight_pattern

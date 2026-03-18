# banish_derive

This crate is the procedural macro implementation backing [`banish`](https://crates.io/crates/banish).

**You should not depend on this crate directly.** Add `banish` to your dependencies instead:

```toml
[dependencies]
banish = "1.2.3"
```

Or with cargo:

```
cargo add banish
```

`banish` re-exports everything you need. This crate is an implementation detail and its API is not considered stable outside of what `banish` exposes.
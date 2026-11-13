# Contributing to probnode-rs

Thank you for helping. Bug reports, fixes and improvements to the documentation are all welcome.

## Ground rules

- **The API is the contract.** The client mirrors the ProbNode HTTP API; a method that needs a
  change on the service side starts as an issue, not a pull request.
- **The same surface as the other clients.** The Python and C++ clients have the same methods;
  a new method here should exist, or be planned, there too.
- **Never trust a download.** Every file is written to a `.part`, checked against its sha256 and
  only then renamed. A change to `download` keeps that, with a test.
- **The key stays with the API.** It is never sent to a signed-link host or across a redirect.

## Development

You need a stable Rust toolchain.

```sh
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc -p probnode --all-features --no-deps
```

The tests run against a mock server and need no network, except `tests/live.rs`, which runs only
when `PROBNODE_LIVE_BASE_URL` is set and calls the public, keyless endpoints:

```sh
PROBNODE_LIVE_BASE_URL=https://probnode.com cargo test --test live
```

CI runs the same on Linux, macOS and Windows.

## Pull requests

- One topic per pull request, with a test that fails without the change.
- Keep public items documented (`#![warn(missing_docs)]`).
- Add a line to `CHANGELOG.md` under the unreleased version.

## Code of conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). By participating you agree
to uphold it.

## License

By contributing you agree that your contributions are licensed under the Apache License 2.0, as
the project is (section 5 of the license).

# Contributing to Group Protocol Stack

Thank you for your interest in contributing.

## Before you start

- Read the [README](README.md) to understand the project structure.
- Check existing [issues](https://github.com/F000NKKK/Group-Protocol-Stack/issues) and
  [pull requests](https://github.com/F000NKKK/Group-Protocol-Stack/pulls) to avoid duplication.
- For significant changes, open an issue first to discuss the approach.

## Development setup

**Prerequisites:** Rust stable toolchain, .NET 10 SDK, Node.js 20+, Python 3.11+.

```bash
# Clone
git clone https://github.com/F000NKKK/Group-Protocol-Stack.git
cd Group-Protocol-Stack

# Build Rust workspace
cargo build --workspace

# Run all Rust tests
cargo test --workspace

# Build .NET bindings
dotnet build csharp/GBPStack

# Build Node.js bindings
cd js && npm install && npm run build

# Build Python bindings
cd python && pip install -e .
```

## Project structure

```
crates/          Rust workspace (protocol crates)
csharp/          .NET bindings
js/              Node.js bindings
python/          Python bindings
docs/            Protocol specifications
scripts/         Release and codegen scripts
```

## Making changes

1. Fork the repository and create a branch from `master`.
2. Make your changes with tests.
3. Run `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings`.
4. Ensure all tests pass: `cargo test --workspace`.
5. Commit using [Conventional Commits](#commit-messages).
6. Open a pull request against `master`.

## Commit messages

This project uses [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add SFrame key rotation support
fix: correct replay window overflow in gbp-node
docs: update GTP spec with watermark semantics
refactor: simplify MLS exporter path in gbp-mls
test: add round-trip vectors for GAP audio frames
chore: bump openmls to 0.6
```

Breaking changes must include `BREAKING CHANGE:` in the commit footer.

## Pull request guidelines

- Keep PRs focused — one logical change per PR.
- Link the related issue in the PR description.
- Update documentation if the public API or protocol behaviour changes.
- All CI checks must pass before merge.

## Crate-level rules

- No `unwrap()` in production paths — use `?` or explicit error handling.
- No `unsafe` outside `gbp-stack-ffi` and narrow cryptographic contexts.
- Public items must have doc comments (`#![deny(missing_docs)]` is enforced).
- Use `tracing` for logging, not `println!`.

## License

By contributing you agree that your contributions will be licensed under
the [Apache License 2.0](LICENSE).

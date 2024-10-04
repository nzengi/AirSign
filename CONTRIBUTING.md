# Contributing

Thanks for taking the time to contribute.

## Getting started

1. Fork the repo and clone your fork
2. `cargo build` — make sure everything compiles
3. `cargo test` — all tests should pass before you start

## Conventions

- **Formatting**: `cargo fmt` before committing. CI enforces this.
- **Lints**: `cargo clippy -- -D warnings` should be clean.
- **Commits**: use [Conventional Commits](https://www.conventionalcommits.org/)
  (`feat:`, `fix:`, `refactor:`, `docs:`, `chore:`).
- **Tests**: new behaviour needs a test. Bug fixes should include a regression
  test.

## Crate structure

Changes to `airsign-core` protocol framing or the fountain code are the most
sensitive — those affect wire compatibility. If you change the frame format,
bump the protocol version constant and update `CHANGELOG.md`.

Changes to `airsign-optical` are more self-contained but camera and display
code is hard to test in CI. Keep the feature-gated paths narrow so the
no-hardware path compiles and tests cleanly.

## Opening a pull request

- Keep PRs focused. One logical change per PR.
- Fill in the PR description: what problem does this solve, how does it solve
  it, what did you test.
- Reference related issues if any.

## Reporting issues

Open a GitHub issue. Include your OS, Rust toolchain version (`rustc -V`),
and the minimal steps to reproduce.
# Contributing to Steward

Thank you for helping improve Steward. Contributions of code, tests, documentation, bug reports, and design feedback are welcome.

## Before you start

- Search existing issues and pull requests before opening a duplicate.
- For substantial changes, open an issue first so the scope and compatibility impact can be discussed.
- Do not include credentials, access tokens, private data, generated secrets, or proprietary code in issues, commits, logs, or test fixtures.
- Report security vulnerabilities privately as described in [SECURITY.md](SECURITY.md).

## Development setup

Requirements:

- Stable Rust toolchain with `rustfmt` and `clippy`
- Node.js 20 or newer
- PowerShell 7 for Windows packaging and setup scripts

Build the web console and Rust workspace:

```powershell
cd web-ui
npm ci
npm run build
cd ..
cargo build --workspace --locked
```

## Verification

Run the checks relevant to your change before opening a pull request:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked -- --test-threads=1
cd web-ui
npm ci
npm run lint
npm run build
```

Document any check you could not run and explain why in the pull request.

## Pull requests

A good pull request should:

- describe the problem and the chosen solution;
- keep unrelated refactoring out of scope;
- include tests for behavioral changes and regressions;
- update documentation when commands, configuration, security boundaries, or user-visible behavior change;
- explain security and compatibility implications;
- remain small enough to review reliably whenever practical.

Conventional-style commit messages such as `feat:`, `fix:`, `docs:`, `test:`, and `chore:` are preferred but not required.

## Licensing

By submitting a contribution, you agree that it may be distributed under the terms of the [MIT License](LICENSE). You also certify that you have the right to submit the contribution.

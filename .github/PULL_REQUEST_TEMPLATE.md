## Summary

Describe the problem and the solution in a few sentences.

## Type of change

- [ ] Bug fix
- [ ] New feature
- [ ] Security hardening
- [ ] Refactor
- [ ] Documentation
- [ ] Build, packaging, or CI

## Verification

List the commands and manual checks you ran. Explain any check you could not run.

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked -- --test-threads=1`
- [ ] `cd web-ui && npm ci && npm run lint && npm run build`
- [ ] Relevant manual behavior was tested

## Security and compatibility

- Does this change tool permissions, command execution, filesystem access, networking, credentials, MCP processes, WebAssembly, persistence, or import/export behavior?
- Does it change a public command, configuration field, API, state format, or installation path?
- What failure behavior should reviewers test?

## Documentation

- [ ] User-facing documentation was updated where required
- [ ] Tests cover behavioral changes or regressions
- [ ] No credentials, private data, or generated secrets are included

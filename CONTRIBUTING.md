# Contributing

PRs and issues are welcome. If you're planning something big, open an issue first so we don't work at cross purposes.

## Project structure

```
cmd/sortie/main.rs      entrypoint, calls cli parser
cmd/agent/main.rs       agent binary (proxy mode)
pkg/cli.rs              clap command definitions
pkg/config.rs           reads and validates sortie.toml
pkg/types.rs            config structs, deploy records, host status
pkg/build.rs            runs cargo build with target triple
pkg/ssh.rs              connects via SSH key auth, runs commands, uploads files
pkg/cluster.rs          rolling deploy orchestration across multiple hosts
pkg/deploy.rs           single-host deploy logic
pkg/systemd.rs          generates systemd unit files and runs systemctl commands
pkg/health.rs           HTTP health check polling
pkg/rollback.rs         restores .bak file on a host
pkg/init.rs             creates sortie.toml and records directory
pkg/proxy.rs            embedded HTTP reverse proxy
```

## Development

```
cargo build
cargo clippy
cargo test
```

Tests worth running before any PR:

```
cargo test          46 unit tests + 2 integration tests across all modules
cargo clippy        must be clean (zero warnings)
cargo build         must compile on stable (1.70+)
```

## Design notes

- Errors are `Result<(), String>` everywhere. No thiserror, no anyhow. Keep it simple.
- SSH functions live in `ssh.rs`, everything remote goes through them. Don't open raw TCP connections outside of `ssh.rs` or `health.rs`.
- Rolling deploys iterate hosts sequentially with a health check between each. That's by design. If you want parallel deploys, that should be a separate command or flag.
- New commands must be added to both `cli.rs` and `main.rs`. If they interact with servers, add a function in `cluster.rs`.
- Every module gets a `#[cfg(test)] mod tests` at the bottom. Pure functions (generate_service, sh_quote, get_hosts) have comprehensive tests. Functions with side effects test their error paths.

## PR guidelines

- Keep PRs focused. One feature or fix per PR.
- Update CHANGELOG.md for user-facing changes.
- Don't add new dependencies unless there's a strong reason. The current dependency list is intentionally small.
- Match the existing code style. No tabs. 4-space indent. No semicolons where Rust lets you skip them. No comments unless the code genuinely needs explanation.

## Releasing

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Commit: `git commit -m "v0.x.y"`
4. Tag: `git tag v0.x.y`
5. Push: `git push origin master --tags`
6. The GitHub Actions release workflow will build binaries and publish to crates.io

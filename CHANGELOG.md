# Changelog

## v0.2.0 (2026-07-24)

### Features
- Multi-host cluster support (declare multiple servers per target)
- Rolling deployments across hosts with health gates
- `sortie apply` command for desired-state reconciliation
- `sortie get` for cluster-wide status
- `sortie describe` for detailed target info
- `sortie logs` for remote journalctl access
- Environment variable injection into systemd units
- Backward compatible single-host mode

### Fixes
- Clippy warnings resolved across all modules

## v0.1.0 (2026-07-24)

### Features
- One-command Rust binary deployment via SSH
- systemd service generation and management
- Health check with automatic rollback
- `sortie init`, `sortie deploy`, `sortie rollback`, `sortie status`
- TOML-based configuration
- Deploy records for audit trail

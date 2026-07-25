# Security

## Reporting

If you find a security issue, open a GitHub issue but mark it confidential if GitHub allows, or email the maintainer. Do not post it publicly until we've had a chance to fix it.

## What sortie does that matters for security

sortie copies binaries and runs commands on remote servers over SSH. That means:

- **SSH keys**: sortie reads your private key to log into servers. It never sends the key itself, just uses it locally to authenticate. The key stays on your machine.
- **Remote command execution**: sortie runs shell commands (systemctl, journalctl, mv, etc.) and uploads your binary via SCP. Whatever the SSH user can do, sortie can do.
- **Binary integrity**: we don't sign or checksum the binaries you deploy. The hash stored in deploy records is for tracking, not verification. If someone can MITM your SSH connection, they can replace the binary.
- **Secrets in config**: sortie reads environment variables from your sortie.toml and writes them into systemd unit files. Keep your config file locked down. Don't commit secrets to git.
- **Health checks**: sortie connects to your app's HTTP endpoint after deploy. It's a plain TCP/HTTP call. No auth is sent.

## What we recommend

- Use ed25519 keys (not RSA) and protect them with a passphrase
- Deploy from a dedicated CI machine, not your laptop
- Keep sortie.toml out of version control or use a .env pattern
- Use firewall rules so only your deploy machine can SSH into production
- Regularly rotate deployment keys
- Pin the sortie version you use in CI (avoid `latest`)

## Supported versions

We support the latest release. Older versions don't get backports. If you need something specific, open an issue.

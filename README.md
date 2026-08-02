<p align="center">
  <img src="logo/sortie.png" alt="sortie" width="400">
</p>

<h1 align="center">sortie</h1>

<p align="center">
  <strong>Kubernetes for Rust.</strong>
  <br>
  Deploy, manage, and monitor Rust services across a cluster of servers from one CLI.
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square" alt="License"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-1.70%2B-orange.svg?style=flat-square" alt="Rust"></a>
  <a href="https://github.com/Emran-goat/sortie/actions"><img src="https://img.shields.io/github/actions/workflow/status/Emran-goat/sortie/ci.yml?style=flat-square" alt="CI"></a>
  <a href="https://github.com/Emran-goat/sortie/releases"><img src="https://img.shields.io/github/v/release/Emran-goat/sortie?style=flat-square" alt="Release"></a>
  <a href="CONTRIBUTING.md"><img src="https://img.shields.io/badge/contributions-welcome-brightgreen.svg?style=flat-square" alt="PRs Welcome"></a>
</p>

## Features

| Capability | What it does |
|------------|-------------|
| Multi host clusters | Deploy to a fleet of servers, not just one |
| Rolling updates | Hosts updated one at a time with health gates |
| Canary & blue-green | Deploy to a subset, or spin a full parallel stack |
| Embedded reverse proxy | Built in HTTP proxy with Host based routing. No nginx needed. |
| Declarative state | Declare targets in sortie.toml; `sortie apply` reconciles |
| Auto rollback | Failed health checks trigger automatic rollback |
| Service registry | Register services, resolve by name, generate ingress config |
| Secrets management | Encrypted key-value store on servers (SSH is the boundary) |
| Auto scaling | CPU-based scaling loop, adjusts systemd instances |
| Self healing | sortie-agent watchdog restarts dead services |
| kubectl style CLI | `apply`, `get`, `describe`, `logs`, `rollback`, `scale` |
| Multi cluster federation | Deploy to all targets with one command |
| TLS provisioning | Auto certs via certbot (Let's Encrypt) |
| Observability | Per-host metrics (CPU, memory) and log aggregation |
| Environment config | Inline env vars for per-target service configuration |

## Quick start

```
cargo install sortie
sortie init
```

Edit `sortie.toml` with your server details, then deploy:

```
sortie apply production
```

This builds your binary, uploads it via SCP to every host, installs a systemd service, starts it, and waits for the health endpoint to respond. If a health check fails, that host rolls back.

To see what would change without deploying:

```
sortie apply production --check
```

### Embedded proxy

No nginx, no caddy, no setup. Install the proxy on your server and it reads the service registry to route requests.

```
sortie svc register production my-api 8080
sortie proxy install production --port 80
```

The proxy matches the Host header against registered service names. If your server IP is 1.2.3.4:

```
curl -H "Host: my-api" http://1.2.3.4/
```

No domain needed. The service name you registered is the hostname clients send. You can use any string.

To add TLS, run `sortie tls setup` with a real domain first (Let's Encrypt), then point DNS at the server.

### Canary and blue-green

Roll out to a subset of hosts before full deploy:

```
sortie apply production --canary 20
```

Or deploy a full parallel stack and flip:

```
sortie apply production --blue-green
```

### Deploy to all targets

```
sortie deploy all
```

Iterates every target in sortie.toml. Also works with --canary and --blue-green.

## Install

```
cargo install sortie
```

Or build from source:

```
git clone <repo-url>
cd sortie
cargo build --release
```

Requires Rust 1.70+, Linux servers with SSH + systemd, key-based auth.

## Commands

| Command | Like kubectl | Action |
|---------|-------------|--------|
| `sortie init` | | Create a sortie.toml |
| `sortie apply <target>` | kubectl apply | Rolling deploy across all hosts. `--check` for dry-run, `--canary`/`--blue-green` for strategies |
| `sortie deploy <target>` | | Same as apply, with `--all` for federation |
| `sortie get` | kubectl get | Show all targets with version, timestamp, services |
| `sortie describe <target>` | kubectl describe | Target config + per-host status |
| `sortie logs <target> [host]` | kubectl logs | Fetch service logs from a host |
| `sortie rollback <target>` | kubectl rollout undo | Revert to previous binary |
| `sortie status <target>` | kubectl get pods | Check service status per host |
| `sortie health <target>` | | Check host connectivity and service health |
| `sortie svc register <target> <name> <port>` | | Register a service in the cluster |
| `sortie svc list <target>` | | List all registered services |
| `sortie svc resolve <target> <name>` | | Resolve a service name to host:port |
| `sortie svc restart <host> <target> <name>` | kubectl rollout restart | Restart a service on a host |
| `sortie svc stop <host> <target> <name>` | | Stop a service on a host |
| `sortie svc start <host> <target> <name>` | | Start a service on a host |
| `sortie proxy install <target>` | | Install embedded reverse proxy as a systemd service |
| `sortie scale <target> <n>` | kubectl scale | Set systemd instance count per host |
| `sortie autoscale <target>` | | Start CPU-based auto-scaling loop |
| `sortie secret set <target> <key> <value>` | | Store a secret file on the server |
| `sortie secret get <target> <key>` | | Read a stored secret |
| `sortie secret rm <target> <key>` | | Delete a stored secret |
| `sortie tls setup <target> <domain> <email>` | | Provision Let's Encrypt certs via certbot |
| `sortie ingress <target>` | | Generate nginx config from service registry |
| `sortie metrics <target>` | | Show CPU and memory per host |

## Configuration

```toml
[targets.production]
hosts = ["10.0.0.1", "10.0.0.2"]
port = 22
user = "deploy"
key_path = "~/.ssh/id_rsa"
target_triple = "x86_64-unknown-linux-gnu"
deploy_path = "/opt/myapp"
health_check_url = "http://localhost:8080/health"
health_check_timeout_secs = 30
pre_deploy = "cd /opt/myapp && ./run_migrations.sh"
post_deploy = "echo 'Deployment completed'"

[targets.production.service]
name = "myapp"
restart = "always"

[targets.production.env]
DATABASE_URL = "postgres://localhost:5432/myapp"
RUST_LOG = "info"
```

### Configuration reference

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `hosts` | yes | | Server addresses in the cluster |
| `port` | no | 22 | SSH port |
| `user` | yes | | SSH user |
| `key_path` | no | `~/.ssh/id_rsa` | SSH key path |
| `target_triple` | yes | | Rust target triple |
| `deploy_path` | yes | | Server directory for binary |
| `health_check_url` | no | | URL to check after deploy |
| `health_check_timeout_secs` | no | 30 | Max wait for health |
| `instances` | no | 1 | Expected replicas per host |
| `pre_deploy` | no | | Command to run on each host before the new binary takes traffic |
| `post_deploy` | no | | Command to run on each host after the deploy succeeds |
| `service.name` | yes | | systemd service name |
| `service.restart` | no | `always` | Restart policy |
| `env.*` | no | | Environment variables |

## How it works

Reads desired state from `sortie.toml`. Builds the binary. Performs a rolling deployment across all hosts: upload via SCP, install systemd unit with env vars, verify health, next host. If a health check fails, that host rolls back and the deploy stops.

Previous versions are kept as `.bak` files for instant rollback. Cluster state is stored in `.sortie/state.json` on each host.

## Architecture

| | |
|---|---|
| <img src="docs/architecture/images/c4-context--0.svg" alt="Context" width="340"> | <img src="docs/architecture/images/c4-containers--0.svg" alt="Containers" width="340"> |
| <img src="docs/architecture/images/c4-components--0.svg" alt="Components" width="340"> | <img src="docs/architecture/images/c4-deployment--0.svg" alt="Deployment" width="340"> |
| <img src="docs/architecture/images/c4-dynamic-build--0.svg" alt="Build" width="340"> | <img src="docs/architecture/images/build-lifecycle--0.svg" alt="Lifecycle" width="340"> |

## Project structure

```
sortie/
  Cargo.toml
  Cargo.lock
  LICENSE
  README.md
  CHANGELOG.md
  CONTRIBUTING.md
  CODE_OF_CONDUCT.md
  SECURITY.md
  SECURITY_CONTACTS
  SUPPORT.md
  OWNERS
  OWNERS_ALIASES
  .gitattributes
  .gitignore
  rust-toolchain.toml
  Makefile
  cmd/
    sortie/
      main.rs
    agent/
      main.rs
  pkg/
    lib.rs
    cli.rs
    types.rs
    config.rs
    init.rs
    build.rs
    ssh.rs
    cluster.rs
    deploy.rs
    systemd.rs
    health.rs
    rollback.rs
    proxy.rs
  logo/
    sortie.png
  build/
    build-release.bash
  hack/
    ci.bash
  tests/
    smoke_test.rs
  docs/
    architecture/
      images/
        c4-context--0.svg
        c4-dynamic-build--0.svg
        build-lifecycle--0.svg
        c4-containers--0.svg
        c4-components--0.svg
        c4-deployment--0.svg
  LICENSES/
    README.md
  examples/
    basic.toml
    multi-host.toml
    with-env.toml
  third_party/
    README.md
  CHANGELOG/
    v0.1.0.md
    v0.2.0.md
```

## Contributing

Pull requests welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sortie")]
#[command(about = "Kubernetes for Rust — deploy, manage, and monitor Rust services across clusters")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Create a sortie.toml to get started
    Init,
    /// Deploy to a target (builds, uploads, installs, health-checks). Applies to all hosts in the target.
    Deploy {
        /// Target name, or "all" for federation deploy to every target
        target: String,
        /// Deploy to a subset of hosts (percent 1-100)
        #[arg(long)]
        canary: Option<u32>,
        /// Parallel blue/green deploy with symlink swap
        #[arg(long)]
        blue_green: bool,
        /// Dry-run: show version diff without deploying
        #[arg(long)]
        check: bool,
    },
    /// Idempotent deploy — same as deploy, mirrors desired state from sortie.toml
    Apply {
        target: String,
        #[arg(long)]
        canary: Option<u32>,
        #[arg(long)]
        blue_green: bool,
        /// Dry-run: show version diff without deploying
        #[arg(long)]
        check: bool,
    },
    /// Show all targets and their status across hosts (like kubectl get)
    Get,
    /// Detailed status of a target and its hosts (like kubectl describe)
    Describe {
        target: String,
    },
    /// Fetch logs from a target's service on a specific host
    Logs {
        target: String,
        /// Host address to fetch logs from (defaults to first host in the target)
        host: Option<String>,
        /// Number of log lines to show
        #[arg(short = 'n', default_value = "50")]
        lines: u32,
    },
    /// Revert to the last working binary
    Rollback {
        target: Option<String>,
    },
    /// Check if the service is running on a target
    Status {
        target: Option<String>,
    },
    /// Check host connectivity and service health across a target
    Health {
        target: String,
    },
    /// Register or list services in the cluster
    Svc {
        #[command(subcommand)]
        action: SvcAction,
    },
    /// Generate nginx config from the service registry
    Ingress {
        target: String,
    },
    /// Set instance count for a target
    Scale {
        target: String,
        instances: u32,
    },
    /// Auto-scale a target based on CPU load
    Autoscale {
        target: String,
        /// Minimum number of instances
        #[arg(long, default_value = "1")]
        min: u32,
        /// Maximum number of instances
        #[arg(long, default_value = "10")]
        max: u32,
    },
    /// Manage secrets (encrypted key-value store on servers)
    Secret {
        #[command(subcommand)]
        action: SecretAction,
    },
    /// Show CPU/memory metrics for a target
    Metrics {
        target: String,
    },
    /// Provision TLS certificate via certbot
    Tls {
        target: String,
        domain: String,
        email: String,
    },
    /// Install and manage the embedded reverse proxy
    Proxy {
        #[command(subcommand)]
        action: ProxyAction,
    },
}

#[derive(Subcommand)]
pub enum ProxyAction {
    /// Install the proxy as a systemd service on the target
    Install {
        target: String,
        /// Port for the proxy to listen on
        #[arg(long, default_value = "80")]
        port: u16,
    },
}

#[derive(Subcommand)]
pub enum SvcAction {
    /// Register a service in the cluster state
    Register {
        target: String,
        name: String,
        port: u16,
    },
    /// List registered services
    List {
        target: String,
    },
    /// Resolve a service name to host:port (DNS-light)
    Resolve {
        target: String,
        name: String,
    },
    /// Restart a service on a specific host
    Restart {
        host: String,
        target: String,
        name: String,
    },
    /// Stop a service on a specific host
    Stop {
        host: String,
        target: String,
        name: String,
    },
    /// Start a service on a specific host
    Start {
        host: String,
        target: String,
        name: String,
    },
}

#[derive(Subcommand)]
pub enum SecretAction {
    /// Set a secret (stored encrypted on servers)
    Set {
        target: String,
        key: String,
        value: String,
    },
    /// Get a secret value
    Get {
        target: String,
        key: String,
    },
    /// Remove a secret
    Rm {
        target: String,
        key: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_init() {
        let cli = Cli::try_parse_from(["sortie", "init"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        match cli.command {
            Commands::Init => {} // correct
            _ => panic!("expected Init command"),
        }
    }

    #[test]
    fn test_cli_deploy() {
        let cli = Cli::try_parse_from(["sortie", "deploy", "production"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_apply() {
        let cli = Cli::try_parse_from(["sortie", "apply", "staging"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_get() {
        let cli = Cli::try_parse_from(["sortie", "get"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_describe() {
        let cli = Cli::try_parse_from(["sortie", "describe", "production"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_logs() {
        let cli = Cli::try_parse_from(["sortie", "logs", "production"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_logs_with_host_and_lines() {
        let cli = Cli::try_parse_from(["sortie", "logs", "production", "10.0.0.1", "-n", "100"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_rollback() {
        let cli = Cli::try_parse_from(["sortie", "rollback", "production"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_status() {
        let cli = Cli::try_parse_from(["sortie", "status", "production"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_health() {
        let cli = Cli::try_parse_from(["sortie", "health", "production"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_no_subcommand_fails() {
        let cli = Cli::try_parse_from(["sortie"]);
        assert!(cli.is_err());
    }
}

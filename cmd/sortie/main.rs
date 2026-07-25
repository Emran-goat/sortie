use std::process;

use clap::Parser;

use sortie::cli::{Cli, Commands, ProxyAction, SecretAction, SvcAction};
use sortie::config;
use sortie::types::Config;

fn load_config_or_exit() -> Config {
    match config::load_config(None) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}

fn get_target_or_exit<'a>(config: &'a Config, name: &str) -> &'a sortie::types::TargetConfig {
    match config.targets.get(name) {
        Some(t) => t,
        None => {
            eprintln!("No target named '{}' in sortie.toml", name);
            process::exit(1);
        }
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init => sortie::init::init_project(),

        Commands::Deploy { target, canary, blue_green, check } | Commands::Apply { target, canary, blue_green, check } => {
            if target == "all" {
                let config = load_config_or_exit();
                return sortie::cluster::deploy_all_targets(&config, *canary, *blue_green);
            }
            let config = load_config_or_exit();
            let tc = get_target_or_exit(&config, target);
            if *check {
                return sortie::cluster::check_apply(target, tc);
            }
            if *blue_green {
                sortie::cluster::blue_green_deploy(target, tc)
            } else if let Some(pct) = canary {
                sortie::cluster::canary_deploy(target, tc, *pct)
            } else {
                sortie::cluster::rolling_deploy(target, tc)
            }
        }

        Commands::Get => {
            let config = load_config_or_exit();
            for (name, tc) in &config.targets {
                println!("Target: {}", name);
                sortie::cluster::get_target_state(name, tc)?;
                println!();
            }
            Ok(())
        }

        Commands::Describe { target } => {
            let config = load_config_or_exit();
            let tc = get_target_or_exit(&config, target);
            let hosts = tc.get_hosts();
            println!("Target:       {}", target);
            println!("Hosts:        {}", hosts.join(", "));
            println!("User:         {}", tc.user);
            println!("Port:         {}", tc.port.unwrap_or(22));
            println!("Target triple: {}", tc.target_triple);
            println!("Deploy path:  {}", tc.deploy_path);
            println!("Instances:    {}", tc.instances.unwrap_or(1));
            if let Some(ref svc) = tc.service {
                println!("Service:      {}", svc.name);
                println!("Restart:      {}", svc.restart.as_deref().unwrap_or("always"));
            }
            if let Some(ref url) = tc.health_check_url {
                println!("Health URL:   {}", url);
                println!("Timeout:      {}s", tc.health_check_timeout_secs.unwrap_or(30));
            }
            println!();
            for host in &hosts {
                match sortie::cluster::get_host_status(host, tc) {
                    Ok(true) => println!("  {}: Running", host),
                    Ok(false) => println!("  {}: Stopped", host),
                    Err(e) => println!("  {}: Error - {}", host, e),
                }
            }
            Ok(())
        }

        Commands::Logs { target, host, lines } => {
            let config = load_config_or_exit();
            let tc = get_target_or_exit(&config, target);
            let hosts = tc.get_hosts();
            let host = host.clone().unwrap_or_else(|| {
                hosts.first().cloned().unwrap_or_else(|| "?".to_string())
            });
            let logs = sortie::cluster::fetch_logs(&host, tc, *lines)?;
            print!("{}", logs);
            Ok(())
        }

        Commands::Health { target } => {
            let config = load_config_or_exit();
            let tc = get_target_or_exit(&config, target);
            let hosts = tc.get_hosts();
            println!("Target: {}", target);
            for host in &hosts {
                match sortie::cluster::check_host(host, tc) {
                    Ok(msg) => println!("  {}", msg),
                    Err(e) => println!("  {}: Unreachable - {}", host, e),
                }
            }
            Ok(())
        }

        Commands::Rollback { target } => {
            let target = target.as_deref().unwrap_or("production");
            let config = load_config_or_exit();
            let tc = get_target_or_exit(&config, target);
            let hosts = tc.get_hosts();
            for host in &hosts {
                println!("Rolling back on {}...", host);
                let session = sortie::ssh::connect(host, tc.port.unwrap_or(22), &tc.user, tc.key_path.as_deref())?;
                sortie::rollback::rollback_host(target, tc, &session)?;
            }
            Ok(())
        }

        Commands::Status { target } => {
            let target = target.as_deref().unwrap_or("production");
            let config = load_config_or_exit();
            let tc = get_target_or_exit(&config, target);
            let hosts = tc.get_hosts();
            for host in &hosts {
                let session = sortie::ssh::connect(host, tc.port.unwrap_or(22), &tc.user, tc.key_path.as_deref())?;
                match &tc.service {
                    Some(svc) => {
                        let active = sortie::systemd::service_status(&session, &svc.name)?;
                        println!("{} on {}: {}", svc.name, host, if active { "Running" } else { "Stopped" });
                    }
                    None => println!("{}: No service configured", host),
                }
            }
            Ok(())
        }

        Commands::Svc { action } => {
            let config = load_config_or_exit();
            match action {
                SvcAction::Register { target, name, port } => {
                    let tc = get_target_or_exit(&config, target);
                    sortie::cluster::register_service(target, tc, name, *port)
                }
                SvcAction::List { target } => {
                    let tc = get_target_or_exit(&config, target);
                    sortie::cluster::list_services(target, tc)
                }
                SvcAction::Resolve { target, name } => {
                    let tc = get_target_or_exit(&config, target);
                    sortie::cluster::resolve_service(target, tc, name)
                }
                SvcAction::Restart { host, target, name } => {
                    let tc = get_target_or_exit(&config, target);
                    sortie::cluster::restart_svc(host, tc, name)
                }
                SvcAction::Stop { host, target, name } => {
                    let tc = get_target_or_exit(&config, target);
                    sortie::cluster::stop_svc(host, tc, name)
                }
                SvcAction::Start { host, target, name } => {
                    let tc = get_target_or_exit(&config, target);
                    sortie::cluster::start_svc(host, tc, name)
                }
            }
        }

        Commands::Ingress { target } => {
            let config = load_config_or_exit();
            let tc = get_target_or_exit(&config, target);
            sortie::cluster::setup_ingress(target, tc)
        }

        Commands::Scale { target, instances } => {
            let config = load_config_or_exit();
            let tc = get_target_or_exit(&config, target);
            sortie::cluster::scale_target(target, tc, *instances)
        }

        Commands::Autoscale { target, min, max } => {
            let config = load_config_or_exit();
            let tc = get_target_or_exit(&config, target);
            sortie::cluster::autoscale_loop(target, tc, *min, *max)
        }

        Commands::Secret { action } => {
            let config = load_config_or_exit();
            match action {
                SecretAction::Set { target, key, value } => {
                    let tc = get_target_or_exit(&config, target);
                    sortie::cluster::set_secret(target, tc, key, value)
                }
                SecretAction::Get { target, key } => {
                    let tc = get_target_or_exit(&config, target);
                    sortie::cluster::get_secret(target, tc, key)
                }
                SecretAction::Rm { target, key } => {
                    let tc = get_target_or_exit(&config, target);
                    sortie::cluster::rm_secret(target, tc, key)
                }
            }
        }

        Commands::Metrics { target } => {
            let config = load_config_or_exit();
            let tc = get_target_or_exit(&config, target);
            sortie::cluster::get_metrics(target, tc)
        }

        Commands::Tls { target, domain, email } => {
            let config = load_config_or_exit();
            let tc = get_target_or_exit(&config, target);
            sortie::cluster::setup_tls(target, tc, domain, email)
        }

        Commands::Proxy { action } => {
            let config = load_config_or_exit();
            match action {
                ProxyAction::Install { target, port } => {
                    let tc = get_target_or_exit(&config, target);
                    sortie::cluster::install_proxy(target, tc, *port)
                }
            }
        }
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

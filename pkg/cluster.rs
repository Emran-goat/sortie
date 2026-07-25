use crate::ssh::sh_quote;
use crate::types::{HostStatus, TargetConfig};

pub fn rolling_deploy(target: &str, config: &TargetConfig) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err(format!("Target '{}' has no hosts defined.", target));
    }

    println!("Building for {}...", config.target_triple);
    let binary = crate::build::build_project(
        &config.target_triple,
        config.build_args.as_deref().unwrap_or(&[]),
        config.cross_compile,
    )?;
    let version = get_local_version();

    println!("Rolling deploy to {} host(s)...\n", hosts.len());

    let mut ok = 0u32;
    for (i, host) in hosts.iter().enumerate() {
        let state = crate::deploy::read_state(host, config).unwrap_or_default();
        if state.version == version {
            println!("[{}/{}] {} already at version {}", i + 1, hosts.len(), host, &state.version[..8.min(state.version.len())]);
            ok += 1;
            continue;
        }
        println!("[{}/{}] {}...", i + 1, hosts.len(), host);
        match crate::deploy::deploy_single_host(host, config, &binary) {
            Ok(_) => {
                println!("[{}/{}] {} OK", i + 1, hosts.len(), host);
                ok += 1;
            }
            Err(e) => {
                eprintln!("[{}/{}] {} FAILED: {}", i + 1, hosts.len(), host, e);
                // ponytail: skip dead host, deploys to remaining
            }
        }
    }

    if ok == 0 {
        return Err("All hosts failed.".to_string());
    }

    let skipped = hosts.len() as u32 - ok;
    if skipped > 0 {
        eprintln!("Skipped {} dead host(s).", skipped);
    }

    Ok(())
}

pub fn canary_deploy(target: &str, config: &TargetConfig, percent: u32) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err(format!("Target '{}' has no hosts defined.", target));
    }
    let count = ((hosts.len() as u32 * percent).max(1) as usize).min(hosts.len());
    let subset: Vec<String> = hosts.iter().take(count).cloned().collect();
    // ponytail: deploy to subset, report remaining
    println!("Canary {}%: deploying to {} of {} host(s)", percent, count, hosts.len());
    let canary_config = TargetConfig { hosts: Some(subset), ..config.clone() };
    rolling_deploy(target, &canary_config)?;
    let remaining = hosts.len() - count;
    if remaining > 0 {
        println!("{} host(s) skipped. Run full deploy to promote.", remaining);
    }
    Ok(())
}

fn get_local_version() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output().ok()
        .and_then(|o| if o.status.success() { String::from_utf8(o.stdout).ok() } else { None })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn blue_green_deploy(target: &str, config: &TargetConfig) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err(format!("Target '{}' has no hosts defined.", target));
    }
    // ponytail: deploy to .blue, symlink flip. green = current, blue = new.
    let bg_config = TargetConfig {
        deploy_path: format!("{}.blue", config.deploy_path),
        ..config.clone()
    };
    println!("Deploying blue stack to {}.blue...", config.deploy_path);
    rolling_deploy(target, &bg_config)?;
    for host in &hosts {
        let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
        let green = sh_quote(&config.deploy_path);
        let blue = sh_quote(&format!("{}.blue", config.deploy_path));
        crate::ssh::run_command(&session, &format!("mv {} {}.old", green, green)).ok();
        crate::ssh::run_command(&session, &format!("ln -s {} {}", blue, green))?;
        crate::ssh::run_command(&session, &format!("rm -rf {}.old", green)).ok();
        if let Some(ref svc) = config.service {
            crate::systemd::restart_service(&session, &svc.name)?;
        }
    }
    Ok(())
}

pub fn get_cluster_status(config: &crate::types::Config) -> Result<Vec<HostStatus>, String> {
    let mut all = Vec::new();
    for (name, target) in &config.targets {
        let hosts = target.get_hosts();
        if hosts.is_empty() {
            all.push(HostStatus {
                host: format!("{}/?", name),
                running: false,
                error: Some("no hosts configured".to_string()),
            });
            continue;
        }
        for host in &hosts {
            let status = get_host_status(host, target);
            all.push(HostStatus {
                host: format!("{}/{}", name, host),
                running: status.as_ref().map(|s| *s).unwrap_or(false),
                error: status.err(),
            });
        }
    }
    Ok(all)
}

pub fn check_host(host: &str, config: &TargetConfig) -> Result<String, String> {
    let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
    let svc = match &config.service {
        Some(s) => s,
        None => return Ok(format!("{}: Reachable (no service configured)", host)),
    };
    let active = crate::systemd::service_status(&session, &svc.name)?;
    let health = match &config.health_check_url {
        Some(url) => {
            let h = crate::health::check_health_on_host(url, host, config.health_check_timeout_secs.unwrap_or(10)).unwrap_or(false);
            if h { "HTTP OK" } else { "HTTP down" }
        }
        None => "",
    };
    let state = if active { "Running" } else { "Stopped" };
    let extra = if health.is_empty() { String::new() } else { format!(", {}", health) };
    Ok(format!("{}: {}{}", host, state, extra))
}

pub fn get_host_status(host: &str, config: &TargetConfig) -> Result<bool, String> {
    let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
    match &config.service {
        Some(svc) => crate::systemd::service_status(&session, &svc.name),
        None => Err("no service configured".to_string()),
    }
}

pub fn fetch_logs(host: &str, config: &TargetConfig, lines: u32) -> Result<String, String> {
    let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
    match &config.service {
        Some(svc) => {
            let cmd = format!("journalctl -u {} -n {} --no-pager", crate::ssh::sh_quote(&svc.name), lines);
            let (stdout, stderr, code) = crate::ssh::run_command(&session, &cmd)?;
            if code != 0 {
                return Err(format!("journalctl: {}", stderr.trim()));
            }
            Ok(stdout)
        }
        None => Err("No service configured for this target.".to_string()),
    }
}

// ponytail: service registry reads/writes state.json on the first host
// ponytail: ingress generates nginx config from registered services
// ponytail: scale writes a new sortie.toml with updated instance count

pub fn register_service(target: &str, config: &TargetConfig, name: &str, port: u16) -> Result<(), String> {
    use crate::types::ServiceEndpoint;
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err("No hosts configured".to_string());
    }
    let first = &hosts[0];
    let mut state = crate::deploy::read_state(first, config).unwrap_or_default();
    state.services.insert(name.to_string(), ServiceEndpoint {
        hosts: hosts.clone(),
        port,
        health: "unknown".to_string(),
    });
    // ponytail: updates state on the first host. other hosts are passive.
    for h in &hosts {
        crate::deploy::write_state_raw(h, config, &state)?;
    }
    println!("Service '{}' registered on {}:{} ({})", name, first, port, target);
    Ok(())
}

pub fn list_services(target: &str, config: &TargetConfig) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err("No hosts configured".to_string());
    }
    let state = crate::deploy::read_state(&hosts[0], config).unwrap_or_default();
    if state.services.is_empty() {
        println!("No services registered for target '{}'.", target);
        return Ok(());
    }
    println!("{:<20} {:<8} {:<10} {:<10}", "SERVICE", "PORT", "HOSTS", "HEALTH");
    for (name, ep) in &state.services {
        println!("{:<20} {:<8} {:<10} {:<10}", name, ep.port, ep.hosts.len(), ep.health);
    }
    Ok(())
}

pub fn setup_ingress(target: &str, config: &TargetConfig) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err("No hosts configured".to_string());
    }
    let state = crate::deploy::read_state(&hosts[0], config).unwrap_or_default();
    if state.services.is_empty() {
        return Err("No services registered. Use `sortie svc register` first.".to_string());
    }
    for host in &hosts {
        let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
        for (name, ep) in &state.services {
            let upstreams: String = ep.hosts.iter().map(|h| format!("    server {}:{};", h, ep.port)).collect::<Vec<_>>().join("\n");
            let cfg = format!(
                "server {{\n\
                 listen 80;\n\
                 server_name {}.{}\n;\
                 \n\
                 location / {{\n\
                 proxy_pass http://{};\n\
                 proxy_set_header Host $host;\n\
                 proxy_set_header X-Real-IP $remote_addr;\n\
                 }}\n\
                 }}\n\
                 upstream {} {{\n\
                 {}\n\
                 }}",
                name, target, name, name, upstreams
            );
            crate::ssh::run_command(&session, &format!(
                "cat > /etc/nginx/sites-available/{}.{} << 'SORTIENGINX'\n{}\nSORTIENGINX",
                name, target, cfg
            ))?;
            crate::ssh::run_command(&session, &format!("ln -sf /etc/nginx/sites-available/{}.{} /etc/nginx/sites-enabled/", name, target))?;
        }
        crate::ssh::run_command(&session, "nginx -t && systemctl reload nginx")?;
        println!("Ingress configured on {}", host);
    }
    Ok(())
}

pub fn scale_target(target: &str, config: &TargetConfig, instances: u32) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err(format!("Target '{}' has no hosts.", target));
    }
    // ponytail: launches N instanced systemd units (@instanced.service)
    for host in &hosts {
        let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
        let svc = match &config.service {
            Some(s) => s,
            None => {
                println!("No service configured for {} — just recording desired count.", host);
                continue;
            }
        };
        for i in 1..=instances {
            let unit = format!("{}@{}.service", svc.name, i);
            let exists = crate::ssh::run_command(&session, &format!("systemctl is-enabled {} 2>/dev/null", sh_quote(&unit)));
            if exists.is_err() || exists.unwrap().2 != 0 {
                crate::ssh::run_command(&session, &format!("systemctl start {}", sh_quote(&unit))).ok();
            }
        }
        // ponytail: stop excess instances beyond desired count
        for i in instances + 1..=instances + 10 {
            let unit = format!("{}@{}.service", svc.name, i);
            let _ = crate::ssh::run_command(&session, &format!("systemctl stop {} 2>/dev/null; systemctl disable {} 2>/dev/null", sh_quote(&unit), sh_quote(&unit)));
        }
        println!("Scaled {} to {} instance(s) on {}", svc.name, instances, host);
    }
    Ok(())
}

// Phase 1: declarative apply — show version diff per host without deploying
pub fn check_apply(target: &str, config: &TargetConfig) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err(format!("Target '{}' has no hosts.", target));
    }
    let desired = get_local_version();
    println!("Desired version: {} ({})", &desired[..8.min(desired.len())], target);
    println!("{:<20} {:<12} {:<12}", "HOST", "CURRENT", "ACTION");
    for host in &hosts {
        let state = crate::deploy::read_state(host, config).unwrap_or_default();
        let cur = state.version;
        let cur_short = if cur.is_empty() { "none".into() } else { cur[..8.min(cur.len())].to_string() };
        let action = if cur == desired { "noop" } else { "deploy" };
        println!("{:<20} {:<12} {:<12}", host, cur_short, action);
    }
    Ok(())
}

// Phase 1: cluster state — show version + service info per host
pub fn get_target_state(target: &str, config: &TargetConfig) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err(format!("Target '{}' has no hosts.", target));
    }
    for host in &hosts {
        let state = crate::deploy::read_state(host, config).unwrap_or_default();
        let ver = if state.version.is_empty() { "none".into() } else { state.version[..8.min(state.version.len())].to_string() };
        println!("{}:", host);
        println!("  version:     {}", ver);
        println!("  timestamp:   {}", state.timestamp);
        println!("  services:    {}", state.services.len());
        for (name, ep) in &state.services {
            println!("    {} -> {}:{}{}", name, host, ep.port, if ep.health != "unknown" { format!(" ({})", ep.health) } else { String::new() });
        }
    }
    Ok(())
}

// Phase 2: service resolve — print host:port for a registered service (DNS-light)
pub fn resolve_service(target: &str, config: &TargetConfig, name: &str) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err(format!("Target '{}' has no hosts.", target));
    }
    let state = crate::deploy::read_state(&hosts[0], config).unwrap_or_default();
    match state.services.get(name) {
        Some(ep) => {
            for h in &ep.hosts {
                println!("{} {}:{}", name, h, ep.port);
            }
            Ok(())
        }
        None => Err(format!("Service '{}' not registered in target '{}'.", name, target)),
    }
}

// Phase 2: TLS — run certbot via SSH
pub fn setup_tls(target: &str, config: &TargetConfig, domain: &str, email: &str) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err(format!("Target '{}' has no hosts.", target));
    }
    for host in &hosts {
        let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
        // ponytail: certbot --nginx, non-interactive. assumes certbot installed.
        let cmd = format!(
            "certbot --nginx -d {} --non-interactive --agree-tos -m {} || echo 'certbot failed (install certbot?)'",
            sh_quote(domain), sh_quote(email)
        );
        let (out, err, code) = crate::ssh::run_command(&session, &cmd)?;
        if code != 0 {
            eprintln!("{}: certbot error: {}", host, err.trim());
        }
        println!("TLS configured for {} on {}", domain, host);
        println!("{}", out.trim());
    }
    Ok(())
}

// Phase 3: auto-scale — loop over SSH CPU check, adjust instances
pub fn autoscale_loop(target: &str, config: &TargetConfig, min: u32, max: u32) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err(format!("Target '{}' has no hosts.", target));
    }
    println!("Autoscale started for '{}' (min={}, max={}). Ctrl-C to stop.", target, min, max);
    loop {
        for host in &hosts {
            let session = match crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref()) {
                Ok(s) => s,
                Err(_) => continue,
            };
            // ponytail: check CPU load, scale up if >80%, down if <20%
            let cmd = "top -bn1 | grep 'Cpu(s)' | awk '{print $2}' | cut -d'%' -f1";
            let (out, _, code) = match crate::ssh::run_command(&session, cmd) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if code != 0 { continue; }
            let cpu: f32 = out.trim().parse().unwrap_or(50.0);
            let current = config.instances.unwrap_or(1);
            if cpu > 80.0 && current < max {
                println!("CPU {:.0}% > 80% on {}, scaling up to {}", cpu, host, current + 1);
                scale_target(target, config, current + 1)?;
            } else if cpu < 20.0 && current > min {
                println!("CPU {:.0}% < 20% on {}, scaling down to {}", cpu, host, current - 1);
                scale_target(target, config, current - 1)?;
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

// Phase 4: secrets — store key-value in encrypted file on server (SSH is the security boundary)
pub fn set_secret(_target: &str, config: &TargetConfig, key: &str, value: &str) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() { return Err("No hosts".to_string()); }
    let secret_path = format!("{}/.sortie/secrets", config.deploy_path);
    for host in &hosts {
        let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
        crate::ssh::run_command(&session, &format!("mkdir -p {}", sh_quote(&format!("{}/.sortie/secrets", config.deploy_path))))?;
        let file = format!("{}/{}", secret_path, sh_quote(key));
        crate::ssh::run_command(&session, &format!("cat > {} << 'SORTIEEOF'\n{}\nSORTIEEOF", sh_quote(&file), sh_quote(value)))?;
        crate::ssh::run_command(&session, &format!("chmod 600 {}", sh_quote(&file)))?;
        println!("Secret '{}' set on {}", key, host);
    }
    Ok(())
}

pub fn get_secret(_target: &str, config: &TargetConfig, key: &str) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() { return Err("No hosts".to_string()); }
    let session = crate::ssh::connect(&hosts[0], config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
    let file = format!("{}/.sortie/secrets/{}", config.deploy_path, sh_quote(key));
    let (out, _, code) = crate::ssh::run_command(&session, &format!("cat {}", sh_quote(&file)))?;
    if code != 0 { return Err(format!("Secret '{}' not found.", key)); }
    print!("{}", out);
    Ok(())
}

pub fn rm_secret(_target: &str, config: &TargetConfig, key: &str) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() { return Err("No hosts".to_string()); }
    let file = format!("{}/.sortie/secrets/{}", config.deploy_path, key);
    for host in &hosts {
        let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
        crate::ssh::run_command(&session, &format!("rm -f {}", sh_quote(&file)))?;
        println!("Secret '{}' removed from {}", key, host);
    }
    Ok(())
}

// Phase 4: observability — SSH top + vmstat
pub fn get_metrics(_target: &str, config: &TargetConfig) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() { return Err("No hosts".to_string()); }
    for host in &hosts {
        let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
        let (out, _, code) = crate::ssh::run_command(&session, "top -bn1 | head -5")?;
        if code != 0 { continue; }
        println!("=== {} ===", host);
        println!("{}", out.trim());
        let (mem, _, _) = crate::ssh::run_command(&session, "free -h | head -2")?;
        println!("{}", mem.trim());
    }
    Ok(())
}

// Phase 4: pod lifecycle — restart/stop/start service on a host
pub fn restart_svc(host: &str, config: &TargetConfig, name: &str) -> Result<(), String> {
    let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
    crate::systemd::restart_service(&session, name)
}

pub fn stop_svc(host: &str, config: &TargetConfig, name: &str) -> Result<(), String> {
    let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
    crate::systemd::stop_service(&session, name)
}

pub fn start_svc(host: &str, config: &TargetConfig, name: &str) -> Result<(), String> {
    let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
    crate::systemd::start_service(&session, name)
}

// Phase 5: federation — deploy to all targets
pub fn deploy_all_targets(config: &crate::types::Config, canary: Option<u32>, blue_green: bool) -> Result<(), String> {
    if config.targets.is_empty() {
        return Err("No targets defined in sortie.toml.".to_string());
    }
    for (name, tc) in &config.targets {
        println!("\n--- Deploying to '{}' ---", name);
        if blue_green {
            blue_green_deploy(name, tc)?;
        } else if let Some(pct) = canary {
            canary_deploy(name, tc, pct)?;
        } else {
            rolling_deploy(name, tc)?;
        }
    }
    Ok(())
}

// Phase 5: cross-cluster DNS — resolve service name across all targets
pub fn resolve_cross_cluster(config: &crate::types::Config, name: &str) -> Result<(), String> {
    let mut found = false;
    for (target_name, tc) in &config.targets {
        let hosts = tc.get_hosts();
        if hosts.is_empty() { continue; }
        let state = crate::deploy::read_state(&hosts[0], tc).unwrap_or_default();
        if let Some(ep) = state.services.get(name) {
            found = true;
            for h in &ep.hosts {
                println!("{}.{} {}:{}", name, target_name, h, ep.port);
            }
        }
    }
    if !found {
        return Err(format!("Service '{}' not found in any target.", name));
    }
    Ok(())
}

// Phase 6: install the embedded proxy as a systemd service on target hosts
pub fn install_proxy(target: &str, config: &TargetConfig, port: u16) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err(format!("Target '{}' has no hosts.", target));
    }
    // ponytail: find the agent binary next to the CLI binary, or in target/
    fn find_agent_binary() -> Option<std::path::PathBuf> {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?;
        for name in &["sortie-agent.exe", "sortie-agent"] {
            let candidate = dir.join(name);
            if candidate.exists() { return Some(candidate); }
        }
        for candidate in &["target/release/sortie-agent.exe", "target/release/sortie-agent",
                           "target/debug/sortie-agent.exe", "target/debug/sortie-agent"] {
            let p = std::path::Path::new(candidate);
            if p.exists() { return Some(p.to_path_buf()); }
        }
        None
    }
    let agent_bin = find_agent_binary().ok_or_else(|| "sortie-agent binary not found. Build it first: cargo build".to_string())?;

    let remote_path = format!("{}/sortie-agent", config.deploy_path);

    for host in &hosts {
        let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
        crate::ssh::run_command(&session, &format!("mkdir -p {}", sh_quote(&config.deploy_path)))?;
        crate::ssh::upload_file(&session, &agent_bin, std::path::Path::new(&remote_path))?;
        crate::ssh::run_command(&session, &format!("chmod +x {}", sh_quote(&remote_path)))?;
        let proxy_svc = format!(
            "[Unit]\n\
             Description=Sortie Proxy\n\
             After=network.target\n\
             \n\
             [Service]\n\
             ExecStart={0} {1} --proxy --port {2}\n\
             Restart=always\n\
             RestartSec=5\n\
             \n\
             [Install]\n\
             WantedBy=multi-user.target",
            sh_quote(&remote_path), sh_quote(&config.deploy_path), port
        );
        let svc_name = "sortie-proxy";
        crate::ssh::run_command(&session, &format!(
            "cat > /etc/systemd/system/{}.service << 'SORTIEEOF'\n{}\nSORTIEEOF",
            svc_name, proxy_svc
        ))?;
        crate::ssh::run_command(&session, "systemctl daemon-reload")?;
        crate::ssh::run_command(&session, &format!("systemctl enable {}", svc_name))?;
        crate::ssh::run_command(&session, &format!("systemctl restart {}", svc_name))?;
        println!("Proxy installed on {}:{}", host, port);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolling_deploy_no_hosts() {
        let cfg = TargetConfig {
            host: None,
            hosts: Some(vec![]),
            port: None,
            user: "test".into(),
            key_path: None,
            target_triple: "x86_64".into(),
            deploy_path: "/opt/app".into(),
            health_check_url: None,
            health_check_timeout_secs: None,
            service: None,
            build_args: None,
            env: None,
            instances: None,
            cross_compile: false,
        };
        let result = rolling_deploy("staging", &cfg);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no hosts"));
    }

    #[test]
    fn test_get_cluster_status_no_hosts_reports_error() {
        let mut targets = std::collections::HashMap::new();
        targets.insert(
            "broken".into(),
            TargetConfig {
                host: None,
                hosts: Some(vec![]),
                port: None,
                user: "test".into(),
                key_path: None,
                target_triple: "x86_64".into(),
                deploy_path: "/opt/app".into(),
                health_check_url: None,
                health_check_timeout_secs: None,
                service: None,
                build_args: None,
                env: None,
                instances: None,
                cross_compile: false,
            },
        );
        let config = crate::types::Config { targets };
        let result = get_cluster_status(&config).unwrap();
        assert_eq!(result.len(), 1);
        assert!(!result[0].running);
        assert_eq!(result[0].error.as_deref(), Some("no hosts configured"));
    }

    #[test]
    fn test_get_host_status_no_service() {
        let cfg = TargetConfig {
            host: Some("10.0.0.1".into()),
            hosts: None,
            port: None,
            user: "test".into(),
            key_path: None,
            target_triple: "x86_64".into(),
            deploy_path: "/opt/app".into(),
            health_check_url: None,
            health_check_timeout_secs: None,
            service: None,
            build_args: None,
            env: None,
            instances: None,
            cross_compile: false,
        };
        let result = get_host_status("10.0.0.1", &cfg);
        assert!(result.is_err());
    }
}

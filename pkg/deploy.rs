use crate::ssh::sh_quote;
use crate::types::{HostState, TargetConfig};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::Path;

pub fn deploy_single_host(
    host: &str,
    config: &TargetConfig,
    binary: &Path,
) -> Result<(), String> {
    let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;

    let remote_new = format!("{}/sortie.new", config.deploy_path);
    let remote_cur = format!("{}/sortie", config.deploy_path);
    let remote_bak = format!("{}/sortie.bak", config.deploy_path);
    let q_cur = sh_quote(&remote_cur);
    let q_new = sh_quote(&remote_new);
    let q_bak = sh_quote(&remote_bak);

    crate::ssh::run_command(&session, &format!("mkdir -p {}", sh_quote(&config.deploy_path)))?;

    println!("Uploading to {}...", host);
    crate::ssh::upload_file(&session, binary, Path::new(&remote_new))?;
    crate::ssh::run_command(&session, &format!("chmod +x {}", q_new))?;

    crate::ssh::run_command(&session, &format!("if [ -f {} ]; then cp {} {}; fi", q_cur, q_cur, q_bak))?;
    crate::ssh::run_command(&session, &format!("mv {} {}", q_new, q_cur))?;

    if let Some(ref svc) = config.service {
        let _ = crate::systemd::stop_service(&session, &svc.name);
        let svc_content = crate::systemd::generate_service(svc, &remote_cur, config.env.as_ref());
        crate::systemd::install_service(&session, svc, &svc_content)?;
        crate::systemd::start_service(&session, &svc.name)?;
    }

    if let Some(ref url) = config.health_check_url {
        let timeout = config.health_check_timeout_secs.unwrap_or(30);
        print!("  Waiting for health check...");
        std::io::Write::flush(&mut std::io::stdout()).ok();
        if !crate::health::check_health_on_host(url, host, timeout)? {
            println!(" FAILED");
            crate::ssh::run_command(&session, &format!("if [ -f {} ]; then mv {} {}; fi", q_bak, q_bak, q_cur))?;
            if let Some(ref svc) = config.service {
                crate::systemd::start_service(&session, &svc.name)?;
            }
            return Err(format!("Health check failed on {}", host));
        }
        println!(" OK");
    }

    write_state(&session, config, binary)?;
    Ok(())
}

fn write_state(session: &ssh2::Session, config: &TargetConfig, binary: &Path) -> Result<(), String> {
    let dir = format!("{}/.sortie", config.deploy_path);
    let path = format!("{}/state.json", dir);
    crate::ssh::run_command(session, &format!("mkdir -p {}", sh_quote(&dir)))?;
    let existing = match crate::ssh::run_command(session, &format!("cat {}", sh_quote(&path))) {
        Ok((out, _, 0)) => serde_json::from_str(&out).unwrap_or_default(),
        _ => HostState::default(),
    };
    let mut s = existing;
    s.version = get_current_commit();
    s.binary_hash = hash_binary(binary)?;
    s.timestamp = format!("{:?}", std::time::SystemTime::now());
    // ponytail: keeps services registry from existing state
    crate::ssh::run_command(session, &format!(
        "cat > {} << 'SORTIEEOF'\n{}\nSORTIEEOF",
        sh_quote(&path),
        serde_json::to_string_pretty(&s).unwrap(),
    ))?;
    Ok(())
}

pub fn read_state(host: &str, config: &TargetConfig) -> Result<HostState, String> {
    let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
    let path = format!("{}/.sortie/state.json", config.deploy_path);
    match crate::ssh::run_command(&session, &format!("cat {}", sh_quote(&path))) {
        Ok((out, _, 0)) => serde_json::from_str(&out).map_err(|e| format!("Bad state: {}", e)),
        _ => Ok(HostState::default()),
    }
}

pub fn write_state_raw(host: &str, config: &TargetConfig, state: &HostState) -> Result<(), String> {
    let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
    let dir = format!("{}/.sortie", config.deploy_path);
    let path = format!("{}/state.json", dir);
    crate::ssh::run_command(&session, &format!("mkdir -p {}", sh_quote(&dir)))?;
    crate::ssh::run_command(&session, &format!(
        "cat > {} << 'SORTIEEOF'\n{}\nSORTIEEOF",
        sh_quote(&path),
        serde_json::to_string_pretty(state).unwrap(),
    ))?;
    Ok(())
}

fn get_current_commit() -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { String::from_utf8(o.stdout).ok() } else { None })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn hash_binary(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| format!("Can't open binary: {}", e))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(|e| format!("Can't read binary: {}", e))?;
    let mut hasher = DefaultHasher::new();
    buf.hash(&mut hasher);
    Ok(format!("{:016x}", hasher.finish()))
}

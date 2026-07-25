use crate::types::ServiceConfig;
use crate::ssh::sh_quote;
use ssh2::Session;
use std::io::Write;
use std::path::Path;

use std::collections::HashMap;

pub fn generate_service(service: &ServiceConfig, binary_path: &str, env: Option<&HashMap<String, String>>) -> String {
    let extra = service.extra_args.as_ref()
        .map(|a| a.join(" "))
        .unwrap_or_default();
    let restart = service.restart.as_deref().unwrap_or("always");

    let env_block = env.map(|vars| {
        vars.iter()
            .map(|(k, v)| format!("Environment={}={}", k, v))
            .collect::<Vec<_>>()
            .join("\n")
    }).unwrap_or_default();

    let env_section = if env_block.is_empty() { String::new() } else { format!("{}\n", env_block) };

    format!(
        "[Unit]\n\
         Description={name}\n\
         After=network.target\n\
         \n\
         [Service]\n\
         {env}ExecStart={path} {extra}\n\
         Restart={restart}\n\
         RestartSec=5\n\
         PrivateTmp=true\n\
         NoNewPrivileges=true\n\
         ProtectSystem=full\n\
         \n\
         [Install]\n\
         WantedBy=multi-user.target\n",
        name = service.name,
        env = env_section,
        path = binary_path,
        extra = extra,
        restart = restart,
    )
}

pub fn install_service(session: &Session, service: &ServiceConfig, content: &str) -> Result<(), String> {
    let remote_path = format!("/etc/systemd/system/{}.service", service.name);
    let mut channel = session.scp_send(
        Path::new(&remote_path),
        0o644_i32,
        content.len() as u64,
        None,
    )
    .map_err(|e| format!("Couldn't write service file on remote: {}", e))?;

    channel.write_all(content.as_bytes())
        .map_err(|e| format!("Couldn't write service content: {}", e))?;

    channel.send_eof().ok();
    channel.wait_eof().ok();
    channel.close().ok();
    channel.wait_close().ok();

    run_systemctl(session, "daemon-reload")?;
    run_systemctl_unit(session, "enable", &service.name)
}

pub fn start_service(session: &Session, name: &str) -> Result<(), String> {
    run_systemctl_unit(session, "start", name)
}

pub fn stop_service(session: &Session, name: &str) -> Result<(), String> {
    let _ = run_systemctl_unit(session, "stop", name);
    Ok(())
}

pub fn restart_service(session: &Session, name: &str) -> Result<(), String> {
    run_systemctl_unit(session, "restart", name)
}

pub fn service_status(session: &Session, name: &str) -> Result<bool, String> {
    let (_, _, code) = crate::ssh::run_command(
        session,
        &format!("systemctl is-active {}", sh_quote(name)),
    )?;
    Ok(code == 0)
}

fn run_systemctl(session: &Session, action: &str) -> Result<(), String> {
    let (_, stderr, code) = crate::ssh::run_command(
        session,
        &format!("systemctl {}", action),
    )?;
    if code != 0 {
        return Err(format!("systemctl {} failed: {}", action, stderr.trim()));
    }
    Ok(())
}

fn run_systemctl_unit(session: &Session, action: &str, unit: &str) -> Result<(), String> {
    let (_, stderr, code) = crate::ssh::run_command(
        session,
        &format!("systemctl {} {}", action, sh_quote(unit)),
    )?;
    if code != 0 {
        return Err(format!("systemctl {} '{}' failed: {}", action, unit, stderr.trim()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_generate_service_basic() {
        let svc = ServiceConfig {
            name: "myapp".into(),
            restart: None,
            extra_args: None,
        };
        let result = generate_service(&svc, "/opt/myapp/sortie", None);
        assert!(result.contains("[Unit]"));
        assert!(result.contains("[Service]"));
        assert!(result.contains("[Install]"));
        assert!(result.contains("ExecStart=/opt/myapp/sortie "));
        assert!(result.contains("Restart=always"));
        assert!(result.contains("Description=myapp"));
        assert!(result.contains("WantedBy=multi-user.target"));
        assert!(result.contains("PrivateTmp=true"));
        assert!(result.contains("NoNewPrivileges=true"));
        assert!(result.contains("ProtectSystem=full"));
        assert!(!result.contains("Environment="));
    }

    #[test]
    fn test_generate_service_with_restart() {
        let svc = ServiceConfig {
            name: "web".into(),
            restart: Some("on-failure".into()),
            extra_args: None,
        };
        let result = generate_service(&svc, "/opt/web/bin", None);
        assert!(result.contains("Restart=on-failure"));
    }

    #[test]
    fn test_generate_service_with_extra_args() {
        let svc = ServiceConfig {
            name: "api".into(),
            restart: None,
            extra_args: Some(vec!["--port".into(), "8080".into()]),
        };
        let result = generate_service(&svc, "/opt/api/sortie", None);
        assert!(result.contains("ExecStart=/opt/api/sortie --port 8080"));
    }

    #[test]
    fn test_generate_service_with_env_vars() {
        let svc = ServiceConfig {
            name: "app".into(),
            restart: None,
            extra_args: None,
        };
        let mut env = HashMap::new();
        env.insert("DATABASE_URL".into(), "postgres://localhost/db".into());
        env.insert("RUST_LOG".into(), "debug".into());
        let result = generate_service(&svc, "/opt/app/sortie", Some(&env));
        assert!(result.contains("Environment=DATABASE_URL=postgres://localhost/db"));
        assert!(result.contains("Environment=RUST_LOG=debug"));
    }

    #[test]
    fn test_generate_service_with_env_and_args() {
        let svc = ServiceConfig {
            name: "svc".into(),
            restart: Some("always".into()),
            extra_args: Some(vec!["--verbose".into()]),
        };
        let mut env = HashMap::new();
        env.insert("MODE".into(), "production".into());
        let result = generate_service(&svc, "/opt/sv/bin", Some(&env));
        assert!(result.contains("Environment=MODE=production"));
        assert!(result.contains("ExecStart=/opt/sv/bin --verbose"));
        assert!(result.contains("Restart=always"));
    }

    #[test]
    fn test_generate_service_custom_name() {
        let svc = ServiceConfig {
            name: "custom-service-name".into(),
            restart: None,
            extra_args: None,
        };
        let result = generate_service(&svc, "/bin/app", None);
        assert!(result.contains("Description=custom-service-name"));
    }
}

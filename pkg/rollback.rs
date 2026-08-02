use crate::ssh::sh_quote;
use crate::types::TargetConfig;

pub fn rollback(target: &str, config: &TargetConfig) -> Result<(), String> {
    let hosts = config.get_hosts();
    if hosts.is_empty() {
        return Err(format!("Target '{}' has no hosts configured.", target));
    }
    let host = &hosts[0];

    let session = crate::ssh::connect(host, config.port.unwrap_or(22), &config.user, config.key_path.as_deref())?;
    rollback_host(target, config, &session)
}

pub fn rollback_host(_target: &str, config: &TargetConfig, session: &ssh2::Session) -> Result<(), String> {
    let deployed = format!("{}/sortie", config.deploy_path);
    let backup = format!("{}/sortie.bak", config.deploy_path);
    let q_deployed = sh_quote(&deployed);
    let q_backup = sh_quote(&backup);

    println!("Restoring the previous version...");
    let cmd = format!(
        "if [ -f {} ]; then cp {} {} && echo ok; else echo 'no backup'; fi",
        q_backup, q_backup, q_deployed
    );
    let (stdout, _, code) = crate::ssh::run_command(session, &cmd)?;

    if code != 0 || stdout.trim() == "no backup" {
        return Err("No backup file found on the remote server.".to_string());
    }

    if let Some(ref svc) = config.service {
        println!("Restarting {}...", svc.name);
        crate::systemd::start_service(session, &svc.name)?;
    }

    println!("Rolled back to the previous version.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rollback_no_hosts() {
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
            pre_deploy: None,
            post_deploy: None,
        };
        let result = rollback("staging", &cfg);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no hosts"));
    }

    #[test]
    fn test_rollback_no_hosts_field_at_all() {
        let cfg = TargetConfig {
            host: None,
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
            pre_deploy: None,
            post_deploy: None,
        };
        let result = rollback("staging", &cfg);
        assert!(result.is_err());
    }
}

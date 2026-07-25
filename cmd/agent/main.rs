use std::env;
use std::process;
use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = env::args().collect();

    let mut deploy_path = String::new();
    let mut interval: u64 = 30;
    let mut proxy_mode = false;
    let mut proxy_port: u16 = 80;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--proxy" => proxy_mode = true,
            "--port" => {
                i += 1;
                proxy_port = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(80);
            }
            "--interval" => {
                i += 1;
                interval = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(30);
            }
            _ => {
                if deploy_path.is_empty() {
                    deploy_path = args[i].clone();
                }
            }
        }
        i += 1;
    }

    if deploy_path.is_empty() {
        eprintln!("Usage: sortie-agent <deploy-path> [--proxy] [--port 80] [--interval 30]");
        process::exit(1);
    }

    if proxy_mode {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(sortie::proxy::run_proxy(&deploy_path, proxy_port)).unwrap();
        return;
    }

    // watchdog mode
    loop {
        let state_path = format!("{}/.sortie/state.json", deploy_path);
        let content = match std::fs::read_to_string(&state_path) {
            Ok(c) => c,
            Err(_) => {
                thread::sleep(Duration::from_secs(interval));
                continue;
            }
        };
        let state: sortie::types::HostState = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(_) => {
                thread::sleep(Duration::from_secs(interval));
                continue;
            }
        };
        for (name, ep) in &state.services {
            let output = process::Command::new("systemctl")
                .args(["is-active", name])
                .output();
            let active = match output {
                Ok(o) => String::from_utf8_lossy(&o.stdout).trim() == "active",
                Err(_) => false,
            };
            if !active {
                eprintln!("{} down, restarting...", name);
                let _ = process::Command::new("systemctl")
                    .args(["restart", name])
                    .output();
            }
            let addr = format!("127.0.0.1:{}", ep.port);
            let ready = std::net::TcpStream::connect_timeout(
                &addr.parse().unwrap(),
                Duration::from_secs(3),
            ).is_ok();
            if !ready {
                eprintln!("{} port {} not ready, restarting...", name, ep.port);
                let _ = process::Command::new("systemctl")
                    .args(["restart", name])
                    .output();
            }
            if ep.health != "unknown" {
                let _ = process::Command::new("curl")
                    .args(["-sf", "-m", "5", &ep.health])
                    .output();
            }
        }
        thread::sleep(Duration::from_secs(interval));
    }
}

use std::path::Path;

use crate::types::Config;

pub fn init_project() -> Result<(), String> {
    let config_path = Path::new("sortie.toml");
    if config_path.exists() {
        return Err("sortie.toml already exists here. Delete it first if you want to start over.".to_string());
    }

    let config = Config::default();
    let content = toml::to_string_pretty(&config)
        .map_err(|_| "Couldn't turn the default config into TOML.".to_string())?;

    std::fs::write(config_path, content)
        .map_err(|e| format!("Couldn't write sortie.toml: {}", e))?;

    println!("Done! sortie.toml is ready to go.");
    println!("Open it up and set your server details, then run:");
    println!("  sortie apply production");
    println!();
    println!("Need help? Check the README for documentation.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
use std::sync::Mutex;

static INIT_LOCK: Mutex<()> = Mutex::new(());

fn lock_init() -> std::sync::MutexGuard<'static, ()> {
    INIT_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

#[test]
fn test_init_creates_toml() {
    let _lock = lock_init();
        let dir = std::env::temp_dir().join(format!("sortie_init_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let prev = std::env::current_dir().ok();
        std::env::set_current_dir(&dir).expect("set_current_dir");

        let result = init_project();
        assert!(result.is_ok());

        let toml_path = dir.join("sortie.toml");
        assert!(toml_path.exists(), "sortie.toml not found at {:?}", toml_path);

        let content = fs::read_to_string(&toml_path).unwrap();
        assert!(content.contains("[targets.production]"));
        assert!(content.contains("x86_64-unknown-linux-gnu"));

        if let Some(p) = prev {
            std::env::set_current_dir(&p).ok();
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_init_fails_if_already_exists() {
        let _lock = lock_init();
        let dir = std::env::temp_dir().join(format!("sortie_init_exists_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let prev = std::env::current_dir().ok();
        std::env::set_current_dir(&dir).expect("set_current_dir");
        fs::write("sortie.toml", "existing").unwrap();

        let result = init_project();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));

        if let Some(p) = prev {
            std::env::set_current_dir(&p).ok();
        }
        let _ = fs::remove_dir_all(&dir);
    }
}

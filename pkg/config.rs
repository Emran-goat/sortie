use std::path::{Path, PathBuf};

use crate::types::Config;

pub fn load_config(path: Option<&Path>) -> Result<Config, String> {
    let config_path = match path {
        Some(p) => p.to_path_buf(),
        None => {
            let root = find_project_root()?;
            root.join("sortie.toml")
        }
    };

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Couldn't read {}: {}", config_path.display(), e))?;

    let config: Config = toml::from_str(&content)
        .map_err(|e| format!("{} isn't valid TOML: {}", config_path.display(), e))?;

    if config.targets.is_empty() {
        return Err("Looks like your sortie.toml has no targets defined. Add at least one.".to_string());
    }

    Ok(config)
}

pub fn find_project_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir()
        .map_err(|_| "Can't figure out where you are.".to_string())?;

    let mut dir = Some(cwd.as_path());
    while let Some(d) = dir {
        if d.join("sortie.toml").exists() {
            return Ok(d.to_path_buf());
        }
        dir = d.parent();
    }

    Err("Couldn't find sortie.toml anywhere from here up to the root.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_find_project_root_not_found() {
        let dir = std::env::temp_dir().join(format!("sortie_test_root_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        std::env::set_current_dir(&dir).ok();
        let result = find_project_root();
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_config_file_not_found() {
        let p = Path::new("/tmp/nonexistent_sortie_config_test.toml");
        let result = load_config(Some(p));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_config_invalid_toml() {
        let dir = std::env::temp_dir().join(format!("sortie_test_config_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sortie.toml");
        fs::write(&path, "not valid toml [[[").unwrap();
        let result = load_config(Some(&path));
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_config_empty_targets() {
        let dir = std::env::temp_dir().join(format!("sortie_test_empty_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sortie.toml");
        fs::write(&path, "[targets]\n").unwrap();
        let result = load_config(Some(&path));
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_load_config_valid() {
        let dir = std::env::temp_dir().join(format!("sortie_test_valid_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sortie.toml");
        fs::write(
            &path,
            "[targets.staging]\nhosts = [\"10.0.0.1\"]\nuser = \"test\"\ntarget_triple = \"x86_64\"\ndeploy_path = \"/opt/app\"\n",
        )
        .unwrap();
        let result = load_config(Some(&path));
        assert!(result.is_ok());
        let cfg = result.unwrap();
        assert!(cfg.targets.contains_key("staging"));
        let _ = fs::remove_dir_all(&dir);
    }
}

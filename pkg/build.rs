use std::path::PathBuf;
use std::process::Command;

fn run_cargo(target_triple: &str, extra_args: &[String]) -> Result<(), String> {
    let status = Command::new("cargo")
        .arg("build").arg("--release").arg("--target").arg(target_triple)
        .args(extra_args)
        .status()
        .map_err(|e| format!("Couldn't run cargo: {}", e))?;
    if !status.success() {
        return Err(format!("cargo build failed for {}", target_triple));
    }
    Ok(())
}

fn run_cross(target_triple: &str, extra_args: &[String]) -> Result<(), String> {
    if Command::new("cross").arg("--version").output().is_err() {
        return Err("cross not found".into());
    }
    let status = Command::new("cross")
        .arg("build").arg("--release").arg("--target").arg(target_triple)
        .args(extra_args)
        .status()
        .map_err(|e| format!("Couldn't run cross: {}", e))?;
    if !status.success() {
        return Err(format!("cross build failed for {}", target_triple));
    }
    Ok(())
}

fn run_docker(target_triple: &str, extra_args: &[String]) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("can't get cwd: {}", e))?;
    let status = Command::new("docker")
        .arg("run").arg("--rm")
        .arg("-v").arg(format!("{}:/work", cwd.display()))
        .arg("-w").arg("/work")
        .arg(format!("rustembedded/cross:{}", target_triple))
        .arg("cargo").arg("build").arg("--release").arg("--target").arg(target_triple)
        .args(extra_args)
        .status()
        .map_err(|e| format!("Couldn't run docker: {}", e))?;
    if !status.success() {
        return Err(format!("docker build failed for {}", target_triple));
    }
    Ok(())
}

pub fn build_project(target_triple: &str, extra_args: &[String], cross_compile: bool) -> Result<PathBuf, String> {
    let is_linux = target_triple.contains("linux");
    let build_result = if cross_compile {
        run_cross(target_triple, extra_args)
            .or_else(|_| run_docker(target_triple, extra_args))
            .or_else(|_| run_cargo(target_triple, extra_args))
    } else {
        run_cargo(target_triple, extra_args)
            .or_else(|e| if is_linux {
                run_cross(target_triple, extra_args)
                    .or_else(|_| run_docker(target_triple, extra_args))
            } else {
                Err(e)
            })
    };
    build_result.map_err(|e| format!("All build methods failed for {}: {}", target_triple, e))?;

    let cargo_toml = std::fs::read_to_string("Cargo.toml")
        .map_err(|_| "No Cargo.toml here. Run this from your project root.".to_string())?;
    let parsed: toml::Value = toml::from_str(&cargo_toml)
        .map_err(|_| "Cargo.toml looks broken.".to_string())?;

    let name = parsed["package"]["name"].as_str()
        .ok_or("Can't find [package] name in Cargo.toml.".to_string())?;
    let binary_name = name.replace('-', "_");

    let path = PathBuf::from("target")
        .join(target_triple)
        .join("release")
        .join(&binary_name);

    if !path.exists() {
        return Err(format!("Built the project, but the binary isn't at {}.", path.display()));
    }

    Ok(path)
}

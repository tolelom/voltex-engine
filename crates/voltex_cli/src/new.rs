//! `voltex new <name>` — scaffold a fresh project.

use std::path::Path;

use crate::error::{CliError, Result};

pub fn cmd(args: &[String]) -> Result<()> {
    let name = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .ok_or(CliError::MissingArg("<name>"))?;
    create_at(Path::new(name))
}

pub fn create_at(target: &Path) -> Result<()> {
    if target.exists() {
        return Err(CliError::NewTargetExists(target.to_path_buf()));
    }
    let project_name = target
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("my_game")
        .to_string();

    std::fs::create_dir_all(target)?;
    std::fs::create_dir_all(target.join("scripts"))?;
    std::fs::create_dir_all(target.join("assets"))?;

    write_manifest(target, &project_name)?;
    write_empty_scene(target)?;
    write_sample_script(target)?;
    write_gitkeep(target)?;

    eprintln!("voltex new: scaffolded `{}`", target.display());
    Ok(())
}

fn write_manifest(target: &Path, name: &str) -> Result<()> {
    let manifest = format!(
        r#"[project]
name = "{}"
version = "0.1.0"

[build]
scene = "scene.vscn"
scripts = ["scripts/**/*.lua"]
assets = ["assets/**/*"]
window_title = "{}"
window_size = [1280, 720]
"#,
        name, name
    );
    std::fs::write(target.join("voltex.toml"), manifest)?;
    Ok(())
}

fn write_empty_scene(target: &Path) -> Result<()> {
    std::fs::write(target.join("scene.vscn"), "{\"entities\":[]}\n")?;
    Ok(())
}

fn write_sample_script(target: &Path) -> Result<()> {
    let script = r#"-- spinner.lua — sample script. Attach a ScriptComponent pointing
-- here to spin an entity around its Y axis.
function on_start(id, gen) end

function on_update(id, gen, dt)
    local rx, ry, rz = get_rotation(id, gen)
    if ry == nil then return end
    set_rotation(id, gen, rx, ry + dt, rz)
end
"#;
    std::fs::write(target.join("scripts").join("spinner.lua"), script)?;
    Ok(())
}

fn write_gitkeep(target: &Path) -> Result<()> {
    let path = target.join("assets").join(".gitkeep");
    std::fs::write(path, "")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_target(label: &str) -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("voltex_cli_new_test_{}_{}", label, stamp))
    }

    #[test]
    fn new_creates_expected_files() {
        let target = unique_target("ok");
        let _ = std::fs::remove_dir_all(&target);
        create_at(&target).unwrap();
        assert!(target.join("voltex.toml").exists());
        assert!(target.join("scene.vscn").exists());
        assert!(target.join("scripts").join("spinner.lua").exists());
        assert!(target.join("assets").join(".gitkeep").exists());

        let manifest = std::fs::read_to_string(target.join("voltex.toml")).unwrap();
        assert!(manifest.contains("[project]"));
        assert!(manifest.contains("name = "));

        let _ = std::fs::remove_dir_all(&target);
    }

    #[test]
    fn new_refuses_existing_directory() {
        let target = unique_target("exists");
        std::fs::create_dir_all(&target).unwrap();
        let err = create_at(&target).unwrap_err();
        matches!(err, CliError::NewTargetExists(_));
        let _ = std::fs::remove_dir_all(&target);
    }

    #[test]
    fn new_default_manifest_parses() {
        // The generated manifest must round-trip through the parser.
        let target = unique_target("roundtrip");
        let _ = std::fs::remove_dir_all(&target);
        create_at(&target).unwrap();
        let manifest = crate::manifest::Manifest::load_from_dir(&target).unwrap();
        assert!(!manifest.project.name.is_empty());
        assert_eq!(manifest.project.version, "0.1.0");
        let _ = std::fs::remove_dir_all(&target);
    }
}

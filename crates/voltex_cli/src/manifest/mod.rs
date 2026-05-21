//! voltex.toml manifest schema + a self-contained mini TOML parser.
//!
//! We accept just enough TOML to express the v1 manifest:
//!
//! - `[section]` and `[section.sub]` headers (one nesting level)
//! - `key = "string"`, `key = 123`, `key = true`
//! - `key = ["s", "s"]` or `key = [1, 2]`
//! - `#` comments to end of line, blank lines ignored
//!
//! Anything fancier (multiline strings, dotted keys, datetime, floats, inline
//! tables) is rejected with a line-numbered parse error.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{CliError, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    pub project: ProjectInfo,
    pub build: BuildConfig,
    /// Map of triple → per-target overrides (currently only `icon`).
    pub target_overrides: HashMap<String, TargetOverride>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildConfig {
    pub scene: String,
    pub scripts: Vec<String>,
    pub assets: Vec<String>,
    pub window_title: String,
    pub window_size: (u32, u32),
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TargetOverride {
    pub icon: Option<String>,
}

impl Manifest {
    /// Locate `voltex.toml` inside `dir` and parse it.
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let path = dir.join("voltex.toml");
        if !path.exists() {
            return Err(CliError::ManifestNotFound(path));
        }
        let text = std::fs::read_to_string(&path)?;
        Self::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let toml = parse_toml(text)?;
        validate_schema(&toml)?;

        let project_section = toml
            .get("project")
            .ok_or(CliError::MissingField { section: "project", field: "name" })?;
        let name = project_section
            .get_string("name")
            .ok_or(CliError::MissingField { section: "project", field: "name" })?
            .to_string();
        let version = project_section
            .get_string("version")
            .ok_or(CliError::MissingField { section: "project", field: "version" })?
            .to_string();

        let build_section = toml.get("build");
        let build = extract_build(build_section, &name)?;

        let mut target_overrides = HashMap::new();
        for (k, v) in toml.sections() {
            if let Some(stripped) = k.strip_prefix("build.") {
                let icon = v.get_string("icon").map(|s| s.to_string());
                target_overrides.insert(stripped.to_string(), TargetOverride { icon });
            }
        }

        Ok(Manifest {
            project: ProjectInfo { name, version },
            build,
            target_overrides,
        })
    }

    /// Verify that every path referenced by the manifest exists relative to
    /// `base_dir`. Called by `voltex build`. Pass `--no-validate-paths` on
    /// the CLI to skip (for build flows that synthesise paths at compile
    /// time).
    ///
    /// Validation rules:
    /// - `build.scene` must point at an existing file.
    /// - Each glob in `build.scripts` / `build.assets` must have its root
    ///   directory (stripped of `**` / `*`) existing.
    pub fn validate_paths(&self, base_dir: &Path) -> Result<()> {
        let scene = base_dir.join(&self.build.scene);
        if !scene.is_file() {
            return Err(CliError::ManifestPathMissing {
                field: "build.scene",
                path: scene,
                base_dir: base_dir.to_path_buf(),
            });
        }
        for pat in &self.build.scripts {
            let root = base_dir.join(strip_glob(pat));
            if !root.exists() {
                return Err(CliError::ManifestPathMissing {
                    field: "build.scripts",
                    path: root,
                    base_dir: base_dir.to_path_buf(),
                });
            }
        }
        for pat in &self.build.assets {
            let root = base_dir.join(strip_glob(pat));
            if !root.exists() {
                return Err(CliError::ManifestPathMissing {
                    field: "build.assets",
                    path: root,
                    base_dir: base_dir.to_path_buf(),
                });
            }
        }
        Ok(())
    }

    /// Resolve every script path in the manifest against `base_dir`.
    pub fn resolved_script_dirs(&self, base_dir: &Path) -> Vec<PathBuf> {
        self.build.scripts.iter().map(|s| base_dir.join(strip_glob(s))).collect()
    }
    pub fn resolved_assets_dirs(&self, base_dir: &Path) -> Vec<PathBuf> {
        self.build.assets.iter().map(|s| base_dir.join(strip_glob(s))).collect()
    }
}

// ── Schema validation (v1.0.11): catch typos in section / key names ─────

const VALID_PROJECT_KEYS: &[&str] = &["name", "version"];
const VALID_BUILD_KEYS: &[&str] = &[
    "scene",
    "scripts",
    "assets",
    "window_title",
    "window_size",
    "targets",
];
const VALID_TARGET_OVERRIDE_KEYS: &[&str] = &["icon"];
const VALID_SECTION_PREFIXES: &[&str] = &["project", "build", "build.<triple>"];

/// Compare two strings character by character for the case-insensitive
/// "did you mean" suggestion. Cheap and sufficient for catching common
/// typos like `windows_title` vs `window_title` or `scrips` vs `scripts`.
fn levenshtein(a: &str, b: &str) -> usize {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=bv.len()).collect();
    let mut curr = vec![0usize; bv.len() + 1];
    for i in 1..=av.len() {
        curr[0] = i;
        for j in 1..=bv.len() {
            let cost = if av[i - 1].eq_ignore_ascii_case(&bv[j - 1]) { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[bv.len()]
}

fn closest_match(field: &str, valid: &[&'static str]) -> Option<&'static str> {
    valid
        .iter()
        .map(|&v| (v, levenshtein(field, v)))
        .filter(|(_, d)| *d <= 3) // within 3 edits — catches realistic typos, rejects unrelated
        .min_by_key(|(_, d)| *d)
        .map(|(v, _)| v)
}

fn validate_schema(toml: &TomlDoc) -> Result<()> {
    for (section_name, section) in toml.sections() {
        let valid_keys: &[&str] = if section_name == "project" {
            VALID_PROJECT_KEYS
        } else if section_name == "build" {
            VALID_BUILD_KEYS
        } else if section_name.starts_with("build.") {
            VALID_TARGET_OVERRIDE_KEYS
        } else {
            return Err(CliError::UnknownSection {
                name: section_name.clone(),
                valid_prefixes: VALID_SECTION_PREFIXES.to_vec(),
            });
        };

        for (key, _) in &section.entries {
            if !valid_keys.iter().any(|v| v == key) {
                return Err(CliError::UnknownField {
                    section: section_name.clone(),
                    field: key.clone(),
                    valid: valid_keys.to_vec(),
                    suggestion: closest_match(key, valid_keys),
                });
            }
        }
    }
    Ok(())
}

/// `scripts/**/*.lua` → `scripts`. Just enough for our prototype copy step.
fn strip_glob(pattern: &str) -> &str {
    if let Some(idx) = pattern.find("/**") {
        &pattern[..idx]
    } else if let Some(idx) = pattern.find("/*") {
        &pattern[..idx]
    } else {
        pattern
    }
}

fn extract_build(
    section: Option<&TomlSection>,
    project_name: &str,
) -> Result<BuildConfig> {
    let default_scene = "scene.vscn".to_string();
    let default_scripts = vec!["scripts/**/*.lua".to_string()];
    let default_assets = vec!["assets/**/*".to_string()];
    let default_title = project_name.to_string();
    let default_size = (1280u32, 720u32);
    let default_targets = vec![host_triple().to_string()];

    let Some(s) = section else {
        return Ok(BuildConfig {
            scene: default_scene,
            scripts: default_scripts,
            assets: default_assets,
            window_title: default_title,
            window_size: default_size,
            targets: default_targets,
        });
    };

    let scene = s.get_string("scene").map(|s| s.to_string()).unwrap_or(default_scene);
    let scripts = s
        .get_string_array("scripts")?
        .unwrap_or(default_scripts);
    let assets = s
        .get_string_array("assets")?
        .unwrap_or(default_assets);
    let window_title = s
        .get_string("window_title")
        .map(|s| s.to_string())
        .unwrap_or(default_title);
    let window_size = match s.get_int_array("window_size")? {
        Some(values) if values.len() == 2 => (values[0] as u32, values[1] as u32),
        Some(other) => {
            return Err(CliError::InvalidValue {
                field: "window_size".to_string(),
                got: format!("array of {} elements (expected 2)", other.len()),
            });
        }
        None => default_size,
    };
    let targets = s
        .get_string_array("targets")?
        .unwrap_or(default_targets);

    Ok(BuildConfig {
        scene,
        scripts,
        assets,
        window_title,
        window_size,
        targets,
    })
}

pub fn host_triple() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", _) => "windows-x64",
        ("linux", _) => "linux-x64",
        ("macos", "aarch64") => "macos-arm64",
        ("macos", _) => "macos-x64",
        _ => "windows-x64",
    }
}

// ── Mini TOML parser ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct TomlSection {
    entries: Vec<(String, TomlValue)>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Int/Bool are produced by the parser even though no
                    // current manifest field reads them; future fields may.
enum TomlValue {
    String(String),
    Int(i64),
    Bool(bool),
    StringArray(Vec<String>),
    IntArray(Vec<i64>),
}

impl TomlSection {
    fn get(&self, key: &str) -> Option<&TomlValue> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    fn get_string(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            TomlValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }
    fn get_string_array(&self, key: &str) -> Result<Option<Vec<String>>> {
        match self.get(key) {
            None => Ok(None),
            Some(TomlValue::StringArray(v)) => Ok(Some(v.clone())),
            Some(other) => Err(CliError::InvalidValue {
                field: key.to_string(),
                got: format!("{:?}", other),
            }),
        }
    }
    fn get_int_array(&self, key: &str) -> Result<Option<Vec<i64>>> {
        match self.get(key) {
            None => Ok(None),
            Some(TomlValue::IntArray(v)) => Ok(Some(v.clone())),
            Some(other) => Err(CliError::InvalidValue {
                field: key.to_string(),
                got: format!("{:?}", other),
            }),
        }
    }
}

#[derive(Debug, Default)]
struct TomlDoc {
    sections: Vec<(String, TomlSection)>,
}

impl TomlDoc {
    fn get(&self, name: &str) -> Option<&TomlSection> {
        self.sections.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }
    fn sections(&self) -> impl Iterator<Item = (&String, &TomlSection)> {
        self.sections.iter().map(|(k, v)| (k, v))
    }
}

fn parse_toml(text: &str) -> Result<TomlDoc> {
    let mut doc = TomlDoc::default();
    let mut current = String::new();
    let mut current_entries: Vec<(String, TomlValue)> = Vec::new();

    let flush = |doc: &mut TomlDoc, name: &mut String, entries: &mut Vec<(String, TomlValue)>| {
        if !name.is_empty() || !entries.is_empty() {
            doc.sections.push((
                name.clone(),
                TomlSection { entries: std::mem::take(entries) },
            ));
            name.clear();
        }
    };

    for (i, raw_line) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = parse_header(line, line_no)? {
            flush(&mut doc, &mut current, &mut current_entries);
            current = header;
            continue;
        }
        let (key, value) = parse_kv(line, line_no)?;
        current_entries.push((key, value));
    }
    flush(&mut doc, &mut current, &mut current_entries);
    Ok(doc)
}

fn strip_comment(line: &str) -> &str {
    // Naive but correct for our subset: `#` outside of a string starts a comment.
    let bytes = line.as_bytes();
    let mut in_str = false;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'"' {
            in_str = !in_str;
        }
        if b == b'#' && !in_str {
            return &line[..i];
        }
    }
    line
}

fn parse_header(line: &str, line_no: usize) -> Result<Option<String>> {
    let bytes = line.as_bytes();
    if bytes.first() != Some(&b'[') {
        return Ok(None);
    }
    if bytes.last() != Some(&b']') {
        return Err(CliError::ManifestParse {
            line: line_no,
            msg: "section header missing closing `]`".to_string(),
        });
    }
    let inner = line[1..line.len() - 1].trim();
    if inner.is_empty() {
        return Err(CliError::ManifestParse {
            line: line_no,
            msg: "empty section header".to_string(),
        });
    }
    Ok(Some(inner.to_string()))
}

fn parse_kv(line: &str, line_no: usize) -> Result<(String, TomlValue)> {
    let Some(eq) = line.find('=') else {
        return Err(CliError::ManifestParse {
            line: line_no,
            msg: "expected `key = value`".to_string(),
        });
    };
    let key = line[..eq].trim().to_string();
    if key.is_empty() {
        return Err(CliError::ManifestParse {
            line: line_no,
            msg: "empty key".to_string(),
        });
    }
    let value_part = line[eq + 1..].trim();
    let value = parse_value(value_part, line_no)?;
    Ok((key, value))
}

fn parse_value(raw: &str, line_no: usize) -> Result<TomlValue> {
    if raw.starts_with('"') {
        return parse_string(raw, line_no).map(TomlValue::String);
    }
    if raw == "true" {
        return Ok(TomlValue::Bool(true));
    }
    if raw == "false" {
        return Ok(TomlValue::Bool(false));
    }
    if raw.starts_with('[') {
        return parse_array(raw, line_no);
    }
    // Plain integer?
    if let Ok(n) = raw.parse::<i64>() {
        return Ok(TomlValue::Int(n));
    }
    Err(CliError::ManifestParse {
        line: line_no,
        msg: format!("unrecognised value `{}`", raw),
    })
}

fn parse_string(raw: &str, line_no: usize) -> Result<String> {
    if !raw.ends_with('"') || raw.len() < 2 {
        return Err(CliError::ManifestParse {
            line: line_no,
            msg: "unterminated string".to_string(),
        });
    }
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => return Err(CliError::ManifestParse {
                    line: line_no,
                    msg: "trailing backslash in string".to_string(),
                }),
            }
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

fn parse_array(raw: &str, line_no: usize) -> Result<TomlValue> {
    if !raw.ends_with(']') || raw.len() < 2 {
        return Err(CliError::ManifestParse {
            line: line_no,
            msg: "unterminated array".to_string(),
        });
    }
    let inner = raw[1..raw.len() - 1].trim();
    if inner.is_empty() {
        return Ok(TomlValue::StringArray(Vec::new()));
    }
    let items = split_array_items(inner);

    // Decide string vs int based on the first element.
    let first = items[0].trim();
    if first.starts_with('"') {
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(parse_string(item.trim(), line_no)?);
        }
        Ok(TomlValue::StringArray(out))
    } else {
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            let n: i64 = item.trim().parse().map_err(|_| CliError::ManifestParse {
                line: line_no,
                msg: format!("expected integer in array, got `{}`", item.trim()),
            })?;
            out.push(n);
        }
        Ok(TomlValue::IntArray(out))
    }
}

/// Split on top-level commas, respecting `"..."` string boundaries.
fn split_array_items(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_str = false;
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        match bytes[i] {
            b'"' => in_str = !in_str,
            b',' if !in_str => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    let tail = &s[start..];
    if !tail.trim().is_empty() {
        parts.push(tail);
    }
    parts
}


#[cfg(test)]
mod tests;

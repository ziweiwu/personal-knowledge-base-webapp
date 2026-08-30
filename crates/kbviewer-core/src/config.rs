//! Configuration: which folders may be served, and how each behaves.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("no config at {0}: copy kbviewer.config.example.json and set your folder path")]
    Missing(PathBuf),
    #[error("{0}: {1}")]
    Invalid(PathBuf, String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Names tried, in order, when deciding what a folder's landing page is.
fn default_index_names() -> Vec<String> {
    vec!["index.md".into(), "README.md".into(), "readme.md".into()]
}

fn default_host() -> String {
    "127.0.0.1".into()
}

/// The port the server listens on when the config does not say.
const DEFAULT_PORT: u16 = 4321;

fn default_port() -> u16 {
    DEFAULT_PORT
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("./data")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootConfig {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub path: PathBuf,

    #[serde(default = "default_index_names")]
    pub index_names: Vec<String>,

    /// Force wikilink parsing on or off. `None` means "decide from `.obsidian/`",
    /// which is what lets a plain markdown folder and a vault both work untouched.
    #[serde(default)]
    pub wikilinks: Option<bool>,

    /// Treat a note named after its folder as that folder's landing page.
    #[serde(default)]
    pub folder_notes: bool,

    /// Refuse all write routes for this root.
    #[serde(default)]
    pub read_only: bool,
}

impl RootConfig {
    /// A root is in Obsidian mode when it has a `.obsidian/` directory, unless config
    /// overrides it. Detection rather than declaration means pointing the app at an
    /// existing vault needs no configuration at all.
    pub fn uses_wikilinks(&self) -> bool {
        self.wikilinks
            .unwrap_or_else(|| self.path.join(".obsidian").is_dir())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    pub roots: Vec<RootConfig>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let raw =
            std::fs::read_to_string(path).map_err(|_| ConfigError::Missing(path.to_path_buf()))?;
        let mut config: Config = serde_json::from_str(&raw)
            .map_err(|e| ConfigError::Invalid(path.to_path_buf(), e.to_string()))?;
        config.validate_and_normalise(path)?;
        Ok(config)
    }

    fn validate_and_normalise(&mut self, source: &Path) -> Result<(), ConfigError> {
        // Applied before validation so an overridden path is expanded, checked and
        // canonicalised exactly like one written in the file. Applying it afterwards left
        // container deployments with a non-canonical root, which silently breaks watching.
        self.apply_env_overrides();

        self.validate_roots()
            .map_err(|reason| ConfigError::Invalid(source.to_path_buf(), reason))
    }

    fn validate_roots(&mut self) -> Result<(), String> {
        if self.roots.is_empty() {
            return Err("\"roots\" must list at least one folder".into());
        }
        for root in &mut self.roots {
            validate_and_normalise_root(root)?;
        }
        reject_duplicate_ids(&self.roots)
    }

    /// `KBVIEWER_ROOT_<ID>` repoints a root without editing the file, so the same config
    /// works in a container where the folder is mounted somewhere else.
    fn apply_env_overrides(&mut self) {
        for root in &mut self.roots {
            let key = format!("KBVIEWER_ROOT_{}", root.id.to_uppercase().replace('-', "_"));
            if let Ok(value) = std::env::var(&key) {
                root.path = PathBuf::from(value);
            }
        }
    }

    pub fn root(&self, id: &str) -> Option<&RootConfig> {
        self.roots.iter().find(|r| r.id == id)
    }
}

/// Reject a root the server could not serve, and fill in what the file left unset.
fn validate_and_normalise_root(root: &mut RootConfig) -> Result<(), String> {
    if root.id.is_empty() || !root.id.chars().all(is_url_safe_id_char) {
        return Err(format!(
            "root id {:?} must be lowercase alphanumeric with dashes",
            root.id
        ));
    }
    if !root.path.is_absolute() {
        return Err(format!("root {:?} needs an absolute \"path\"", root.id));
    }
    if root.name.is_empty() {
        root.name = root.id.clone();
    }
    root.path = canonical(&expand_tilde(&root.path));
    Ok(())
}

fn is_url_safe_id_char(character: char) -> bool {
    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
}

/// Two roots sharing an id would make every route addressing them ambiguous.
fn reject_duplicate_ids(roots: &[RootConfig]) -> Result<(), String> {
    let mut ids: Vec<&str> = roots.iter().map(|root| root.id.as_str()).collect();
    ids.sort_unstable();
    match ids.windows(2).find(|pair| pair[0] == pair[1]) {
        Some(duplicate) => Err(format!("duplicate root id {:?}", duplicate[0])),
        None => Ok(()),
    }
}

/// Resolve symlinks in a configured root.
///
/// Filesystem watch events always carry canonical paths, so a root configured through a
/// symlink (on macOS `/tmp` is a link to `/private/tmp`) would never match its own
/// events: every change would be silently discarded and the index would go stale with no
/// error anywhere. Canonicalising once here keeps watching, indexing and path containment
/// all speaking the same paths.
fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn expand_tilde(path: &Path) -> PathBuf {
    let Ok(rest) = path.strip_prefix("~") else {
        return path.to_path_buf();
    };
    match std::env::var_os("HOME") {
        Some(home) => PathBuf::from(home).join(rest),
        None => path.to_path_buf(),
    }
}

/// Normalise an email the same way the user store does, so a login attempt and a stored
/// account agree on identity regardless of how the address was typed.
pub fn normalise_email_for_login(email: &str) -> String {
    email.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_config(label: &str, body: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("kbviewer-config-{label}.json"));
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn loads_a_minimal_config_and_defaults_the_rest() {
        let path = write_config(
            "minimal",
            r#"{"roots":[{"id":"kb","path":"/tmp/kbviewer-test-vault"}]}"#,
        );
        let config = Config::load(&path).unwrap();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, DEFAULT_PORT);
        assert_eq!(config.roots[0].name, "kb", "name defaults to the id");
        assert_eq!(config.roots[0].index_names[0], "index.md");
    }

    #[test]
    fn rejects_a_relative_root_path() {
        let path = write_config("relative", r#"{"roots":[{"id":"kb","path":"./notes"}]}"#);
        assert!(Config::load(&path).is_err());
    }

    #[test]
    fn rejects_an_empty_roots_list() {
        let path = write_config("empty", r#"{"roots":[]}"#);
        assert!(Config::load(&path).is_err());
    }

    #[test]
    fn rejects_duplicate_root_ids() {
        let path = write_config(
            "dup",
            r#"{"roots":[{"id":"kb","path":"/tmp/a"},{"id":"kb","path":"/tmp/b"}]}"#,
        );
        assert!(Config::load(&path).is_err());
    }

    #[test]
    fn rejects_an_id_that_would_not_be_url_safe() {
        let path = write_config("badid", r#"{"roots":[{"id":"My Vault","path":"/tmp/a"}]}"#);
        assert!(Config::load(&path).is_err());
    }

    #[test]
    fn an_env_override_is_canonicalised_like_a_configured_path() {
        let real = std::env::temp_dir().join("kbviewer-config-envroot");
        let _ = std::fs::remove_dir_all(&real);
        std::fs::create_dir_all(&real).unwrap();

        let path = write_config(
            "envoverride",
            r#"{"roots":[{"id":"envroot","path":"/nonexistent"}]}"#,
        );
        // SAFETY: single-threaded test; the variable is removed immediately after loading.
        unsafe { std::env::set_var("KBVIEWER_ROOT_ENVROOT", &real) };
        let config = Config::load(&path);
        unsafe { std::env::remove_var("KBVIEWER_ROOT_ENVROOT") };

        assert_eq!(
            config.unwrap().roots[0].path,
            real.canonicalize().unwrap(),
            "an override that skipped canonicalisation would silently stop the watcher"
        );
    }

    #[test]
    fn a_root_reached_through_a_symlink_is_stored_canonically() {
        let real = std::env::temp_dir().join("kbviewer-config-real");
        let _ = std::fs::remove_dir_all(&real);
        std::fs::create_dir_all(&real).unwrap();

        let path = write_config(
            "symlink",
            &format!(r#"{{"roots":[{{"id":"kb","path":"{}"}}]}}"#, real.display()),
        );
        let config = Config::load(&path).unwrap();
        assert_eq!(
            config.roots[0].path,
            real.canonicalize().unwrap(),
            "watch events carry canonical paths; a non-canonical root would never match them"
        );
    }

    #[test]
    fn honours_an_explicit_wikilink_override_over_detection() {
        let root = RootConfig {
            id: "plain".into(),
            name: "plain".into(),
            path: PathBuf::from("/tmp/definitely-not-a-vault"),
            index_names: default_index_names(),
            wikilinks: Some(true),
            folder_notes: false,
            read_only: false,
        };
        assert!(
            root.uses_wikilinks(),
            "explicit true must win over detection"
        );
    }
}

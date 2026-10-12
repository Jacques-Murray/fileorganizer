use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::FoError;
use crate::util::expand_tilde;

/// The default config, shared with `examples/config.toml`.
pub const DEFAULT_CONFIG: &str = include_str!("../examples/config.toml");

/// Template variables allowed inside a rule's `target`.
pub const TEMPLATE_VARS: [&str; 6] = ["year", "month", "day", "ext", "name", "artist_or_unknown"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Collision {
    #[default]
    Rename,
    Skip,
    Overwrite,
    TrashExisting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct General {
    pub target_root: String,
    #[serde(default)]
    pub always_apply: bool,
    #[serde(default = "default_true")]
    pub skip_hidden: bool,
    #[serde(default)]
    pub min_size_bytes: u64,
    #[serde(default)]
    pub collision: Collision,
    pub journal_dir: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub name: String,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub mimes: Vec<String>,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub general: General,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

impl Config {
    pub fn target_root(&self) -> PathBuf {
        expand_tilde(&self.general.target_root)
    }

    pub fn journal_dir(&self) -> PathBuf {
        expand_tilde(&self.general.journal_dir)
    }

    pub fn parse(text: &str) -> Result<Config, FoError> {
        let cfg: Config =
            toml::from_str(text).map_err(|e| FoError::InvalidConfig(e.to_string()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Load from `path`, or from the platform default location.
    pub fn load(path: Option<&Path>) -> Result<(Config, PathBuf), FoError> {
        let path = match path {
            Some(p) => p.to_path_buf(),
            None => default_config_path()?,
        };
        if !path.exists() {
            return Err(FoError::ConfigNotFound(path));
        }
        let text = std::fs::read_to_string(&path).map_err(|e| FoError::io(&path, e))?;
        Ok((Config::parse(&text)?, path))
    }

    fn validate(&self) -> Result<(), FoError> {
        let bad = |msg: String| Err(FoError::InvalidConfig(msg));

        if self.general.target_root.trim().is_empty() {
            return bad("general.target_root must not be empty".into());
        }
        for rule in &self.rules {
            if rule.extensions.is_empty() && rule.patterns.is_empty() && rule.mimes.is_empty() {
                return bad(format!(
                    "rule '{}' needs at least one of: extensions, patterns, mimes",
                    rule.name
                ));
            }
            for p in rule.patterns.iter().chain(rule.mimes.iter()) {
                if glob::Pattern::new(p).is_err() {
                    return bad(format!("rule '{}' has an invalid pattern '{p}'", rule.name));
                }
            }
            let target = Path::new(&rule.target);
            if target.is_absolute()
                || target
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                return bad(format!(
                    "rule '{}': target must be relative and must not contain '..'",
                    rule.name
                ));
            }
            // Check every {var} is known.
            let mut rest = rule.target.as_str();
            while let Some(start) = rest.find('{') {
                let Some(len) = rest[start..].find('}') else {
                    return bad(format!("rule '{}': unclosed '{{' in target", rule.name));
                };
                let var = &rest[start + 1..start + len];
                if !TEMPLATE_VARS.contains(&var) {
                    return bad(format!(
                        "rule '{}': unknown template variable {{{var}}}",
                        rule.name
                    ));
                }
                rest = &rest[start + len + 1..];
            }
        }
        Ok(())
    }
}

pub fn default_config_path() -> Result<PathBuf, FoError> {
    dirs::config_dir()
        .map(|d| d.join("fileorganizer").join("config.toml"))
        .ok_or(FoError::NoHomeDir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = Config::parse(DEFAULT_CONFIG).expect("default config must parse");
        assert_eq!(cfg.general.collision, Collision::Rename);
        assert_eq!(cfg.rules.last().unwrap().name, "Other");
    }

    #[test]
    fn rejects_unknown_template_var() {
        let text = r#"
            [general]
            target_root = "/tmp/x"
            journal_dir = "/tmp/j"
            [[rules]]
            name = "Bad"
            extensions = ["txt"]
            target = "Docs/{nope}"
        "#;
        assert!(Config::parse(text).is_err());
    }

    #[test]
    fn rejects_parent_dir_target() {
        let text = r#"
            [general]
            target_root = "/tmp/x"
            journal_dir = "/tmp/j"
            [[rules]]
            name = "Bad"
            extensions = ["txt"]
            target = "../escape"
        "#;
        assert!(Config::parse(text).is_err());
    }

    #[test]
    fn rejects_rule_without_criteria() {
        let text = r#"
            [general]
            target_root = "/tmp/x"
            journal_dir = "/tmp/j"
            [[rules]]
            name = "Empty"
            target = "Nothing"
        "#;
        assert!(Config::parse(text).is_err());
    }
}

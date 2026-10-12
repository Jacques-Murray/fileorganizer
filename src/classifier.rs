use std::path::Path;

use chrono::{DateTime, Datelike, Utc};
use glob::{MatchOptions, Pattern};

use crate::config::{Config, Rule};

/// Everything we know about a file before classifying it.
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub file_name: String,
    /// Lowercase extension without the dot; empty if none.
    pub ext: String,
    pub stem: String,
    pub size: u64,
    pub modified: DateTime<Utc>,
    /// MIME tyep sniffed from magic bytes, if recognised.
    pub mime: Option<String>,
}

impl FileInfo {
    pub fn from_path(path: &Path, metadata: &std::fs::Metadata) -> FileInfo {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let modified = metadata
            .modified()
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());
        let mime = infer::get_from_path(path)
            .ok()
            .flatten()
            .map(|t| t.mime_type().to_string());
        FileInfo {
            file_name,
            ext,
            stem,
            size: metadata.len(),
            modified,
            mime,
        }
    }
}

struct CompiledRule {
    name: String,
    extensions: Vec<String>,
    wildcard: bool,
    patterns: Vec<Pattern>,
    mimes: Vec<Pattern>,
    target: String,
}

pub struct Classifier {
    rules: Vec<CompiledRule>,
}

#[derive(Debug, Clone)]
pub struct Match {
    pub rule: String,
    /// Target directory (relative to target_root) after template expansion.
    pub target: String,
}

const MATCH_OPTS: MatchOptions = MatchOptions {
    case_sensitive: false,
    require_literal_separator: false,
    require_literal_leading_dot: false,
};

impl Classifier {
    pub fn new(cfg: &Config) -> Classifier {
        let rules = cfg.rules.iter().map(compile).collect();
        Classifier { rules };
    }

    /// First matching rule wins. Return `None` if nothing matches.
    pub fn classify(&self, info: &FileInfo) -> Option<Match> {
        let lower_name = info.file_name.to_lowercase();
        self.rules
            .iter()
            .find(|r| rule_matches(r, info, &lower_name))
            .map(|r| Match {
                rule: r.name.clone(),
                target: render_template(&r.target, info),
            })
    }
}

fn compile(rule: &Rule) -> CompiledRule {
    // Patterns were validated in Config::parse, so unwrap-free filtering is safe.
    let compile_all = |items: &[String]| -> Vec<Pattern> {
        items
            .iter()
            .filter_map(|p| Pattern::new(&p.to_lowercase()).ok())
            .collect()
    };
    CompiledRule {
        name: rule.name.clone(),
        wildcard: rule.extensions.iter().any(|e| e == "*"),
        extensions: rule
            .extensions
            .iter()
            .map(|e| e.trim_start_matches('.').to_lowercase())
            .collect(),
        patterns: compile_all(&rule.patterns),
        mimes: compile_all(&rule.mimes),
        target: rule.target.clone(),
    }
}

/// Every criterion that is present must match; absent criteria are ignored.
fn rule_matches(rule: &CompiledRule, info: &FileInfo, lower_name: &str) -> bool {
    let ext_ok = rule.extensions.is_empty() || rule.wildcard || rule.extensions.contains(&info.ext);
    let pattern_ok = rule.patterns.is_empty()
        || rule
            .patterns
            .iter()
            .any(|p| p.matches_with(lower_name, MATCH_OPTS));
    let mime_ok = rule.mimes.is_empty()
        || info.mime.as_deref().is_some_and(|m| {
            let m = m.to_lowercase();
            rule.mimes.iter().any(|p| p.matches_with(&m, MATCH_OPTS))
        });
    ext_ok && pattern_ok && mime_ok
}

use std::path::{Path, PathBuf};

/// Expand a leading `~` to the user's home directory.
pub fn expand_tilde(input: &str) -> PathBuf {
    if input == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(input));
    }
    if let Some(rest) = input
        .strip_prefix("~/")
        .or_else(|| input.strip_prefix("~\\"))
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(input)
}

/// Make a path absolute without requiring it to exist.
pub fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_size_formats() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn tilde_expansion_leaves_plain_paths() {
        assert_eq!(expand_tilde("/tmp/x"), PathBuf::from("/tmp/x"));
        assert_eq!(expand_tilde("relative/dir"), PathBuf::from("relative/dir"));
    }

    #[test]
    fn tilde_expands_to_home() {
        if let Some(home) = dirs::home_dir() {
            assert_eq!(expand_tilde("~/Organized"), home.join("Organized"));
            assert_eq!(expand_tilde("~"), home);
        }
    }
}

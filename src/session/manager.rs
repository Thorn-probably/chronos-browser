use std::fs;
use std::path::{Path, PathBuf};

/// Manages browser profiles and session configuration (headless vs headful).
#[derive(Debug, Clone)]
pub struct SessionManager {
    profiles_dir: PathBuf,
    active_profile: String,
    headful: bool,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    /// Creates a new SessionManager using the standard data directory:
    /// `~/.local/share/chronos-browser/profiles/`
    pub fn new() -> Self {
        let base = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("chronos-browser")
            .join("profiles");

        Self::with_profiles_dir(base)
    }

    /// Creates a new SessionManager with a custom profiles directory (useful for testing).
    pub fn with_profiles_dir(profiles_dir: impl Into<PathBuf>) -> Self {
        let profiles_dir = profiles_dir.into();
        Self {
            profiles_dir,
            active_profile: "default".to_string(),
            headful: false,
        }
    }

    /// Returns the root directory where profiles are stored.
    pub fn profiles_dir(&self) -> &Path {
        &self.profiles_dir
    }

    /// Returns the name of the currently active profile.
    pub fn active_profile(&self) -> &str {
        &self.active_profile
    }

    /// Sets the active profile name.
    pub fn set_active_profile(&mut self, name: impl Into<String>) {
        self.active_profile = name.into();
    }

    /// Returns whether browser sessions should run in headful mode.
    pub fn is_headful(&self) -> bool {
        self.headful
    }

    /// Sets whether browser sessions should run in headful mode.
    pub fn set_headful(&mut self, headful: bool) {
        self.headful = headful;
    }

    /// Returns the path to the directory for a given profile name.
    pub fn profile_dir(&self, name: &str) -> PathBuf {
        self.profiles_dir.join(name)
    }

    /// Returns the path for the currently active profile.
    pub fn active_profile_dir(&self) -> PathBuf {
        self.profile_dir(&self.active_profile)
    }

    /// Ensures that the profile directory for the specified profile exists.
    pub fn ensure_profile_dir(&self, name: &str) -> std::io::Result<PathBuf> {
        let dir = self.profile_dir(name);
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }
        Ok(dir)
    }

    /// Lists all existing profiles found in the profiles directory.
    pub fn list_profiles(&self) -> std::io::Result<Vec<String>> {
        if !self.profiles_dir.exists() {
            return Ok(Vec::new());
        }

        let mut profiles = Vec::new();
        for entry in fs::read_dir(&self.profiles_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    // Ignore hidden directories
                    if !name.starts_with('.') {
                        profiles.push(name.to_string());
                    }
                }
            }
        }

        profiles.sort();
        Ok(profiles)
    }

    /// Deletes a profile directory.
    pub fn delete_profile(&self, name: &str) -> std::io::Result<bool> {
        let dir = self.profile_dir(name);
        if dir.exists() {
            fs::remove_dir_all(dir)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_profile_lifecycle() {
        let tmp = tempdir().unwrap();
        let mut mgr = SessionManager::with_profiles_dir(tmp.path().join("profiles"));

        assert_eq!(mgr.active_profile(), "default");
        assert!(!mgr.is_headful());

        mgr.set_headful(true);
        assert!(mgr.is_headful());

        // List when empty
        let profiles = mgr.list_profiles().unwrap();
        assert!(profiles.is_empty());

        // Ensure directories
        mgr.ensure_profile_dir("default").unwrap();
        mgr.ensure_profile_dir("work").unwrap();

        let mut profiles = mgr.list_profiles().unwrap();
        profiles.sort();
        assert_eq!(profiles, vec!["default", "work"]);

        // Delete profile
        let deleted = mgr.delete_profile("work").unwrap();
        assert!(deleted);
        let profiles = mgr.list_profiles().unwrap();
        assert_eq!(profiles, vec!["default"]);
    }
}

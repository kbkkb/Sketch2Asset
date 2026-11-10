use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const KEYRING_SERVICE: &str = "com.sketch2asset.app";

pub struct SecretStore {
    fallback_path: PathBuf,
}

impl SecretStore {
    pub fn new(app_data_dir: &Path) -> Self {
        let _ = fs::create_dir_all(app_data_dir);
        Self {
            fallback_path: app_data_dir.join("secrets.json"),
        }
    }

    fn read_fallback_map(&self) -> HashMap<String, String> {
        fs::read_to_string(&self.fallback_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn write_fallback_map(&self, map: &HashMap<String, String>) {
        if let Ok(json) = serde_json::to_string_pretty(map) {
            let tmp = self.fallback_path.with_extension("json.tmp");
            if fs::write(&tmp, json).is_ok() {
                let _ = fs::rename(&tmp, &self.fallback_path);
            }
        }
    }

    pub fn set_key(&self, profile_id: &str, api_key: &str) {
        let trimmed = api_key.trim();
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, profile_id) {
            if trimmed.is_empty() {
                let _ = entry.delete_credential();
            } else {
                let _ = entry.set_password(trimmed);
            }
        }
        let mut map = self.read_fallback_map();
        if trimmed.is_empty() {
            map.remove(profile_id);
        } else {
            map.insert(profile_id.to_string(), trimmed.to_string());
        }
        self.write_fallback_map(&map);
    }

    pub fn get_key(&self, profile_id: &str) -> Option<String> {
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, profile_id) {
            if let Ok(pw) = entry.get_password() {
                if !pw.trim().is_empty() {
                    return Some(pw);
                }
            }
        }
        self.read_fallback_map()
            .get(profile_id)
            .cloned()
            .filter(|s| !s.trim().is_empty())
    }

    pub fn has_key(&self, profile_id: &str) -> bool {
        self.get_key(profile_id).is_some()
    }

    pub fn delete_key(&self, profile_id: &str) {
        self.set_key(profile_id, "");
    }
}

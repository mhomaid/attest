//! Pinned models, one per detection family.
//!
//! A registry maps a family name to a file, a sha256, and a feature-set
//! version. An unknown family uses `general`. A file whose hash does not match
//! the pin is rejected before it can score an event.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct ModelPin {
    pub path: PathBuf,
    pub sha256: String,
    pub feature_set: String,
    pub budget_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct RegistryFile {
    families: HashMap<String, ModelPin>,
}

#[derive(Debug, Clone)]
pub struct ModelRegistry {
    families: HashMap<String, ModelPin>,
}

impl ModelRegistry {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("read model registry {}", path.display()))?;
        let file: RegistryFile = serde_json::from_str(&raw)
            .with_context(|| format!("{} is not a model registry", path.display()))?;
        if !file.families.contains_key("general") {
            bail!("model registry is missing the general family");
        }
        Ok(Self {
            families: file.families,
        })
    }

    /// Family to score. Unknown names fall back to `general`.
    pub fn resolve(&self, family: &str) -> (String, &ModelPin) {
        if self.families.contains_key(family) {
            (family.to_string(), &self.families[family])
        } else {
            ("general".to_string(), &self.families["general"])
        }
    }

    /// Read the file and refuse it when the bytes do not match the pin.
    /// The returned family name and feature-set version are what an envelope records.
    pub fn load_bytes(&self, family: &str) -> Result<(String, Vec<u8>, String)> {
        let (name, pin) = self.resolve(family);
        let name = name.clone();
        let bytes =
            fs::read(&pin.path).with_context(|| format!("read model {}", pin.path.display()))?;
        let digest = hex::encode(Sha256::digest(&bytes));
        if digest != pin.sha256 {
            bail!(
                "model {name} hash mismatch: expected {} got {digest}",
                pin.sha256
            );
        }
        Ok((name.to_string(), bytes, pin.feature_set.clone()))
    }

    pub fn pin(&self, family: &str) -> &ModelPin {
        self.resolve(family).1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn write_registry(dir: &Path, general: &Path, auth: &Path) -> PathBuf {
        let general_bytes = fs::read(general).unwrap();
        let auth_bytes = fs::read(auth).unwrap();
        let g = hex::encode(Sha256::digest(general_bytes.as_slice()));
        let a = hex::encode(Sha256::digest(auth_bytes.as_slice()));
        let path = dir.join("models.json");
        fs::write(
            &path,
            format!(
                r#"{{"families":{{"general":{{"path":"{}","sha256":"{g}","feature_set":"v1","budget_ms":5}},"auth":{{"path":"{}","sha256":"{a}","feature_set":"v2","budget_ms":5}}}}}}"#,
                general.display(),
                auth.display()
            ),
        )
        .unwrap();
        path
    }

    #[test]
    fn unknown_family_falls_back_to_general() {
        let dir = tempfile::tempdir().unwrap();
        let general = dir.path().join("general.bin");
        let auth = dir.path().join("auth.bin");
        fs::write(&general, b"general-model".as_slice()).unwrap();
        fs::write(&auth, b"auth-model".as_slice()).unwrap();
        let registry = ModelRegistry::load(&write_registry(dir.path(), &general, &auth)).unwrap();
        assert_eq!(registry.resolve("missing").0, "general");
        assert_eq!(registry.resolve("auth").0, "auth");
    }

    #[test]
    fn swapped_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let general = dir.path().join("general.bin");
        let auth = dir.path().join("auth.bin");
        fs::write(&general, b"general-model".as_slice()).unwrap();
        fs::write(&auth, b"auth-model".as_slice()).unwrap();
        let registry = ModelRegistry::load(&write_registry(dir.path(), &general, &auth)).unwrap();
        fs::write(&general, b"swapped".as_slice()).unwrap();
        assert!(registry.load_bytes("general").is_err());
    }

    #[test]
    fn each_family_read_stays_inside_its_budget() {
        let dir = tempfile::tempdir().unwrap();
        let general = dir.path().join("general.bin");
        let auth = dir.path().join("auth.bin");
        fs::write(&general, b"general-model".as_slice()).unwrap();
        fs::write(&auth, b"auth-model".as_slice()).unwrap();
        let registry = ModelRegistry::load(&write_registry(dir.path(), &general, &auth)).unwrap();
        for family in ["general", "auth"] {
            let started = Instant::now();
            let mut samples = Vec::new();
            for _ in 0..50 {
                let t = Instant::now();
                registry.load_bytes(family).unwrap();
                samples.push(t.elapsed().as_secs_f64() * 1000.0);
            }
            samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let p99 = samples[(samples.len() * 99 / 100).min(samples.len() - 1)];
            let budget = registry.pin(family).budget_ms as f64;
            assert!(
                p99 < budget,
                "{family} p99 {p99} ms exceeded {budget} (wall {})",
                started.elapsed().as_secs_f64()
            );
        }
    }
}

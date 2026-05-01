//! Load and parse all `.heliql` rule files from a directory.

use anyhow::{Context, Result};
use attest_heliql::{parse, Detection};
use std::path::Path;
use tracing::{info, warn};

/// Scan `rules_dir` for `*.heliql` files and parse them all.
/// Files that fail to parse are logged and skipped (non-fatal).
pub fn load_rules(rules_dir: &Path) -> Result<Vec<Detection>> {
    let mut detections = Vec::new();

    let entries = std::fs::read_dir(rules_dir)
        .with_context(|| format!("cannot read rules dir: {}", rules_dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("heliql") {
            continue;
        }

        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read {}", path.display()))?;

        match parse(&source) {
            Ok(mut parsed) => {
                info!(
                    file = %path.display(),
                    count = parsed.len(),
                    "loaded detections"
                );
                detections.append(&mut parsed);
            }
            Err(e) => {
                warn!(file = %path.display(), error = %e, "skipping rule with parse error");
            }
        }
    }

    Ok(detections)
}

//! `ATTEST_DEMO_MODE` is opt-in. Unset, empty, and any other value are production.

/// Production unless the variable is `1`, `true`, `on`, or `yes`.
pub fn enabled() -> bool {
    match std::env::var("ATTEST_DEMO_MODE") {
        Ok(value) => enabled_value(Some(value.as_str())),
        Err(_) => false,
    }
}

pub fn enabled_value(value: Option<&str>) -> bool {
    matches!(
        value.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("1") | Some("true") | Some("on") | Some("yes")
    )
}

pub fn is_stub_source(source: &str) -> bool {
    source.ends_with("_stub")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_or_empty_is_production() {
        assert!(!enabled_value(None));
        assert!(!enabled_value(Some("")));
        assert!(!enabled_value(Some("0")));
        assert!(!enabled_value(Some("off")));
        assert!(enabled_value(Some("1")));
        assert!(enabled_value(Some("true")));
        assert!(enabled_value(Some(" ON ")));
    }
}

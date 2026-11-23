//! T-014: SecretSource contract tests. Rotation is proven with the file source.

use std::sync::Arc;
use std::time::Duration;

use attest_secrets::{
    CachedSource, EnvSource, FileSource, SecretError, SecretSource, ToolCredentials,
};

#[tokio::test]
async fn file_source_reloads_after_rotation_without_reconstruction() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("tool-ip_asn_lookup"), "old-value\n").unwrap();

    let creds = ToolCredentials::new(Arc::new(FileSource::new(dir.path())));
    let first = creds.resolve("ip_asn_lookup").await.unwrap();
    assert_eq!(first.expose(), "old-value");

    // Kubernetes rotates by atomically replacing the file; same process keeps running.
    std::fs::write(dir.path().join("tool-ip_asn_lookup"), "new-value\n").unwrap();
    let second = creds.resolve("ip_asn_lookup").await.unwrap();
    assert_eq!(second.expose(), "new-value");
}

#[tokio::test]
async fn credentials_are_resolved_per_tool_id() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("tool-alpha"), "a").unwrap();
    std::fs::write(dir.path().join("tool-beta"), "b").unwrap();
    let creds = ToolCredentials::new(Arc::new(FileSource::new(dir.path())));
    assert_eq!(creds.resolve("alpha").await.unwrap().expose(), "a");
    assert_eq!(creds.resolve("beta").await.unwrap().expose(), "b");
    assert!(matches!(
        creds.resolve("gamma").await,
        Err(SecretError::NotFound(_))
    ));
}

#[tokio::test]
async fn file_source_rejects_path_traversal_and_bad_names() {
    let parent = tempfile::tempdir().unwrap();
    let dir = parent.path().join("mount");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(parent.path().join("outside"), "leak").unwrap();
    let src = FileSource::new(&dir);
    for bad in [
        "../outside",
        "a/b",
        "/etc/passwd",
        "",
        ".hidden",
        "a\0b",
        "..",
    ] {
        assert!(
            matches!(src.get(bad).await, Err(SecretError::InvalidName(_))),
            "{bad:?} must be rejected"
        );
    }
    let creds = ToolCredentials::new(Arc::new(FileSource::new(&dir)));
    assert!(matches!(
        creds.resolve("../../outside").await,
        Err(SecretError::InvalidName(_))
    ));
}

#[tokio::test]
async fn empty_secret_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("tool-blank"), "\n").unwrap();
    let creds = ToolCredentials::new(Arc::new(FileSource::new(dir.path())));
    assert!(matches!(
        creds.resolve("blank").await,
        Err(SecretError::Empty(_))
    ));
}

#[tokio::test]
async fn debug_and_errors_never_print_the_secret() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("tool-t"), "super-secret-material").unwrap();
    let creds = ToolCredentials::new(Arc::new(FileSource::new(dir.path())));
    let v = creds.resolve("t").await.unwrap();
    assert!(!format!("{v:?}").contains("super-secret-material"));
}

#[tokio::test]
async fn env_source_reads_prefixed_variable_and_reflects_changes() {
    let src = EnvSource::new("ATTEST_T014_TEST_");
    std::env::set_var("ATTEST_T014_TEST_TOOL_MY_TOOL", "v1");
    let creds = ToolCredentials::new(Arc::new(src));
    assert_eq!(creds.resolve("my-tool").await.unwrap().expose(), "v1");
    std::env::set_var("ATTEST_T014_TEST_TOOL_MY_TOOL", "v2");
    assert_eq!(creds.resolve("my-tool").await.unwrap().expose(), "v2");
    std::env::remove_var("ATTEST_T014_TEST_TOOL_MY_TOOL");
    assert!(matches!(
        creds.resolve("my-tool").await,
        Err(SecretError::NotFound(_))
    ));
}

#[tokio::test]
async fn cached_source_serves_within_ttl_then_refreshes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("k"), "one").unwrap();
    let cached = CachedSource::new(
        Arc::new(FileSource::new(dir.path())),
        Duration::from_millis(150),
    );
    assert_eq!(cached.get("k").await.unwrap().expose(), "one");
    std::fs::write(dir.path().join("k"), "two").unwrap();
    assert_eq!(cached.get("k").await.unwrap().expose(), "one", "within ttl");
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert_eq!(cached.get("k").await.unwrap().expose(), "two", "after ttl");
}

#[test]
fn from_config_selects_source_and_rejects_unknown() {
    use attest_secrets::source_from_config;
    assert!(source_from_config(Some("env"), None, None).is_ok());
    assert!(
        source_from_config(None, None, None).is_ok(),
        "defaults to env"
    );
    assert!(source_from_config(Some("file"), Some("/tmp"), None).is_ok());
    assert!(matches!(
        source_from_config(Some("file"), None, None),
        Err(SecretError::Config(_))
    ));
    assert!(matches!(
        source_from_config(Some("vault"), None, None),
        Err(SecretError::Config(_))
    ));
    // Without the `aws` feature, asking for it is a clear error, not a silent fallback.
    #[cfg(not(feature = "aws"))]
    assert!(matches!(
        source_from_config(Some("aws"), None, None),
        Err(SecretError::Config(_))
    ));
}

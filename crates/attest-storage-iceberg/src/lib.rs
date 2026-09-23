//! Iceberg warm-tier writer: Kafka events → Parquet data files + catalog snapshots.

pub const CRATE_NAME: &str = "attest-storage-iceberg";

mod catalog;
mod schema;
mod storage;
mod writer;

pub use catalog::{open_or_create_table, Warehouse};
pub use writer::{FlatEvent, IcebergBatchWriter};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_crate_name() {
        assert_eq!(CRATE_NAME, "attest-storage-iceberg");
    }

    #[tokio::test]
    async fn write_batch_commits_iceberg_snapshot() {
        let dir = tempfile::tempdir().expect("tempdir");
        let warehouse = Warehouse::local_fs(dir.path()).expect("warehouse");
        let open = open_or_create_table(warehouse, "cloudtrail")
            .await
            .expect("create table");
        let writer = IcebergBatchWriter::from_open(open);

        let events = vec![
            FlatEvent {
                event_id: "e1".into(),
                class_uid: Some("3002".into()),
                time: Some("2026-09-23T12:00:00Z".into()),
                tenant_id: Some("demo".into()),
                actor_user_name: Some("alice".into()),
                actor_user_uid: None,
                cloud_region: Some("us-east-1".into()),
                cloud_account_uid: Some("111".into()),
                severity: Some("informational".into()),
                auth_status: Some("Success".into()),
                api_operation: Some("ConsoleLogin".into()),
                api_service: Some("signin.amazonaws.com".into()),
                raw: None,
            },
            FlatEvent {
                event_id: "e2".into(),
                class_uid: Some("3002".into()),
                time: Some("2026-09-23T12:01:00Z".into()),
                tenant_id: Some("demo".into()),
                actor_user_name: Some("bob".into()),
                actor_user_uid: None,
                cloud_region: Some("eu-west-1".into()),
                cloud_account_uid: Some("111".into()),
                severity: Some("medium".into()),
                auth_status: Some("Failure".into()),
                api_operation: Some("ConsoleLogin".into()),
                api_service: Some("signin.amazonaws.com".into()),
                raw: None,
            },
        ];

        let first = writer.write_batch(&events).await.expect("first commit");
        assert_eq!(first.records, 2);
        assert!(first.snapshot_id != 0);
        assert!(first.data_path.contains(".parquet"));

        let second = writer
            .write_batch(&events[..1])
            .await
            .expect("second commit");
        assert_eq!(second.records, 1);
        assert_ne!(first.snapshot_id, second.snapshot_id);
        assert_eq!(writer.snapshot_count().await, 2);
    }

    #[tokio::test]
    async fn restart_reloads_table_from_version_hint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let warehouse = Warehouse::local_fs(dir.path()).expect("warehouse");
        let open = open_or_create_table(warehouse, "cloudtrail")
            .await
            .expect("create");
        let writer = IcebergBatchWriter::from_open(open);
        let event = FlatEvent {
            event_id: "e-restart".into(),
            class_uid: None,
            time: Some("2026-09-23T15:00:00Z".into()),
            tenant_id: Some("demo".into()),
            actor_user_name: None,
            actor_user_uid: None,
            cloud_region: None,
            cloud_account_uid: None,
            severity: None,
            auth_status: None,
            api_operation: None,
            api_service: None,
            raw: None,
        };
        writer.write_batch(&[event]).await.expect("commit");

        let warehouse2 = Warehouse::local_fs(dir.path()).expect("warehouse2");
        let reopened = open_or_create_table(warehouse2, "cloudtrail")
            .await
            .expect("reopen");
        assert!(
            reopened.table.metadata().current_snapshot().is_some(),
            "reopened table should still have the committed snapshot"
        );
        assert_eq!(reopened.table.metadata().snapshots().count(), 1);
    }
}

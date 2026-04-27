use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use std::sync::Arc;

/// Arrow schema that mirrors the FlatEvent / cloudtrail_events columns.
pub fn cloudtrail_arrow_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("event_id", DataType::Utf8, false),
        Field::new("class_uid", DataType::Utf8, true),
        Field::new(
            "time",
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            true,
        ),
        Field::new("tenant_id", DataType::Utf8, true),
        Field::new("actor_user_name", DataType::Utf8, true),
        Field::new("actor_user_uid", DataType::Utf8, true),
        Field::new("cloud_region", DataType::Utf8, true),
        Field::new("cloud_account_uid", DataType::Utf8, true),
        Field::new("severity", DataType::Utf8, true),
        Field::new("auth_status", DataType::Utf8, true),
        Field::new("api_operation", DataType::Utf8, true),
        Field::new("api_service", DataType::Utf8, true),
        Field::new("raw", DataType::Utf8, true),
    ]))
}

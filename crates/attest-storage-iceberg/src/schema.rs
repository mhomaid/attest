use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use iceberg::spec::{NestedField, PrimitiveType, Schema as IcebergSchema, Type};
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

/// Iceberg schema for the `cloudtrail` warm table (same columns as Arrow).
pub fn cloudtrail_iceberg_schema() -> anyhow::Result<IcebergSchema> {
    IcebergSchema::builder()
        .with_schema_id(1)
        .with_fields(vec![
            NestedField::required(1, "event_id", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::optional(2, "class_uid", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::optional(3, "time", Type::Primitive(PrimitiveType::Timestamptz)).into(),
            NestedField::optional(4, "tenant_id", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::optional(5, "actor_user_name", Type::Primitive(PrimitiveType::String))
                .into(),
            NestedField::optional(6, "actor_user_uid", Type::Primitive(PrimitiveType::String))
                .into(),
            NestedField::optional(7, "cloud_region", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::optional(
                8,
                "cloud_account_uid",
                Type::Primitive(PrimitiveType::String),
            )
            .into(),
            NestedField::optional(9, "severity", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::optional(10, "auth_status", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::optional(11, "api_operation", Type::Primitive(PrimitiveType::String))
                .into(),
            NestedField::optional(12, "api_service", Type::Primitive(PrimitiveType::String)).into(),
            NestedField::optional(13, "raw", Type::Primitive(PrimitiveType::String)).into(),
        ])
        .build()
        .map_err(|e| anyhow::anyhow!("iceberg schema: {e}"))
}

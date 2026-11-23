//! Named, parameterized entity lookups.
//!
//! The agent picks a query name and supplies parameter values. SQL comes only
//! from the operator's YAML. Values are bound to placeholders and are never
//! written into the statement. Free-form SQL is rejected.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

const MAX_ROW_CAP: usize = 1_000;
const MAX_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Postgres,
    Mysql,
}

#[derive(Debug, Clone)]
struct QueryTemplate {
    sql: String,
    params: Vec<String>,
    columns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Catalog {
    dialect: Dialect,
    row_cap: usize,
    timeout: Duration,
    queries: BTreeMap<String, QueryTemplate>,
}

#[derive(Debug, Clone)]
pub struct BoundQuery {
    pub name: String,
    pub sql: String,
    pub params: Vec<String>,
    pub columns: Vec<String>,
    pub row_cap: usize,
    pub timeout: Duration,
    pub dialect: Dialect,
}

#[derive(Debug, Clone)]
pub struct EntityRow {
    pub cells: Vec<(String, Value)>,
}

#[derive(Debug, Deserialize)]
struct CatalogFile {
    #[serde(default)]
    dialect: String,
    #[serde(default)]
    row_cap: Option<usize>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    columns: Vec<String>,
    #[serde(default)]
    queries: BTreeMap<String, QueryFile>,
}

#[derive(Debug, Deserialize)]
struct QueryFile {
    sql: String,
    #[serde(default)]
    params: Vec<String>,
}

impl Catalog {
    pub fn parse(yaml: &str) -> Result<Self, String> {
        let file: CatalogFile =
            serde_yaml::from_str(yaml).map_err(|e| format!("invalid entity query file: {e}"))?;
        let dialect = match file.dialect.trim().to_ascii_lowercase().as_str() {
            "" | "postgres" | "postgresql" => Dialect::Postgres,
            "mysql" | "mariadb" => Dialect::Mysql,
            other => return Err(format!("unsupported entity dialect {other}")),
        };
        let row_cap = file.row_cap.unwrap_or(100);
        if row_cap == 0 || row_cap > MAX_ROW_CAP {
            return Err(format!("entity row cap must be 1..={MAX_ROW_CAP}"));
        }
        let timeout_ms = file.timeout_ms.unwrap_or(5_000);
        if timeout_ms == 0 || timeout_ms > MAX_TIMEOUT_MS {
            return Err(format!("entity timeout must be 1..={MAX_TIMEOUT_MS} ms"));
        }
        if file.columns.is_empty() {
            return Err("entity column allow list is empty".into());
        }
        let columns: BTreeSet<String> = file.columns.into_iter().collect();
        for column in &columns {
            if !is_ident(column) {
                return Err(format!("column {column} is not a plain identifier"));
            }
        }
        if file.queries.is_empty() {
            return Err("entity query file has no queries".into());
        }
        let mut queries = BTreeMap::new();
        for (name, query) in file.queries {
            if !is_ident(&name) {
                return Err(format!(
                    "entity query name {name} is not a plain identifier"
                ));
            }
            for param in &query.params {
                if !is_ident(param) {
                    return Err(format!(
                        "entity parameter {param} is not a plain identifier"
                    ));
                }
            }
            let sql_columns = validate_sql(&query.sql, dialect, query.params.len(), &columns)?;
            queries.insert(
                name,
                QueryTemplate {
                    sql: query.sql.trim().to_string(),
                    params: query.params,
                    columns: sql_columns,
                },
            );
        }
        Ok(Self {
            dialect,
            row_cap,
            timeout: Duration::from_millis(timeout_ms),
            queries,
        })
    }

    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    pub fn bind(&self, args: &Value) -> Result<BoundQuery, String> {
        if args.get("sql").is_some() || args.get("statement").is_some() {
            return Err("entity_lookup does not accept free-form sql".into());
        }
        let name = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "entity_lookup requires a named query".to_string())?;
        let template = self
            .queries
            .get(name)
            .ok_or_else(|| format!("unknown entity query {name}"))?;
        let provided = args.get("params").cloned().unwrap_or_else(|| json!({}));
        let provided = provided
            .as_object()
            .ok_or_else(|| "entity_lookup params must be an object".to_string())?;
        for key in provided.keys() {
            if !template.params.iter().any(|p| p == key) {
                return Err(format!("unknown entity parameter {key}"));
            }
        }
        let mut params = Vec::with_capacity(template.params.len());
        for param in &template.params {
            let value = provided
                .get(param)
                .ok_or_else(|| format!("missing entity parameter {param}"))?;
            params.push(bind_value(param, value)?);
        }
        Ok(BoundQuery {
            name: name.to_string(),
            sql: template.sql.clone(),
            params,
            columns: template.columns.clone(),
            row_cap: self.row_cap,
            timeout: self.timeout,
            dialect: self.dialect,
        })
    }
}

fn bind_value(name: &str, value: &Value) -> Result<String, String> {
    match value {
        Value::String(text) => Ok(text.clone()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Null => Err(format!("entity parameter {name} is null")),
        _ => Err(format!(
            "entity parameter {name} must be a string, number, or bool"
        )),
    }
}

pub async fn execute<F, Fut>(catalog: &Catalog, args: &Value, query: F) -> Result<Value, String>
where
    F: FnOnce(BoundQuery) -> Fut,
    Fut: Future<Output = Result<Vec<EntityRow>, String>>,
{
    let bound = catalog.bind(args)?;
    let name = bound.name.clone();
    let columns = bound.columns.clone();
    let row_cap = bound.row_cap;
    let timeout = bound.timeout;
    let fetched = tokio::time::timeout(timeout, query(bound))
        .await
        .map_err(|_| format!("entity lookup timed out after {}ms", timeout.as_millis()))??;
    Ok(project(&name, &columns, row_cap, fetched))
}

pub fn project(name: &str, columns: &[String], row_cap: usize, rows: Vec<EntityRow>) -> Value {
    let truncated = rows.len() > row_cap;
    let mut out = Vec::new();
    for row in rows.into_iter().take(row_cap) {
        let mut obj = Map::new();
        for (column, value) in row.cells {
            if !columns.iter().any(|c| c == &column) {
                continue;
            }
            obj.insert(column, redact_value(&value));
        }
        out.push(Value::Object(obj));
    }
    json!({
        "query": name,
        "rows": out,
        "row_count": out.len(),
        "truncated": truncated,
        "source": "entity_lookup"
    })
}

pub fn session_preamble(dialect: Dialect, timeout: Duration) -> Vec<String> {
    let ms = u64::try_from(timeout.as_millis()).unwrap_or(MAX_TIMEOUT_MS);
    match dialect {
        Dialect::Postgres => vec![
            "SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY".into(),
            format!("SET statement_timeout = {ms}"),
        ],
        Dialect::Mysql => vec![
            "SET SESSION TRANSACTION READ ONLY".into(),
            format!("SET SESSION MAX_EXECUTION_TIME = {ms}"),
        ],
    }
}

pub fn limited_sql(sql: &str, row_cap: usize) -> String {
    format!(
        "SELECT * FROM ({}) AS attest_entity_q LIMIT {}",
        sql.trim(),
        row_cap.saturating_add(1)
    )
}

pub async fn query_postgres(url: String, bound: BoundQuery) -> Result<Vec<EntityRow>, String> {
    if bound.dialect != Dialect::Postgres {
        return Err("postgres executor received a non-postgres template".into());
    }
    if url.to_ascii_lowercase().starts_with("mysql:")
        || url.to_ascii_lowercase().starts_with("mariadb:")
    {
        return Err("entity database url does not match the postgres dialect".into());
    }
    let (client, connection) = tokio_postgres::connect(&url, tokio_postgres::NoTls)
        .await
        .map_err(|_| "entity database connection failed".to_string())?;
    tokio::spawn(async move {
        if connection.await.is_err() {
            tracing::warn!("entity database connection closed");
        }
    });
    for stmt in session_preamble(Dialect::Postgres, bound.timeout) {
        client
            .batch_execute(&stmt)
            .await
            .map_err(|_| "entity database session setup failed".to_string())?;
    }
    let sql = limited_sql(&bound.sql, bound.row_cap);
    let statement = client
        .prepare(&sql)
        .await
        .map_err(|_| "entity database prepare failed".to_string())?;
    let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = bound
        .params
        .iter()
        .map(|p| p as &(dyn tokio_postgres::types::ToSql + Sync))
        .collect();
    let rows = client
        .query(&statement, &params)
        .await
        .map_err(|_| "entity database query failed".to_string())?;
    Ok(rows.iter().map(row_to_entity).collect())
}

pub async fn query_mysql(_url: String, bound: BoundQuery) -> Result<Vec<EntityRow>, String> {
    let _ = bound;
    Err("mysql entity connections are not linked in this build".into())
}

fn row_to_entity(row: &tokio_postgres::Row) -> EntityRow {
    let mut cells = Vec::with_capacity(row.len());
    for idx in 0..row.len() {
        let Some(column) = row.columns().get(idx) else {
            continue;
        };
        cells.push((column.name().to_string(), cell_value(row, idx)));
    }
    EntityRow { cells }
}

fn cell_value(row: &tokio_postgres::Row, idx: usize) -> Value {
    match row.columns()[idx].type_().name() {
        "int2" => optional_i64(
            row.try_get::<_, Option<i16>>(idx)
                .ok()
                .flatten()
                .map(i64::from),
        ),
        "int4" => optional_i64(
            row.try_get::<_, Option<i32>>(idx)
                .ok()
                .flatten()
                .map(i64::from),
        ),
        "int8" => optional_i64(row.try_get::<_, Option<i64>>(idx).ok().flatten()),
        "bool" => match row.try_get::<_, Option<bool>>(idx) {
            Ok(Some(v)) => Value::Bool(v),
            _ => Value::Null,
        },
        "float4" => optional_f64(
            row.try_get::<_, Option<f32>>(idx)
                .ok()
                .flatten()
                .map(f64::from),
        ),
        "float8" => optional_f64(row.try_get::<_, Option<f64>>(idx).ok().flatten()),
        _ => match row.try_get::<_, Option<String>>(idx) {
            Ok(Some(v)) => Value::String(v),
            _ => Value::Null,
        },
    }
}

fn optional_i64(value: Option<i64>) -> Value {
    value
        .map(|n| Value::Number(n.into()))
        .unwrap_or(Value::Null)
}

fn optional_f64(value: Option<f64>) -> Value {
    value
        .and_then(serde_json::Number::from_f64)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn validate_sql(
    sql: &str,
    dialect: Dialect,
    param_count: usize,
    allow: &BTreeSet<String>,
) -> Result<Vec<String>, String> {
    let trimmed = sql.trim();
    if !starts_with_select(trimmed) {
        return Err("entity query must be a single SELECT".into());
    }
    if trimmed.contains(';') {
        return Err("entity query rejects stacked statements".into());
    }
    if trimmed.contains('{') || trimmed.contains('}') {
        return Err("entity query parameters must be bound placeholders, not interpolated".into());
    }
    for word in [
        "insert", "update", "delete", "drop", "alter", "copy", "grant", "revoke", "union", "into",
        "truncate", "create", "attach", "pragma", "call", "execute", "set",
    ] {
        if contains_word(trimmed, word) {
            return Err(format!("entity query must be read-only (found {word})"));
        }
    }
    if trimmed.contains('*') {
        return Err("entity query must name columns; star selects are refused".into());
    }
    match dialect {
        Dialect::Postgres => check_postgres_placeholders(trimmed, param_count)?,
        Dialect::Mysql => check_mysql_placeholders(trimmed, param_count)?,
    }
    let columns = select_columns(trimmed)?;
    for column in &columns {
        if !allow.contains(column) {
            return Err(format!("column {column} is not in the allow list"));
        }
    }
    Ok(columns)
}

fn check_postgres_placeholders(sql: &str, param_count: usize) -> Result<(), String> {
    for i in 1..=param_count {
        let token = format!("${i}");
        if !sql.contains(&token) {
            return Err(format!("entity query is missing {token}"));
        }
        if placeholder_is_quoted(sql, &token) {
            return Err(format!(
                "entity query quotes {token}; parameters must be bound"
            ));
        }
    }
    let extra = format!("${}", param_count + 1);
    if sql.contains(&extra) {
        return Err("entity query has more placeholders than parameters".into());
    }
    if param_count == 0 && sql.contains('$') {
        return Err("entity query has more placeholders than parameters".into());
    }
    Ok(())
}

fn check_mysql_placeholders(sql: &str, param_count: usize) -> Result<(), String> {
    let n = sql.matches('?').count();
    if n != param_count {
        return Err(format!(
            "entity query has {n} placeholders and {param_count} parameters"
        ));
    }
    if placeholder_is_quoted(sql, "?") {
        return Err("entity query quotes a placeholder; parameters must be bound".into());
    }
    Ok(())
}

fn placeholder_is_quoted(sql: &str, token: &str) -> bool {
    sql.match_indices(token)
        .any(|(i, _)| i > 0 && sql.as_bytes()[i - 1] == b'\'')
}

fn select_columns(sql: &str) -> Result<Vec<String>, String> {
    let (list, _) = split_select_from(sql)?;
    let list = list.trim();
    if !starts_with_select(list) {
        return Err("entity query must be a single SELECT".into());
    }
    let list = list[6..].trim();
    if list.is_empty() {
        return Err("entity query select list is empty".into());
    }
    let mut columns = Vec::new();
    for part in list.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err("entity query has an empty select item".into());
        }
        let tokens: Vec<&str> = part.split_whitespace().collect();
        let name = if let Some(i) = tokens.iter().position(|t| t.eq_ignore_ascii_case("as")) {
            tokens
                .get(i + 1)
                .copied()
                .ok_or_else(|| "entity query alias is missing a name".to_string())?
        } else {
            tokens
                .last()
                .copied()
                .ok_or_else(|| "entity query select item has no column".to_string())?
        };
        let name = name.trim_matches('"').trim_matches('`');
        let name = name.rsplit('.').next().unwrap_or(name);
        if !is_ident(name) {
            return Err(format!("column {name} is not a plain identifier"));
        }
        columns.push(name.to_string());
    }
    Ok(columns)
}

fn split_select_from(sql: &str) -> Result<(&str, &str), String> {
    let lower = sql.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if bytes[i..].starts_with(b"from") {
            let before_ok = i == 0 || bytes[i - 1].is_ascii_whitespace();
            let after = i + 4;
            let after_ok = after == bytes.len() || bytes[after].is_ascii_whitespace();
            if before_ok && after_ok && i > 6 {
                return Ok((sql[..i].trim(), sql[after..].trim()));
            }
        }
        i += 1;
    }
    Err("entity query is missing FROM".into())
}

fn starts_with_select(sql: &str) -> bool {
    let sql = sql.trim_start();
    sql.len() >= 6
        && sql[..6].eq_ignore_ascii_case("select")
        && (sql.len() == 6 || sql.as_bytes()[6].is_ascii_whitespace())
}

fn contains_word(sql: &str, word: &str) -> bool {
    let lower = sql.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let word = word.as_bytes();
    let mut i = 0;
    while i + word.len() <= bytes.len() {
        if bytes[i..].starts_with(word) {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after = i + word.len();
            let after_ok = after == bytes.len() || !is_ident_byte(bytes[after]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

fn is_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') && name.len() <= 64
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn redact_value(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(redact_text(text)),
        other => other.clone(),
    }
}

fn redact_text(input: &str) -> String {
    let mut out = String::new();
    for (n, token) in input.split_whitespace().enumerate() {
        if n > 0 {
            out.push(' ');
        }
        if token.contains('@') && token.contains('.') {
            out.push_str(&hash_text(token.trim_matches(|c: char| {
                !c.is_ascii_alphanumeric() && c != '@' && c != '.'
            })));
        } else if is_card_like(token) {
            out.push_str("[redacted-card]");
        } else {
            out.push_str(token);
        }
    }
    if out.is_empty() {
        input.to_string()
    } else {
        out
    }
}

fn is_card_like(token: &str) -> bool {
    let digits: String = token.chars().filter(|c| c.is_ascii_digit()).collect();
    (13..=19).contains(&digits.len()) && digits.len() == token.chars().filter(|c| *c != '-').count()
}

fn hash_text(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    format!("sha256:{}", hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = r#"
dialect: postgres
row_cap: 2
timeout_ms: 1500
columns:
  - tenant_id
  - store_id
  - owner
  - note
queries:
  store_owner:
    sql: "SELECT tenant_id, store_id, owner, note FROM entities WHERE store_id = $1 AND tenant_id = $2"
    params:
      - store_id
      - tenant_id
"#;

    fn catalog() -> Catalog {
        Catalog::parse(YAML).unwrap()
    }

    fn args(store: &str) -> Value {
        json!({"query": "store_owner", "params": {"store_id": store, "tenant_id": "tenant-a"}})
    }

    #[test]
    fn free_form_sql_is_rejected() {
        let catalog = catalog();
        let err = catalog
            .bind(&json!({"sql": "SELECT password FROM users"}))
            .unwrap_err();
        assert!(err.contains("free-form"), "{err}");
        let err = catalog
            .bind(&json!({
                "query": "store_owner",
                "sql": "DROP TABLE entities",
                "params": {"store_id": "1001", "tenant_id": "tenant-a"}
            }))
            .unwrap_err();
        assert!(err.contains("free-form"), "{err}");
        let err = catalog
            .bind(&json!({"query": "SELECT password FROM users"}))
            .unwrap_err();
        assert!(err.contains("unknown entity query"), "{err}");
        let err = Catalog::parse(
            r#"
columns: [owner]
queries:
  bad:
    sql: "SELECT owner FROM entities WHERE store_id = $1; DROP TABLE entities"
    params: [store_id]
"#,
        )
        .unwrap_err();
        assert!(err.contains("stacked"), "{err}");
        let err = Catalog::parse(
            r#"
columns: [owner]
queries:
  bad:
    sql: "SELECT owner FROM entities WHERE store_id = $1 UNION SELECT owner FROM secrets"
    params: [store_id]
"#,
        )
        .unwrap_err();
        assert!(err.contains("union"), "{err}");
    }

    #[test]
    fn parameters_are_bound_not_interpolated() {
        let catalog = catalog();
        let attack = "1001'; DROP TABLE entities; --";
        let bound = catalog.bind(&args(attack)).unwrap();
        assert_eq!(
            bound.sql,
            "SELECT tenant_id, store_id, owner, note FROM entities WHERE store_id = $1 AND tenant_id = $2"
        );
        assert_eq!(
            bound.params,
            vec![attack.to_string(), "tenant-a".to_string()]
        );
        assert!(!bound.sql.to_ascii_lowercase().contains("drop"));
        assert!(bound.sql.contains("$1"));
        assert!(!bound.sql.contains(attack));
    }

    #[test]
    fn interpolated_template_is_refused() {
        let err = Catalog::parse(
            r#"
columns: [owner]
queries:
  bad:
    sql: "SELECT owner FROM entities WHERE store_id = '{store_id}'"
    params: [store_id]
"#,
        )
        .unwrap_err();
        assert!(err.contains("interpolated"), "{err}");
        let err = Catalog::parse(
            r#"
columns: [owner]
queries:
  bad:
    sql: "SELECT owner FROM entities WHERE store_id = '$1'"
    params: [store_id]
"#,
        )
        .unwrap_err();
        assert!(err.contains("quotes"), "{err}");
    }

    #[test]
    fn column_outside_the_allow_list_is_refused_at_load() {
        let err = Catalog::parse(
            r#"
columns: [owner]
queries:
  bad:
    sql: "SELECT password FROM entities WHERE store_id = $1"
    params: [store_id]
"#,
        )
        .unwrap_err();
        assert!(err.contains("not in the allow list"), "{err}");
        let err = Catalog::parse(
            r#"
columns: [owner]
queries:
  bad:
    sql: "SELECT * FROM entities WHERE store_id = $1"
    params: [store_id]
"#,
        )
        .unwrap_err();
        assert!(err.contains("star"), "{err}");
    }

    #[tokio::test]
    async fn injection_reaches_the_executor_as_a_bound_parameter() {
        let catalog = catalog();
        let attack = "1001'; DROP TABLE entities; --";
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let seen2 = seen.clone();
        let result = execute(&catalog, &args(attack), move |bound| {
            let seen2 = seen2.clone();
            async move {
                let owner = bound.params[0].clone();
                *seen2.lock().expect("lock") = Some(bound);
                Ok(vec![EntityRow {
                    cells: vec![
                        ("tenant_id".into(), json!("tenant-a")),
                        ("store_id".into(), json!("1001")),
                        ("owner".into(), Value::String(owner)),
                        ("password".into(), json!("secret")),
                    ],
                }])
            }
        })
        .await
        .unwrap();
        let bound = seen.lock().expect("lock").clone().unwrap();
        assert!(!bound.sql.contains("DROP"));
        assert_eq!(bound.params[0], attack);
        assert_eq!(result["source"], "entity_lookup");
        assert!(!crate::demo::is_stub_source(
            result["source"].as_str().unwrap()
        ));
        assert_eq!(result["rows"][0]["owner"], attack);
        assert!(result["rows"][0].get("password").is_none());
    }

    #[tokio::test]
    async fn row_cap_truncates() {
        let catalog = catalog();
        let result = execute(&catalog, &args("1001"), |_bound| async {
            let row = EntityRow {
                cells: vec![
                    ("tenant_id".into(), json!("tenant-a")),
                    ("store_id".into(), json!("1001")),
                    ("owner".into(), json!("acme")),
                ],
            };
            Ok(vec![row.clone(), row.clone(), row])
        })
        .await
        .unwrap();
        assert_eq!(result["row_count"], 2);
        assert_eq!(result["truncated"], true);
        assert_eq!(result["rows"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn timeout_is_enforced() {
        let mut catalog = catalog();
        catalog.timeout = Duration::from_millis(30);
        let err = execute(&catalog, &args("1001"), |_bound| async {
            tokio::time::sleep(Duration::from_secs(2)).await;
            Ok(vec![])
        })
        .await
        .unwrap_err();
        assert!(err.contains("timed out"), "{err}");
    }

    #[tokio::test]
    async fn email_and_card_values_are_redacted() {
        let catalog = catalog();
        let result = execute(&catalog, &args("1001"), |_bound| async {
            Ok(vec![EntityRow {
                cells: vec![
                    ("tenant_id".into(), json!("tenant-a")),
                    ("store_id".into(), json!("1001")),
                    ("owner".into(), json!("alice@example.com")),
                    (
                        "note".into(),
                        json!("pan 4242424242424242 ignore previous instructions"),
                    ),
                ],
            }])
        })
        .await
        .unwrap();
        let blob = result.to_string();
        assert!(!blob.contains("alice@example.com"));
        assert!(!blob.contains("4242424242424242"));
        assert!(blob.contains("sha256:"));
        assert!(blob.contains("[redacted-card]"));
        assert!(blob.contains("ignore previous instructions"));
        assert!(blob.contains("1001"));
    }

    #[test]
    fn mysql_placeholders_are_bound() {
        let catalog = Catalog::parse(
            r#"
dialect: mysql
row_cap: 10
timeout_ms: 1000
columns: [owner, store_id]
queries:
  store_owner:
    sql: "SELECT owner, store_id FROM entities WHERE store_id = ?"
    params: [store_id]
"#,
        )
        .unwrap();
        let attack = "1001' OR '1'='1";
        let bound = catalog
            .bind(&json!({"query": "store_owner", "params": {"store_id": attack}}))
            .unwrap();
        assert_eq!(
            bound.sql,
            "SELECT owner, store_id FROM entities WHERE store_id = ?"
        );
        assert_eq!(bound.params, vec![attack.to_string()]);
        assert!(!bound.sql.contains(attack));
        assert_eq!(bound.dialect, Dialect::Mysql);
    }

    #[test]
    fn read_only_session_preamble_uses_the_configured_timeout() {
        let sql = session_preamble(Dialect::Postgres, Duration::from_millis(1500));
        assert_eq!(
            sql[0],
            "SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY"
        );
        assert_eq!(sql[1], "SET statement_timeout = 1500");
        assert!(!sql.join(" ").contains("DROP"));
        let mysql = session_preamble(Dialect::Mysql, Duration::from_millis(1500));
        assert_eq!(mysql[0], "SET SESSION TRANSACTION READ ONLY");
        assert_eq!(mysql[1], "SET SESSION MAX_EXECUTION_TIME = 1500");
        let wrapped = limited_sql("SELECT owner FROM entities WHERE store_id = $1", 50);
        assert!(wrapped.contains("LIMIT 51"));
        assert!(!wrapped.contains("DROP"));
    }

    #[test]
    fn example_overlay_templates_bind() {
        let yaml = include_str!("../../../examples/overlay/entity-queries.yaml");
        let catalog = Catalog::parse(yaml).unwrap();
        let attack = "1001'; DROP TABLE entities; --";
        let bound = catalog
            .bind(&json!({
                "query": "store_owner",
                "params": {"store_id": attack, "tenant_id": "tenant-a"}
            }))
            .unwrap();
        assert!(bound.sql.contains("$1"));
        assert_eq!(bound.params[0], attack);
        assert!(!bound.sql.contains("DROP"));
    }
}

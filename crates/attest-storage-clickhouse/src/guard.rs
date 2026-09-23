//! Static validation for warm-tier SQL that arrives from outside the process — analysts in the
//! workbench hunt view and, more importantly, LLM agents via the MCP `query_warm_tier` tool,
//! where the SQL can be steered by attacker-controlled log fields.
//!
//! ClickHouse's own `readonly` setting (applied by the client) blocks writes. What it does not
//! block at `readonly=2` — which the warm tier needs for `s3()` — are table functions that make
//! ClickHouse open network connections or files (`url`, `remote`, `file`, `mysql`, …). This
//! guard rejects those, pins `s3()` to the configured warm bucket, and refuses per-query
//! `SETTINGS` overrides so the client's limits cannot be raised from inside the query.

/// Why a query was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Rejection {
    #[error("empty query")]
    Empty,
    #[error("only SELECT / WITH queries are permitted")]
    NotSelect,
    #[error("multiple statements are not permitted")]
    MultipleStatements,
    #[error("per-query SETTINGS are not permitted")]
    SettingsOverride,
    #[error("INTO OUTFILE is not permitted")]
    IntoOutfile,
    #[error("system tables are not queryable")]
    SystemTables,
    #[error("table function `{0}` is not permitted")]
    ForbiddenFunction(String),
    #[error("s3() may only read from {0}")]
    ForbiddenS3Target(String),
    #[error("unterminated string literal or comment")]
    Unterminated,
}

/// Table functions that reach outside the warm tier (network, filesystem, other databases,
/// processes). Lower-case; ClickHouse function names are case-insensitive.
const FORBIDDEN_FUNCTIONS: &[&str] = &[
    "url",
    "urlcluster",
    "remote",
    "remotesecure",
    "cluster",
    "clusterallreplicas",
    "file",
    "filecluster",
    "input",
    "executable",
    "mysql",
    "postgresql",
    "mongodb",
    "redis",
    "sqlite",
    "jdbc",
    "odbc",
    "hdfs",
    "hdfscluster",
    "s3cluster",
    "gcs",
    "azureblobstorage",
    "azureblobstoragecluster",
    "deltalake",
    "deltalakecluster",
    "hudi",
    "hudicluster",
    "iceberg",
    "icebergs3",
    "icebergazure",
    "iceberghdfs",
    "icebergcluster",
    "icebergs3cluster",
];

#[derive(Debug, Clone, PartialEq)]
enum Token {
    /// Bare or quoted identifier / keyword, lower-cased.
    Ident(String),
    /// Contents of a single-quoted string literal.
    Str(String),
    Punct(char),
}

fn tokenize(sql: &str) -> Result<Vec<Token>, Rejection> {
    let chars: Vec<char> = sql.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '-' && chars.get(i + 1) == Some(&'-') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            let end = (i + 2..chars.len().saturating_sub(1))
                .find(|&j| chars[j] == '*' && chars[j + 1] == '/')
                .ok_or(Rejection::Unterminated)?;
            i = end + 2;
        } else if c == '\'' || c == '"' || c == '`' {
            let mut text = String::new();
            let mut j = i + 1;
            loop {
                match chars.get(j) {
                    None => return Err(Rejection::Unterminated),
                    Some('\\') => {
                        if let Some(&next) = chars.get(j + 1) {
                            text.push(next);
                        }
                        j += 2;
                    }
                    Some(&q) if q == c => {
                        if chars.get(j + 1) == Some(&c) {
                            text.push(c);
                            j += 2;
                        } else {
                            break;
                        }
                    }
                    Some(&other) => {
                        text.push(other);
                        j += 1;
                    }
                }
            }
            out.push(if c == '\'' {
                Token::Str(text)
            } else {
                Token::Ident(text.to_lowercase())
            });
            i = j + 1;
        } else if c.is_alphanumeric() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Token::Ident(
                chars[start..i].iter().collect::<String>().to_lowercase(),
            ));
        } else {
            out.push(Token::Punct(c));
            i += 1;
        }
    }
    Ok(out)
}

/// Validate a warm-tier query. Returns the SQL with any trailing `;` removed, ready to have a
/// `FORMAT` clause appended.
pub fn validate_read_query<'a>(sql: &'a str, s3_prefix: &str) -> Result<&'a str, Rejection> {
    let sql = sql.trim().trim_end_matches(';').trim_end();
    let tokens = tokenize(sql)?;

    match tokens.first() {
        None => return Err(Rejection::Empty),
        Some(Token::Ident(k)) if k == "select" || k == "with" => {}
        Some(_) => return Err(Rejection::NotSelect),
    }

    for (i, tok) in tokens.iter().enumerate() {
        let next = tokens.get(i + 1);
        match tok {
            Token::Punct(';') => return Err(Rejection::MultipleStatements),
            Token::Ident(k) if k == "settings" => return Err(Rejection::SettingsOverride),
            Token::Ident(k) if k == "outfile" => return Err(Rejection::IntoOutfile),
            Token::Ident(k) if k == "system" && next == Some(&Token::Punct('.')) => {
                return Err(Rejection::SystemTables)
            }
            Token::Ident(name) if next == Some(&Token::Punct('(')) => {
                if FORBIDDEN_FUNCTIONS.contains(&name.as_str()) {
                    return Err(Rejection::ForbiddenFunction(name.clone()));
                }
                if name == "s3" {
                    match tokens.get(i + 2) {
                        Some(Token::Str(target)) if target.starts_with(s3_prefix) => {}
                        _ => return Err(Rejection::ForbiddenS3Target(s3_prefix.to_string())),
                    }
                }
            }
            _ => {}
        }
    }
    Ok(sql)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PREFIX: &str = "http://minio:9000/attest-warm/";

    fn ok(sql: &str) {
        assert!(
            validate_read_query(sql, PREFIX).is_ok(),
            "expected OK: {sql} -> {:?}",
            validate_read_query(sql, PREFIX)
        );
    }

    fn rejected(sql: &str, expected: Rejection) {
        assert_eq!(validate_read_query(sql, PREFIX), Err(expected), "{sql}");
    }

    #[test]
    fn accepts_queries_used_by_the_platform() {
        ok("SELECT 1 AS ok");
        ok("SELECT count() FROM cloudtrail_events");
        ok("select event_id, time from cloudtrail where event_id = 'abc' limit 1;");
        ok("SELECT count(*) FROM s3('http://minio:9000/attest-warm/cloudtrail/**/*.parquet', 'minioadmin', 'minioadmin', 'Parquet')");
        ok("WITH recent AS (SELECT * FROM cloudtrail_events) SELECT cloud_region, count() FROM recent GROUP BY cloud_region");
    }

    #[test]
    fn keywords_inside_literals_and_comments_are_ignored() {
        ok("SELECT 'url(http://x)', 'a;b', 'settings' FROM cloudtrail_events");
        ok("SELECT 1 -- ; url('http://x')\n");
        ok("SELECT /* file('x') */ 1");
        ok("SELECT 'it''s', 'back\\'slash' FROM cloudtrail_events");
    }

    #[test]
    fn rejects_non_select_and_multiple_statements() {
        rejected("", Rejection::Empty);
        rejected("  ;  ", Rejection::Empty);
        rejected("DROP TABLE cloudtrail_events", Rejection::NotSelect);
        rejected("INSERT INTO t SELECT 1", Rejection::NotSelect);
        rejected("SELECT 1; DROP TABLE t", Rejection::MultipleStatements);
    }

    #[test]
    fn rejects_network_and_file_table_functions_in_any_case() {
        rejected(
            "SELECT * FROM url('http://169.254.169.254/latest/meta-data', 'LineAsString')",
            Rejection::ForbiddenFunction("url".into()),
        );
        rejected(
            "SELECT * FROM URL('http://internal:8080', 'LineAsString')",
            Rejection::ForbiddenFunction("url".into()),
        );
        rejected(
            "SELECT * FROM remote('10.0.0.1', system.users)",
            Rejection::ForbiddenFunction("remote".into()),
        );
        rejected(
            "SELECT * FROM `file` ('/etc/passwd', 'LineAsString')",
            Rejection::ForbiddenFunction("file".into()),
        );
        rejected(
            "SELECT * FROM (SELECT * FROM mysql('db:3306', 'x', 'y', 'u', 'p'))",
            Rejection::ForbiddenFunction("mysql".into()),
        );
    }

    #[test]
    fn pins_s3_to_the_warm_bucket() {
        let err = Rejection::ForbiddenS3Target(PREFIX.into());
        rejected(
            "SELECT * FROM s3('http://attacker.example/x.parquet', 'Parquet')",
            err.clone(),
        );
        rejected(
            "SELECT * FROM s3('http://minio:9000/other-bucket/x', 'Parquet')",
            err.clone(),
        );
        rejected("SELECT * FROM s3(my_named_collection)", err);
    }

    #[test]
    fn rejects_limit_bypasses_and_system_tables() {
        rejected(
            "SELECT 1 SETTINGS max_execution_time = 0",
            Rejection::SettingsOverride,
        );
        rejected("SELECT 1 INTO OUTFILE '/tmp/x'", Rejection::IntoOutfile);
        rejected("SELECT * FROM system.users", Rejection::SystemTables);
        rejected("SELECT * FROM System . tables", Rejection::SystemTables);
    }

    #[test]
    fn rejects_unterminated_input() {
        rejected("SELECT 'abc", Rejection::Unterminated);
        rejected("SELECT 1 /* never closed", Rejection::Unterminated);
    }
}

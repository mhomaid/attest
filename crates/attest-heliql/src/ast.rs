use serde::{Deserialize, Serialize};

/// Severity level matching the OCSF severity vocabulary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Severity::Critical => "critical",
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
            Severity::Info => "info",
        };
        write!(f, "{s}")
    }
}

/// Execution target for a compiled rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Runtime {
    Stream,
    Batch,
    Federated,
}

/// Duration literal: value + unit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Duration {
    pub value: i64,
    pub unit: DurationUnit,
}

impl Duration {
    /// Convert to seconds (approximate — months not supported).
    pub fn to_seconds(&self) -> i64 {
        match self.unit {
            DurationUnit::Seconds => self.value,
            DurationUnit::Minutes => self.value * 60,
            DurationUnit::Hours => self.value * 3600,
            DurationUnit::Days => self.value * 86_400,
        }
    }

    /// Format as a RisingWave / Postgres INTERVAL string.
    pub fn to_interval_sql(&self) -> String {
        match self.unit {
            DurationUnit::Seconds => format!("INTERVAL '{} seconds'", self.value),
            DurationUnit::Minutes => format!("INTERVAL '{} minutes'", self.value),
            DurationUnit::Hours => format!("INTERVAL '{} hours'", self.value),
            DurationUnit::Days => format!("INTERVAL '{} days'", self.value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DurationUnit {
    Seconds,
    Minutes,
    Hours,
    Days,
}

/// A single value in a condition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Field(String),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Str(s) => write!(f, "'{s}'"),
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Field(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CmpOp {
    Eq,
    Ne,
    Gt,
    Lt,
    Gte,
    Lte,
}

impl std::fmt::Display for CmpOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CmpOp::Eq => "=",
            CmpOp::Ne => "!=",
            CmpOp::Gt => ">",
            CmpOp::Lt => "<",
            CmpOp::Gte => ">=",
            CmpOp::Lte => "<=",
        };
        write!(f, "{s}")
    }
}

/// A reference to an entity baseline window: `baseline(field, 90d)`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaselineRef {
    pub field: String,
    pub window: Duration,
}

/// The right-hand side of an IN / NOT IN expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InRhs {
    Baseline(BaselineRef),
    List(Vec<Value>),
}

/// A single boolean atom in a `where` or `condition` clause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConditionAtom {
    /// `field = value`
    Cmp {
        field: String,
        op: CmpOp,
        value: Value,
    },
    /// `field IN [values]` or `field IN baseline(...)`
    In { field: String, rhs: InRhs },
    /// `field NOT IN [values]` or `field NOT IN baseline(...)`
    NotIn { field: String, rhs: InRhs },
    /// `unique(field, window: 90d) > 0`
    Unique {
        field: String,
        window: Duration,
        op: CmpOp,
        threshold: i64,
    },
}

/// Combined boolean expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    And(Vec<ConditionAtom>),
    Or(Vec<ConditionAtom>),
    Single(ConditionAtom),
}

/// The parsed representation of a single HELIQL detection rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection {
    /// Unique rule identifier (snake_case).
    pub id: String,
    pub description: Option<String>,
    /// OCSF event class this rule applies to (e.g. `ocsf.authentication`).
    pub applies_to: Option<String>,
    /// The entity field used to scope baselines (e.g. `identity.user`).
    pub entity: Option<String>,
    /// Pre-filter conditions in `where:` block.
    pub where_conditions: Vec<Condition>,
    /// Detection-specific conditions in `condition:` block.
    pub conditions: Vec<Condition>,
    pub severity: Severity,
    pub mitre: Vec<String>,
    pub runtime: Vec<Runtime>,
}

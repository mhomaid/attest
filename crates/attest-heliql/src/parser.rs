use crate::ast::*;
use crate::error::HeliqlError;
use pest::Parser;
use pest::iterators::Pair;

#[derive(pest_derive::Parser)]
#[grammar = "src/grammar.pest"]
struct HeliqlParser;

/// Parse a HELIQL source string into a list of [`Detection`]s.
pub fn parse(source: &str) -> Result<Vec<Detection>, HeliqlError> {
    let file = HeliqlParser::parse(Rule::file, source)
        .map_err(|e| HeliqlError::ParseError(e.to_string()))?
        .next()
        .ok_or_else(|| HeliqlError::ParseError("empty file".into()))?;

    let mut detections = Vec::new();
    for pair in file.into_inner() {
        if pair.as_rule() == Rule::detection_block {
            detections.push(parse_detection_block(pair)?);
        }
    }
    Ok(detections)
}

/// Parse conditions from a `where_clause` or `condition_kw` pair.
/// Handles both layouts: clause → where_list → where_item*, or clause → where_item*.
fn parse_where_list(clause_pair: Pair<Rule>) -> Result<Vec<Condition>, HeliqlError> {
    let mut result = Vec::new();
    for child in clause_pair.into_inner() {
        match child.as_rule() {
            Rule::where_list => {
                for item in child.into_inner() {
                    if item.as_rule() == Rule::where_item {
                        let cond = parse_condition_expr(item.into_inner().next().unwrap())?;
                        result.push(cond);
                    }
                }
            }
            Rule::where_item => {
                let cond = parse_condition_expr(child.into_inner().next().unwrap())?;
                result.push(cond);
            }
            _ => {}
        }
    }
    Ok(result)
}

fn parse_detection_block(pair: Pair<Rule>) -> Result<Detection, HeliqlError> {
    let mut id = String::new();
    let mut description = None;
    let mut applies_to = None;
    let mut entity = None;
    let mut where_conditions = Vec::new();
    let mut conditions = Vec::new();
    let mut severity = Severity::Medium;
    let mut mitre = Vec::new();
    let mut runtime = vec![Runtime::Stream];

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::detection_id => {
                id = inner.into_inner().next().unwrap().as_str().to_string();
            }
            Rule::description => {
                let raw = inner.into_inner().next().unwrap().as_str();
                description = Some(strip_quotes(raw));
            }
            Rule::applies_to => {
                applies_to = Some(inner.into_inner().next().unwrap().as_str().to_string());
            }
            Rule::entity_clause => {
                entity = Some(inner.into_inner().next().unwrap().as_str().to_string());
            }
            Rule::where_clause => {
                where_conditions.extend(parse_where_list(inner)?);
            }
            Rule::condition_kw => {
                conditions.extend(parse_where_list(inner)?);
            }
            Rule::severity_kw => {
                severity = parse_severity(
                    inner.into_inner().next().unwrap().as_str(),
                )?;
            }
            Rule::mitre_kw => {
                for item in inner.into_inner() {
                    // mitre_kw → mitre_list → mitre_id*
                    let ids = if item.as_rule() == Rule::mitre_list {
                        item.into_inner().collect::<Vec<_>>()
                    } else {
                        vec![item]
                    };
                    for m in ids {
                        if m.as_rule() == Rule::mitre_id {
                            mitre.push(m.as_str().to_string());
                        }
                    }
                }
            }
            Rule::runtime_kw => {
                runtime = inner
                    .into_inner()
                    .flat_map(|p| parse_runtime(p.as_str()).ok())
                    .collect();
            }
            _ => {}
        }
    }

    Ok(Detection {
        id,
        description,
        applies_to,
        entity,
        where_conditions,
        conditions,
        severity,
        mitre,
        runtime,
    })
}

fn parse_condition_expr(pair: Pair<Rule>) -> Result<Condition, HeliqlError> {
    match pair.as_rule() {
        Rule::and_cond => {
            let atoms = pair
                .into_inner()
                .map(parse_bool_atom)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Condition::And(atoms))
        }
        Rule::or_cond => {
            let atoms = pair
                .into_inner()
                .map(parse_bool_atom)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Condition::Or(atoms))
        }
        Rule::bool_atom => Ok(Condition::Single(parse_bool_atom(pair)?)),
        Rule::condition_expr => {
            parse_condition_expr(pair.into_inner().next().unwrap())
        }
        other => Err(HeliqlError::ParseError(format!(
            "unexpected rule in condition_expr: {other:?}"
        ))),
    }
}

fn parse_bool_atom(pair: Pair<Rule>) -> Result<ConditionAtom, HeliqlError> {
    let inner = if pair.as_rule() == Rule::bool_atom {
        pair.into_inner().next().unwrap()
    } else {
        pair
    };

    match inner.as_rule() {
        Rule::cmp_expr => {
            let mut parts = inner.into_inner();
            let field = parts.next().unwrap().as_str().to_string();
            let op    = parse_cmp_op(parts.next().unwrap().as_str())?;
            let value = parse_value(parts.next().unwrap())?;
            Ok(ConditionAtom::Cmp { field, op, value })
        }
        Rule::in_expr => {
            let mut parts = inner.into_inner();
            let field = parts.next().unwrap().as_str().to_string();
            let rhs   = parse_in_rhs(parts.next().unwrap())?;
            Ok(ConditionAtom::In { field, rhs })
        }
        Rule::not_in_expr => {
            let mut parts = inner.into_inner();
            let field = parts.next().unwrap().as_str().to_string();
            let rhs   = parse_in_rhs(parts.next().unwrap())?;
            Ok(ConditionAtom::NotIn { field, rhs })
        }
        Rule::unique_expr => {
            let mut parts = inner.into_inner();
            let field     = parts.next().unwrap().as_str().to_string();
            let window    = parse_duration(parts.next().unwrap())?;
            let op        = parse_cmp_op(parts.next().unwrap().as_str())?;
            let threshold = parts.next().unwrap().as_str().parse::<i64>()
                .map_err(|e| HeliqlError::ParseError(e.to_string()))?;
            Ok(ConditionAtom::Unique { field, window, op, threshold })
        }
        other => Err(HeliqlError::ParseError(format!(
            "unexpected bool_atom rule: {other:?}"
        ))),
    }
}

fn parse_in_rhs(pair: Pair<Rule>) -> Result<InRhs, HeliqlError> {
    match pair.as_rule() {
        Rule::baseline_expr => {
            let mut parts = pair.into_inner();
            let field  = parts.next().unwrap().as_str().to_string();
            let window = parse_duration(parts.next().unwrap())?;
            Ok(InRhs::Baseline(BaselineRef { field, window }))
        }
        Rule::value_list => {
            let values = pair
                .into_inner()
                .map(parse_value)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(InRhs::List(values))
        }
        other => Err(HeliqlError::ParseError(format!(
            "unexpected in_rhs rule: {other:?}"
        ))),
    }
}

fn parse_duration(pair: Pair<Rule>) -> Result<Duration, HeliqlError> {
    let mut parts = pair.into_inner();
    let value = parts.next().unwrap().as_str().parse::<i64>()
        .map_err(|e| HeliqlError::ParseError(e.to_string()))?;
    let unit = match parts.next().unwrap().as_str() {
        "s" => DurationUnit::Seconds,
        "m" => DurationUnit::Minutes,
        "h" => DurationUnit::Hours,
        "d" => DurationUnit::Days,
        u   => return Err(HeliqlError::ParseError(format!("unknown duration unit: {u}"))),
    };
    Ok(Duration { value, unit })
}

fn parse_value(pair: Pair<Rule>) -> Result<Value, HeliqlError> {
    let inner = if pair.as_rule() == Rule::value {
        pair.into_inner().next().unwrap()
    } else {
        pair
    };
    match inner.as_rule() {
        Rule::string_lit => Ok(Value::Str(strip_quotes(inner.as_str()))),
        Rule::integer    => Ok(Value::Int(inner.as_str().parse().map_err(|e: std::num::ParseIntError| HeliqlError::ParseError(e.to_string()))?)),
        Rule::number     => Ok(Value::Float(inner.as_str().parse().map_err(|e: std::num::ParseFloatError| HeliqlError::ParseError(e.to_string()))?)),
        Rule::field_ref  => Ok(Value::Field(inner.as_str().to_string())),
        other => Err(HeliqlError::ParseError(format!("unexpected value rule: {other:?}"))),
    }
}

fn parse_cmp_op(s: &str) -> Result<CmpOp, HeliqlError> {
    match s {
        "="  => Ok(CmpOp::Eq),
        "!=" => Ok(CmpOp::Ne),
        ">"  => Ok(CmpOp::Gt),
        "<"  => Ok(CmpOp::Lt),
        ">=" => Ok(CmpOp::Gte),
        "<=" => Ok(CmpOp::Lte),
        other => Err(HeliqlError::ParseError(format!("unknown cmp_op: {other}"))),
    }
}

fn parse_severity(s: &str) -> Result<Severity, HeliqlError> {
    match s {
        "critical" => Ok(Severity::Critical),
        "high"     => Ok(Severity::High),
        "medium"   => Ok(Severity::Medium),
        "low"      => Ok(Severity::Low),
        "info"     => Ok(Severity::Info),
        other => Err(HeliqlError::ParseError(format!("unknown severity: {other}"))),
    }
}

fn parse_runtime(s: &str) -> Result<Runtime, HeliqlError> {
    match s.trim() {
        "stream"    => Ok(Runtime::Stream),
        "batch"     => Ok(Runtime::Batch),
        "federated" => Ok(Runtime::Federated),
        other => Err(HeliqlError::ParseError(format!("unknown runtime: {other}"))),
    }
}

fn strip_quotes(s: &str) -> String {
    if (s.starts_with('"') && s.ends_with('"'))
        || (s.starts_with('\'') && s.ends_with('\''))
    {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

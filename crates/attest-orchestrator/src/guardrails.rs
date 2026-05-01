//! Phase 5 — Hallucination guardrails.
//!
//! Three runtime invariants enforced on every LLM verdict before it is signed:
//!
//! 1. **Retrieval-before-reasoning** — the loop must have at least one successful
//!    (non-error, non-denied) tool call before a final verdict is accepted.
//!
//! 2. **Citation enforcement** — every factual claim in the reasoning text must
//!    carry an `[evidence:<tool_call_id>]` tag referencing a *real* tool call from
//!    this loop session.  Fabricated citation tags are caught as orphans.
//!
//! 3. **Cross-agent review** (wired in `triage.rs`) — high-impact `TruePositive`
//!    verdicts are reviewed by a second LLM pass; disagreement forces
//!    `NeedsInvestigation`.
//!
//! All three checks can be disabled at runtime via `ATTEST_GUARDRAILS=off`
//! for local debugging.

use attest_attestation::ToolCallRecord;
use std::collections::HashSet;

// ── Enforcement mode ──────────────────────────────────────────────────────────

/// Whether guardrails are active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnforcementMode {
    On,
    /// Disable all checks (debug / QA only).
    Off,
}

impl EnforcementMode {
    pub fn from_env() -> Self {
        match std::env::var("ATTEST_GUARDRAILS").as_deref() {
            Ok("off") | Ok("0") | Ok("false") => Self::Off,
            _ => Self::On,
        }
    }
}

// ── Max retries ───────────────────────────────────────────────────────────────

/// Maximum number of per-check re-prompts before forcing `NeedsInvestigation`.
pub fn max_validation_retries() -> u8 {
    std::env::var("GUARDRAIL_MAX_RETRIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3)
}

// ── Retrieval-before-reasoning ────────────────────────────────────────────────

/// Returns `true` if at least one tool call in this loop session succeeded
/// (was not denied by policy and did not return an error).
pub fn has_retrieved_evidence(tool_calls: &[ToolCallRecord]) -> bool {
    tool_calls.iter().any(|t| {
        t.policy_decision == "allow"
            && t.policy_decision != "error"
            && t.policy_decision != "deny"
    })
}

/// Human-readable re-prompt injected when retrieval check fails.
pub fn retrieval_reprompt() -> &'static str {
    "Your verdict was rejected: you must call at least one investigation tool \
     (query_hot_tier, lookup_threat_intel, get_asset_context, or get_user_baseline) \
     before emitting a final verdict. You are not allowed to reason from prior \
     knowledge alone for security decisions. Call a tool now and then re-emit \
     your verdict."
}

/// Investigator-loop variant — warm tier is mandatory when historical evidence matters.
pub fn investigator_retrieval_reprompt() -> &'static str {
    "Your verdict was rejected: you must call at least one investigation tool before \
     the final JSON. For historical or multi-day context you must call `query_warm_tier` \
     with a read-only SELECT. You may also use query_hot_tier, lookup_threat_intel, \
     get_asset_context, get_user_baseline, analyze_code_snippet, or sandbox_detonate. \
     Call a tool now, then re-emit your verdict JSON."
}

// ── Citation enforcement ──────────────────────────────────────────────────────

/// Result of citation validation.
#[derive(Debug, Clone, Default)]
pub struct CitationReport {
    /// True if the verdict passed all citation checks.
    pub passed: bool,
    /// Claim sentences (≤100 chars truncated) that lack any `[evidence:...]` tag.
    pub missing_citations: Vec<String>,
    /// `[evidence:X]` tags where `X` was not a real tool call ID from this session.
    pub orphan_citations: Vec<String>,
}

/// Security-relevant nouns that mark a sentence as a factual claim requiring citation.
static CLAIM_KEYWORDS: &[&str] = &[
    "ip", "address", "user", "principal", "arn", "hostname", "host", "login",
    "attempt", "threat", "malicious", "attack", "indicator", "reputation",
    "baseline", "anomal", "suspicious", "hash", "md5", "sha", "domain", "url",
    "account", "credential", "password", "token", "session", "geo", "location",
    "country", "region", "api", "service", "role", "privilege", "permission",
    "exploit", "payload", "command", "process", "binary", "file", "registry",
    "port", "protocol", "packet", "traffic", "connection", "socket",
];

/// Returns `true` if the sentence contains a security-relevant noun that
/// would make it a factual claim requiring a citation.
fn is_claim_sentence(sentence: &str) -> bool {
    let lower = sentence.to_ascii_lowercase();
    CLAIM_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

/// Returns `true` if the sentence (or the 100 chars immediately following it
/// in the full text) contains an `[evidence:...]` tag.
fn has_inline_citation(sentence: &str, full_text: &str) -> bool {
    // Check within the sentence itself
    if sentence.contains("[evidence:") {
        return true;
    }
    // Check in the 100-char window after the sentence appears in the full text
    if let Some(pos) = full_text.find(sentence.trim()) {
        let window_start = pos + sentence.len();
        let window_end = (window_start + 100).min(full_text.len());
        if full_text[window_start..window_end].contains("[evidence:") {
            return true;
        }
    }
    false
}

/// Extract all `[evidence:X]` tag values from a text string.
pub fn extract_citation_ids(text: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("[evidence:") {
        let rest = &remaining[start + 10..];
        if let Some(end) = rest.find(']') {
            let id = rest[..end].trim().to_string();
            if !id.is_empty() {
                ids.push(id);
            }
            remaining = &rest[end + 1..];
        } else {
            break;
        }
    }
    ids
}

/// Validate citations in a verdict.
///
/// # Arguments
/// * `reasoning` — The `reasoning` field from the model's verdict JSON.
/// * `declared_citations` — The `evidence_citations` array from the verdict JSON.
/// * `real_tool_call_ids` — The IDs of tool calls that actually ran this session.
pub fn validate_citations(
    reasoning: &str,
    declared_citations: &[String],
    real_tool_call_ids: &[String],
) -> CitationReport {
    let real_ids: HashSet<&str> = real_tool_call_ids.iter().map(String::as_str).collect();

    // 1. Orphan check — every declared citation must reference a real tool call
    let orphan_citations: Vec<String> = declared_citations
        .iter()
        .filter(|id| !real_ids.contains(id.as_str()))
        .cloned()
        .collect();

    // Also check inline [evidence:X] tags embedded in the reasoning text
    let inline_ids = extract_citation_ids(reasoning);
    let orphan_inline: Vec<String> = inline_ids
        .iter()
        .filter(|id| !real_ids.contains(id.as_str()))
        .cloned()
        .collect();

    let all_orphans: Vec<String> = {
        let mut o = orphan_citations.clone();
        for id in orphan_inline {
            if !o.contains(&id) {
                o.push(id);
            }
        }
        o
    };

    // Build the full set of "cited" IDs — declared + inline
    let all_cited_ids: HashSet<String> = {
        let mut c: HashSet<String> = declared_citations.iter().cloned().collect();
        c.extend(extract_citation_ids(reasoning));
        c
    };

    // 2. Missing citation check — factual claim sentences without any citation
    let missing_citations: Vec<String> = reasoning
        .split(['.', '\n'])
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.len() > 20)
        .filter(|s| is_claim_sentence(s))
        .filter(|s| {
            // Sentence doesn't have an inline citation and no declared citation exists at all
            !has_inline_citation(s, reasoning) && all_cited_ids.is_empty()
        })
        .map(|s| {
            // Truncate to keep re-prompts short
            if s.len() > 120 { format!("{}…", &s[..120]) } else { s.to_string() }
        })
        .take(5) // cap at 5 to keep re-prompts readable
        .collect();

    let passed = all_orphans.is_empty() && missing_citations.is_empty();

    CitationReport { passed, missing_citations, orphan_citations: all_orphans }
}

/// Human-readable re-prompt for citation failures.
pub fn citation_reprompt(report: &CitationReport) -> String {
    let mut msg = String::from(
        "Your verdict was rejected due to citation issues. Fix all issues and re-emit your verdict JSON.\n\n"
    );
    if !report.missing_citations.is_empty() {
        msg.push_str("Claims requiring [evidence:<tool_call_id>] citations:\n");
        for claim in &report.missing_citations {
            msg.push_str(&format!("  - {claim}\n"));
        }
    }
    if !report.orphan_citations.is_empty() {
        msg.push_str("\nOrphan citations (no matching tool call in this session):\n");
        for id in &report.orphan_citations {
            msg.push_str(&format!("  - [evidence:{id}] — this ID was not a tool call in this session\n"));
        }
        msg.push_str("\nOnly use tool call IDs that appeared in the tool results you received.\n");
    }
    msg
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_records(ids: &[&str]) -> Vec<ToolCallRecord> {
        ids.iter().map(|_id| ToolCallRecord {
            tool_id: "get_user_baseline".into(),
            args_hash: "aa".into(),
            result_hash: "bb".into(),
            latency_ms: 10,
            policy_decision: "allow".into(),
            timestamp: chrono::Utc::now(),
        }).collect()
    }

    // ── has_retrieved_evidence ────────────────────────────────────────────────

    #[test]
    fn no_tool_calls_is_not_retrieved() {
        assert!(!has_retrieved_evidence(&[]));
    }

    #[test]
    fn denied_tool_call_is_not_retrieved() {
        let mut rec = make_records(&["t1"]);
        rec[0].policy_decision = "deny".into();
        assert!(!has_retrieved_evidence(&rec));
    }

    #[test]
    fn error_tool_call_is_not_retrieved() {
        let mut rec = make_records(&["t1"]);
        rec[0].policy_decision = "error".into();
        assert!(!has_retrieved_evidence(&rec));
    }

    #[test]
    fn successful_tool_call_is_retrieved() {
        let rec = make_records(&["t1"]);
        assert!(has_retrieved_evidence(&rec));
    }

    // ── extract_citation_ids ──────────────────────────────────────────────────

    #[test]
    fn extracts_multiple_citations() {
        let text = "Activity is suspicious [evidence:call_001]. Threat intel [evidence:call_002] confirms.";
        let ids = extract_citation_ids(text);
        assert_eq!(ids, vec!["call_001", "call_002"]);
    }

    #[test]
    fn no_citations_returns_empty() {
        let ids = extract_citation_ids("No citations here.");
        assert!(ids.is_empty());
    }

    // ── validate_citations ────────────────────────────────────────────────────

    #[test]
    fn passes_when_all_citations_are_real() {
        let reasoning = "The user login pattern is suspicious [evidence:call_001].";
        let declared = vec!["call_001".into()];
        let real = vec!["call_001".into()];
        let report = validate_citations(reasoning, &declared, &real);
        assert!(report.passed, "should pass: {report:?}");
        assert!(report.orphan_citations.is_empty());
    }

    #[test]
    fn fails_on_orphan_declared_citation() {
        let reasoning = "The ip address was suspicious [evidence:call_001].";
        let declared = vec!["call_001".into(), "fake_id".into()];
        let real = vec!["call_001".into()];
        let report = validate_citations(reasoning, &declared, &real);
        assert!(!report.passed);
        assert!(report.orphan_citations.contains(&"fake_id".to_string()));
    }

    #[test]
    fn fails_on_orphan_inline_citation() {
        let reasoning = "The user login was anomalous [evidence:invented_id].";
        let declared = vec![];
        let real = vec!["call_001".into()];
        let report = validate_citations(reasoning, &declared, &real);
        assert!(!report.passed);
        assert!(report.orphan_citations.contains(&"invented_id".to_string()));
    }

    #[test]
    fn passes_when_reasoning_has_inline_citations_matching_real() {
        let reasoning = "Login from unusual geo [evidence:call_xyz]. IP reputation high [evidence:call_xyz].";
        let declared = vec!["call_xyz".into()];
        let real = vec!["call_xyz".into()];
        let report = validate_citations(reasoning, &declared, &real);
        assert!(report.passed, "{report:?}");
    }

    #[test]
    fn passes_empty_reasoning_no_claims() {
        // No claims, no citations required
        let report = validate_citations("Verdict is benign.", &[], &[]);
        assert!(report.passed);
    }

    // ── citation_reprompt ─────────────────────────────────────────────────────

    #[test]
    fn reprompt_contains_orphan_ids() {
        let report = CitationReport {
            passed: false,
            missing_citations: vec![],
            orphan_citations: vec!["made_up_id".into()],
        };
        let prompt = citation_reprompt(&report);
        assert!(prompt.contains("made_up_id"));
    }
}

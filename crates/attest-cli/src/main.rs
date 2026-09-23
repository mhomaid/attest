//! `attest` — verify and replay signed attestation envelopes.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "attest",
    about = "Verify and replay Attest attestation envelopes",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Recompute the canonical hash and verify the Ed25519 signature on every envelope.
    Verify {
        /// NDJSON attestation log (one envelope per line).
        log: PathBuf,
        /// Hex-encoded 32-byte Ed25519 verifying key.
        #[arg(long)]
        key: String,
        /// Fail envelopes whose classifier `model_artifact_hash` differs from this SHA-256.
        #[arg(long)]
        pin_model: Option<String>,
        /// Fail envelopes whose LLM `system_prompt_hash` differs from this SHA-256.
        #[arg(long)]
        pin_prompt: Option<String>,
    },
    /// Replay one action. Classifier decisions are re-inferred; LLM decisions are integrity-checked.
    Replay {
        /// `agent_action_id` of the envelope to replay.
        action_id: String,
        /// NDJSON attestation log containing the envelope.
        #[arg(long)]
        log: PathBuf,
        /// Hex-encoded 32-byte Ed25519 verifying key.
        #[arg(long)]
        key: String,
        /// Path to the ONNX model that produced the recorded prediction (classifier / hybrid).
        #[arg(long)]
        model: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let ok = match cli.command {
        Command::Verify {
            log,
            key,
            pin_model,
            pin_prompt,
        } => {
            let envelopes = attest_cli::read_log(&log)?;
            if envelopes.is_empty() {
                anyhow::bail!("{} contains no envelopes", log.display());
            }
            let mut report = attest_cli::verify_log(&envelopes, &key);
            let pins = attest_cli::Pins {
                model_artifact_hash: pin_model,
                system_prompt_hash: pin_prompt,
            };
            attest_cli::apply_pins(&mut report, &envelopes, &pins);
            attest_cli::write_report_verify(&report);
            report.all_ok()
        }
        Command::Replay {
            action_id,
            log,
            key,
            model,
        } => {
            let envelopes = attest_cli::read_log(&log)?;
            let id = attest_cli::parse_action_id(&action_id)?;
            let envelope = attest_cli::find_envelope(&envelopes, &id)
                .with_context(|| format!("in {}", log.display()))?;
            let report = attest_cli::replay(envelope, &key, model.as_deref())?;
            attest_cli::write_report_replay(&report);
            report.ok
        }
    };
    Ok(ExitCode::from(attest_cli::exit_code(ok) as u8))
}

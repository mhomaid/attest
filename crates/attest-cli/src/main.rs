//! `attest` — verify and replay signed attestation envelopes.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};

const AFTER_HELP: &str = "\
Examples:
  cargo install --path crates/attest-cli --locked
  attest verify examples/verify/attestations.ndjson --key-file examples/verify/verifying-key.txt
  attest replay <action-id> --log attestations.ndjson --key-file verifying-key.txt \\
    --model ml/triager/artifacts/model.onnx

From a checkout without installing (the `--` is required so clap sees the subcommand):
  cargo run -q -p attest-cli -- verify LOG --key-file KEY

Exit codes: 0 all envelopes passed, 1 any failure or usage error.
Prefer --key-file (or ATTEST_VERIFYING_KEY) over --key so the hex is not in `ps` or history.
";

#[derive(Parser)]
#[command(
    name = "attest",
    about = "Verify and replay Attest attestation envelopes",
    version,
    propagate_version = true,
    arg_required_else_help = true,
    after_help = AFTER_HELP
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Clone)]
struct VerifyingKey {
    /// Hex-encoded 32-byte Ed25519 verifying key. Prefer --key-file.
    #[arg(long, env = "ATTEST_VERIFYING_KEY", value_name = "HEX")]
    key: Option<String>,
    /// File containing the hex verifying key (one line, whitespace trimmed).
    #[arg(long, value_name = "PATH")]
    key_file: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Recompute the canonical hash and verify the Ed25519 signature on every envelope.
    Verify {
        /// NDJSON attestation log (one envelope per line).
        #[arg(value_name = "LOG")]
        log: PathBuf,
        #[command(flatten)]
        verifying_key: VerifyingKey,
        /// Fail envelopes whose classifier `model_artifact_hash` differs from this SHA-256.
        #[arg(long, value_name = "SHA256")]
        pin_model: Option<String>,
        /// Fail envelopes whose LLM `system_prompt_hash` differs from this SHA-256.
        #[arg(long, value_name = "SHA256")]
        pin_prompt: Option<String>,
    },
    /// Replay one action. Classifier decisions are re-inferred; LLM decisions are integrity-checked.
    Replay {
        /// `agent_action_id` of the envelope to replay.
        #[arg(value_name = "ACTION_ID")]
        action_id: String,
        /// NDJSON attestation log containing the envelope.
        #[arg(long, value_name = "PATH")]
        log: PathBuf,
        #[command(flatten)]
        verifying_key: VerifyingKey,
        /// Path to the ONNX model that produced the recorded prediction (classifier / hybrid).
        #[arg(long, value_name = "ONNX")]
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
            verifying_key,
            pin_model,
            pin_prompt,
        } => {
            let key = attest_cli::resolve_verifying_key(
                verifying_key.key,
                verifying_key.key_file.as_deref(),
            )?;
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
            verifying_key,
            model,
        } => {
            let key = attest_cli::resolve_verifying_key(
                verifying_key.key,
                verifying_key.key_file.as_deref(),
            )?;
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

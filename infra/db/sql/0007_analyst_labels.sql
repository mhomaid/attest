-- Analyst verdict labels. One row per submission. User and time are written
-- by the workbench from the session; envelope_id is the attestation envelope
-- the analyst was looking at (agent_action_id).

CREATE TABLE IF NOT EXISTS workbench_analyst_labels (
  id text PRIMARY KEY,
  case_id text NOT NULL,
  envelope_id text NOT NULL,
  agreement text NOT NULL CHECK (agreement IN ('agree', 'disagree')),
  disposition text NOT NULL CHECK (
    disposition IN ('true_positive', 'false_positive', 'benign', 'needs_investigation')
  ),
  reason text NOT NULL,
  user_id text NOT NULL,
  user_email text NOT NULL,
  created_at timestamptz NOT NULL
);

CREATE INDEX IF NOT EXISTS workbench_analyst_labels_created_at_idx
  ON workbench_analyst_labels (created_at);

CREATE INDEX IF NOT EXISTS workbench_analyst_labels_envelope_idx
  ON workbench_analyst_labels (envelope_id);

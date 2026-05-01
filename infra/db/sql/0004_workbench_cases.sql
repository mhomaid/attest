CREATE TABLE IF NOT EXISTS workbench_cases (
  case_id text PRIMARY KEY,
  tenant_id text NOT NULL DEFAULT 'default',
  event jsonb NOT NULL,
  baseline jsonb,
  detection jsonb,
  triage_status text NOT NULL DEFAULT 'idle',
  triage_started_at timestamptz,
  triage_completed_at timestamptz,
  verdict jsonb,
  error text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS workbench_case_trace_steps (
  id text PRIMARY KEY,
  case_id text NOT NULL REFERENCES workbench_cases(case_id) ON DELETE CASCADE,
  agent_action_id text NOT NULL,
  agent_id text NOT NULL,
  execution_path text NOT NULL,
  step_kind text NOT NULL,
  summary text NOT NULL,
  ts timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  UNIQUE (case_id, agent_action_id, step_kind, ts)
);

CREATE INDEX IF NOT EXISTS workbench_case_trace_steps_case_ts_idx
  ON workbench_case_trace_steps (case_id, ts);

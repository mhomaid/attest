/** Kysely database model — Better Auth tables (`auth_*` / `modelName` in auth config). */

export interface AuthUsersTable {
  id: string;
  name: string;
  email: string;
  email_verified: boolean;
  image: string | null;
  created_at: Date;
  updated_at: Date;
}

export interface AuthSessionsTable {
  id: string;
  user_id: string;
  token: string;
  expires_at: Date;
  ip_address: string | null;
  user_agent: string | null;
  created_at: Date;
  updated_at: Date;
}

export interface AuthAccountsTable {
  id: string;
  user_id: string;
  account_id: string;
  provider_id: string;
  access_token: string | null;
  refresh_token: string | null;
  access_token_expires_at: Date | null;
  refresh_token_expires_at: Date | null;
  scope: string | null;
  id_token: string | null;
  password: string | null;
  created_at: Date;
  updated_at: Date;
}

export interface AuthVerificationsTable {
  id: string;
  identifier: string;
  value: string;
  expires_at: Date;
  created_at: Date;
  updated_at: Date;
}

export interface WorkbenchCasesTable {
  case_id: string;
  tenant_id: string;
  event: unknown;
  baseline: unknown | null;
  detection: unknown | null;
  triage_status: "idle" | "running" | "complete" | "failed";
  triage_started_at: Date | null;
  triage_completed_at: Date | null;
  verdict: unknown | null;
  error: string | null;
  created_at: Date;
  updated_at: Date;
}

export interface WorkbenchCaseTraceStepsTable {
  id: string;
  case_id: string;
  agent_action_id: string;
  agent_id: string;
  execution_path: string;
  step_kind: string;
  summary: string;
  ts: Date;
  created_at: Date;
}

export interface DB {
  auth_users: AuthUsersTable;
  auth_sessions: AuthSessionsTable;
  auth_accounts: AuthAccountsTable;
  auth_verifications: AuthVerificationsTable;
  workbench_cases: WorkbenchCasesTable;
  workbench_case_trace_steps: WorkbenchCaseTraceStepsTable;
}

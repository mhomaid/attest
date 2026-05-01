-- Extra local dev personas (same password as analyst for simplicity).
-- Password for all: analyst-dev  (hash reused from 0002; Better Auth verifies per-hash, same plaintext is fine).
--
--   analyst@attest.local   — already in 0002
--   viewer@attest.local    — read-only / reviewer persona (app may treat all equally today)
--   operator@attest.local    — second hands-on persona
--
-- Idempotent: one row per email; credential account linked only if missing.

INSERT INTO auth_users (id, name, email, email_verified, image, created_at, updated_at)
SELECT
  '33333333-3333-4333-8333-333333333333',
  'Viewer',
  'viewer@attest.local',
  true,
  NULL,
  now(),
  now()
WHERE NOT EXISTS (
  SELECT 1 FROM auth_users WHERE email = 'viewer@attest.local'
);

INSERT INTO auth_accounts (
  id,
  user_id,
  account_id,
  provider_id,
  access_token,
  refresh_token,
  access_token_expires_at,
  refresh_token_expires_at,
  scope,
  id_token,
  password,
  created_at,
  updated_at
)
SELECT
  '44444444-4444-4444-8444-444444444444',
  u.id,
  u.id,
  'credential',
  NULL,
  NULL,
  NULL,
  NULL,
  NULL,
  NULL,
  '92d3520e028abefd6392aa9f9534d8a4:a501e314db471160e368831e1c23b42f92e568b5bdd3e823e2f7525bc906df986019dd3239b48b2727f69651d42c61f9cabfe439fb578e7e286f797be4bc181b',
  now(),
  now()
FROM auth_users u
WHERE u.email = 'viewer@attest.local'
  AND NOT EXISTS (
    SELECT 1
    FROM auth_accounts a
    WHERE a.user_id = u.id
      AND a.provider_id = 'credential'
  );

INSERT INTO auth_users (id, name, email, email_verified, image, created_at, updated_at)
SELECT
  '55555555-5555-4555-8555-555555555555',
  'Operator',
  'operator@attest.local',
  true,
  NULL,
  now(),
  now()
WHERE NOT EXISTS (
  SELECT 1 FROM auth_users WHERE email = 'operator@attest.local'
);

INSERT INTO auth_accounts (
  id,
  user_id,
  account_id,
  provider_id,
  access_token,
  refresh_token,
  access_token_expires_at,
  refresh_token_expires_at,
  scope,
  id_token,
  password,
  created_at,
  updated_at
)
SELECT
  '66666666-6666-4666-8666-666666666666',
  u.id,
  u.id,
  'credential',
  NULL,
  NULL,
  NULL,
  NULL,
  NULL,
  NULL,
  '92d3520e028abefd6392aa9f9534d8a4:a501e314db471160e368831e1c23b42f92e568b5bdd3e823e2f7525bc906df986019dd3239b48b2727f69651d42c61f9cabfe439fb578e7e286f797be4bc181b',
  now(),
  now()
FROM auth_users u
WHERE u.email = 'operator@attest.local'
  AND NOT EXISTS (
    SELECT 1
    FROM auth_accounts a
    WHERE a.user_id = u.id
      AND a.provider_id = 'credential'
  );

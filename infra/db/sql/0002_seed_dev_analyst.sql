-- Dev analyst login (local workbench only).
-- Login: analyst@attest.local / analyst-dev
--
-- Password hash is Better Auth scrypt ("salt:derived" hex). Regenerate if you change the password:
--   cd apps/workbench && bun -e "import { hashPassword } from 'better-auth/crypto'; console.log(await hashPassword('NEW_PASS'));"
--
-- Idempotent: skips if analyst@attest.local already exists; adds credential account only if missing.

INSERT INTO auth_users (id, name, email, email_verified, image, created_at, updated_at)
SELECT
  '11111111-1111-4111-8111-111111111111',
  'Analyst',
  'analyst@attest.local',
  true,
  NULL,
  now(),
  now()
WHERE NOT EXISTS (
  SELECT 1 FROM auth_users WHERE email = 'analyst@attest.local'
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
  '22222222-2222-4222-8222-222222222222',
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
WHERE u.email = 'analyst@attest.local'
  AND NOT EXISTS (
    SELECT 1
    FROM auth_accounts a
    WHERE a.user_id = u.id
      AND a.provider_id = 'credential'
  );

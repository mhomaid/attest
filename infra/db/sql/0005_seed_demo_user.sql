-- Public demo login (hosted + local).
-- Login: demo@attest.local / try-attest
--
-- Password hash is Better Auth scrypt ("salt:derived" hex).
-- Idempotent: skips if demo@attest.local already exists.

INSERT INTO auth_users (id, name, email, email_verified, image, created_at, updated_at)
SELECT
  '33333333-3333-4333-8333-333333333333',
  'Demo',
  'demo@attest.local',
  true,
  NULL,
  now(),
  now()
WHERE NOT EXISTS (
  SELECT 1 FROM auth_users WHERE email = 'demo@attest.local'
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
  '28c852d867b664ea6782a12d8e32ea2a:f1c736e27c7538112c61f776b933a5482a546ebd0349dd8b77c763d2b34e430456dfc20848eb2a748e2e8000dfc47cb1641a3d64cb43b7ecfb5111cc786b6a3c',
  now(),
  now()
FROM auth_users u
WHERE u.email = 'demo@attest.local'
  AND NOT EXISTS (
    SELECT 1
    FROM auth_accounts a
    WHERE a.user_id = u.id
      AND a.provider_id = 'credential'
  );

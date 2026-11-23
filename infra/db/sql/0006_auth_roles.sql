-- Workbench roles (viewer | analyst | lead | auditor). OIDC sign-in rewrites this column from
-- the configured role claim; everything else defaults to the least-privileged role.

ALTER TABLE auth_users ADD COLUMN IF NOT EXISTS role text NOT NULL DEFAULT 'viewer';

ALTER TABLE auth_users DROP CONSTRAINT IF EXISTS auth_users_role_check;
ALTER TABLE auth_users
  ADD CONSTRAINT auth_users_role_check CHECK (role IN ('viewer', 'analyst', 'lead', 'auditor'));

-- Local dev users seeded by 0002 / 0003. Demo and viewer users stay viewers.
UPDATE auth_users SET role = 'analyst'
WHERE email IN ('analyst@attest.local', 'operator@attest.local');

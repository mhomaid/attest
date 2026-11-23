"""Add a role column to auth_users.

Revision ID: 0006_auth_roles
Revises: 0005_seed_demo_user
Create Date: 2026-11-21

"""

from __future__ import annotations

from pathlib import Path

from alembic import op

revision = "0006_auth_roles"
down_revision = "0005_seed_demo_user"
branch_labels = None
depends_on = None

_SQL_PATH = Path(__file__).resolve().parent.parent.parent / "sql" / "0006_auth_roles.sql"


def upgrade() -> None:
    op.execute(_SQL_PATH.read_text(encoding="utf-8"))


def downgrade() -> None:
    op.execute("ALTER TABLE auth_users DROP CONSTRAINT IF EXISTS auth_users_role_check;")
    op.execute("ALTER TABLE auth_users DROP COLUMN IF EXISTS role;")

"""Better Auth tables (snake_case columns).

Revision ID: 0001_better_auth
Revises:
Create Date: 2026-05-01

"""

from __future__ import annotations

from pathlib import Path

from alembic import op

revision = "0001_better_auth"
down_revision = None
branch_labels = None
depends_on = None

_SQL_PATH = Path(__file__).resolve().parent.parent.parent / "sql" / "0001_auth.sql"


def upgrade() -> None:
    op.execute(_SQL_PATH.read_text(encoding="utf-8"))


def downgrade() -> None:
    op.execute("DROP TABLE IF EXISTS auth_verifications CASCADE;")
    op.execute("DROP TABLE IF EXISTS auth_accounts CASCADE;")
    op.execute("DROP TABLE IF EXISTS auth_sessions CASCADE;")
    op.execute("DROP TABLE IF EXISTS auth_users CASCADE;")

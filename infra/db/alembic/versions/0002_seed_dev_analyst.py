"""Seed dev analyst user (Better Auth email/password).

Revision ID: 0002_seed_dev_analyst
Revises: 0001_better_auth
Create Date: 2026-05-01

"""

from __future__ import annotations

from pathlib import Path

from alembic import op

revision = "0002_seed_dev_analyst"
down_revision = "0001_better_auth"
branch_labels = None
depends_on = None

_SQL_PATH = Path(__file__).resolve().parent.parent.parent / "sql" / "0002_seed_dev_analyst.sql"
_SEED_USER_ID = "11111111-1111-4111-8111-111111111111"
_ACCOUNT_ID = "22222222-2222-4222-8222-222222222222"


def upgrade() -> None:
    op.execute(_SQL_PATH.read_text(encoding="utf-8"))


def downgrade() -> None:
    op.execute(f"DELETE FROM auth_accounts WHERE id = '{_ACCOUNT_ID}';")
    op.execute(f"DELETE FROM auth_users WHERE id = '{_SEED_USER_ID}';")

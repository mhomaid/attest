"""Seed public demo user (Better Auth email/password).

Revision ID: 0005_seed_demo_user
Revises: 0004_workbench_cases
Create Date: 2026-09-23

"""

from __future__ import annotations

from pathlib import Path

from alembic import op

revision = "0005_seed_demo_user"
down_revision = "0004_workbench_cases"
branch_labels = None
depends_on = None

_SQL_PATH = Path(__file__).resolve().parent.parent.parent / "sql" / "0005_seed_demo_user.sql"
_SEED_USER_ID = "33333333-3333-4333-8333-333333333333"
_ACCOUNT_ID = "44444444-4444-4444-8444-444444444444"


def upgrade() -> None:
    op.execute(_SQL_PATH.read_text(encoding="utf-8"))


def downgrade() -> None:
    op.execute(f"DELETE FROM auth_accounts WHERE id = '{_ACCOUNT_ID}';")
    op.execute(f"DELETE FROM auth_users WHERE id = '{_SEED_USER_ID}';")

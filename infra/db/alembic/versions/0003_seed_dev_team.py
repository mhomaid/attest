"""Seed additional dev users (viewer, operator).

Revision ID: 0003_seed_dev_team
Revises: 0002_seed_dev_analyst
Create Date: 2026-05-01

"""

from __future__ import annotations

from pathlib import Path

from alembic import op

revision = "0003_seed_dev_team"
down_revision = "0002_seed_dev_analyst"
branch_labels = None
depends_on = None

_SQL_PATH = Path(__file__).resolve().parent.parent.parent / "sql" / "0003_seed_dev_team.sql"

_VIEWER_USER = "33333333-3333-4333-8333-333333333333"
_VIEWER_ACCOUNT = "44444444-4444-4444-8444-444444444444"
_OPERATOR_USER = "55555555-5555-4555-8555-555555555555"
_OPERATOR_ACCOUNT = "66666666-6666-4666-8666-666666666666"


def upgrade() -> None:
    op.execute(_SQL_PATH.read_text(encoding="utf-8"))


def downgrade() -> None:
    for account_id in (_VIEWER_ACCOUNT, _OPERATOR_ACCOUNT):
        op.execute(f"DELETE FROM auth_accounts WHERE id = '{account_id}';")
    for user_id in (_VIEWER_USER, _OPERATOR_USER):
        op.execute(f"DELETE FROM auth_users WHERE id = '{user_id}';")

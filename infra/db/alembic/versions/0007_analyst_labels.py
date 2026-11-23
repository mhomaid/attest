"""Create the analyst verdict label table.

Revision ID: 0007_analyst_labels
Revises: 0006_auth_roles
Create Date: 2026-11-22

"""

from __future__ import annotations

from pathlib import Path

from alembic import op

revision = "0007_analyst_labels"
down_revision = "0006_auth_roles"
branch_labels = None
depends_on = None

_SQL_PATH = Path(__file__).resolve().parent.parent.parent / "sql" / "0007_analyst_labels.sql"


def upgrade() -> None:
    op.execute(_SQL_PATH.read_text(encoding="utf-8"))


def downgrade() -> None:
    op.execute("DROP TABLE IF EXISTS workbench_analyst_labels;")

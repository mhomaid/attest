"""Create durable workbench case tables.

Revision ID: 0004_workbench_cases
Revises: 0003_seed_dev_team
Create Date: 2026-05-01

"""

from __future__ import annotations

from pathlib import Path

from alembic import op

revision = "0004_workbench_cases"
down_revision = "0003_seed_dev_team"
branch_labels = None
depends_on = None

_SQL_PATH = Path(__file__).resolve().parent.parent.parent / "sql" / "0004_workbench_cases.sql"


def upgrade() -> None:
    op.execute(_SQL_PATH.read_text(encoding="utf-8"))


def downgrade() -> None:
    op.execute("DROP TABLE IF EXISTS workbench_case_trace_steps;")
    op.execute("DROP TABLE IF EXISTS workbench_cases;")

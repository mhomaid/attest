import { Kysely, PostgresDialect } from "kysely";
import { Pool } from "pg";
import type { DB } from "@/types/db";

const connectionString =
  process.env.DATABASE_URL ??
  process.env.POSTGRES_URL ??
  "postgres://attest:attest@localhost:5432/attest";

/** Shared pool — Better Auth (`database: pool`) and Kysely. */
export const pool = new Pool({ connectionString });

export const db = new Kysely<DB>({
  dialect: new PostgresDialect({
    pool,
  }),
});

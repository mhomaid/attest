import { Kysely, PostgresDialect } from "kysely";
import { Pool } from "pg";
import type { DB } from "@/types/db";

const connectionString =
  process.env.DATABASE_URL ??
  process.env.POSTGRES_URL ??
  "postgres://attest:attest@localhost:5432/attest";

/**
 * Pool tuning rationale:
 * - `max: 20` — single Next.js process; Better Auth + workbench routes
 *   share this pool. 20 is comfortable for local dev and small prod.
 * - `idleTimeoutMillis` — release idle clients quickly so HMR/restarts
 *   don't pile up half-dead connections.
 * - `connectionTimeoutMillis` — fail fast (1.5 s) if Postgres is down,
 *   instead of hanging server components forever.
 * - `statement_timeout` — kill any individual statement that exceeds 5 s
 *   so a slow query never wedges a connection for the whole pool.
 *
 * In dev, Next.js HMR re-evaluates server modules — without a global
 * cache we'd leak a fresh pool on every change.
 */
type PoolHolder = { pool: Pool; db: Kysely<DB> };
const globalForDb = globalThis as unknown as { __attestDb?: PoolHolder };

function makeHolder(): PoolHolder {
  const pool = new Pool({
    connectionString,
    max: 20,
    idleTimeoutMillis: 30_000,
    connectionTimeoutMillis: 1_500,
    statement_timeout: 5_000,
  });

  pool.on("error", (err) => {
    // pg surfaces idle-client errors here; logging avoids unhandled rejections.
    console.error("[db] idle client error", err);
  });

  const db = new Kysely<DB>({
    dialect: new PostgresDialect({ pool }),
  });

  return { pool, db };
}

const holder = globalForDb.__attestDb ?? (globalForDb.__attestDb = makeHolder());

export const pool = holder.pool;
export const db = holder.db;

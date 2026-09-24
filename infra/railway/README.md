# Railway Deployment Guide

This directory contains Railway service configuration templates for all
application services. Infrastructure services (Redpanda, RisingWave,
ClickHouse, MinIO) are deployed as Docker-image services directly from the
Railway dashboard.

## Architecture

```
┌─────────────────────── Railway Project: attest ───────────────────────┐
│                                                                         │
│  Infrastructure (Docker image services — internal only)                │
│  ┌──────────┐  ┌────────────┐  ┌────────────┐  ┌────────┐            │
│  │ Redpanda │  │ RisingWave │  │ ClickHouse │  │  MinIO │            │
│  │  :9092   │  │   :4566    │  │   :8123    │  │  :9000 │            │
│  └──────────┘  └────────────┘  └────────────┘  └────────┘            │
│                                                                         │
│  Application services (built from Dockerfiles)                         │
│  ┌───────────┐  ┌───────────────┐  ┌──────────────────┐              │
│  │ collector │  │ control-plane │  │ detection-runtime │              │
│  │   :4000   │  │    :8080 ◄──────── WebSocket ──────► workbench     │
│  └───────────┘  └───────────────┘  └──────────────────┘              │
│                      ┌──────────────────┐                              │
│                      │ storage-iceberg  │                              │
│                      │ (background)     │                              │
│                      └──────────────────┘                              │
└─────────────────────────────────────────────────────────────────────────┘
```

Services communicate over Railway's private network using
`<service-name>.railway.internal:<port>`. Only `control-plane` and
`workbench` need public domains.

---

## Prerequisites

```bash
# Install Railway CLI
brew install railwayapp/railway/railway

# Login
railway login
```

---

## Step 1 — Create the Railway project

```bash
railway project create attest
railway environment create production
```

---

## Step 2 — Add infrastructure services (Docker image)

Add each of the following via the Railway dashboard ("New Service → Docker
Image") or with the CLI. These services do **not** need a public domain.

### Redpanda

- **Image**: `redpandadata/redpanda:latest`
- **Start command**:
  ```
  redpanda start --overprovisioned --smp 1 --memory 1G --reserve-memory 0M --node-id 0 --check=false --kafka-addr PLAINTEXT://0.0.0.0:9092 --advertise-kafka-addr PLAINTEXT://$RAILWAY_PRIVATE_DOMAIN:9092
  ```
- **Port**: `9092` (do NOT expose publicly)
- **Volume**: not required (ephemeral for now)

### RisingWave

- **Image**: `risingwavelabs/risingwave:v2.4.2` (or `latest`)
- **Start command**: `/risingwave/bin/risingwave single_node`
  Railway replaces the image entrypoint, so a bare `playground` / `single-node`
  never execs. Current images use the `single_node` subcommand (underscore).
- **Port**: `4566` (internal only)
- **State**: `RW_STATE_STORE=hummock+minio://…@minio.railway.internal:9000/risingwave`
  and `RW_DATA_DIRECTORY=attest-data`. MinIO must already have a `risingwave` bucket.

### ClickHouse

- **Image**: `clickhouse/clickhouse-server:latest`
- **Environment**:
  - `CLICKHOUSE_USER=default`
  - `CLICKHOUSE_PASSWORD=` *(leave blank or set a secret)*
  - `CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT=1`
- **Port**: `8123` (internal only)
- **Volume**: attach a Railway volume at `/var/lib/clickhouse`

### MinIO

- **Image**: `pgsty/minio:RELEASE.2026-08-04T00-00-00Z` (MinIO no longer publishes
  community images; `quay.io/minio/*` and `minio/*` pulls are denied)
- **Start command**: `server /data --console-address :9001`
- **Environment**:
  - `MINIO_ROOT_USER=<secret>`
  - `MINIO_ROOT_PASSWORD=<secret>`
- **Ports**: `9000` (internal API), `9001` (console — expose publicly if needed)
- **Volume**: attach a Railway volume at `/data`

> After MinIO is up, create the `attest-warm` bucket by opening the MinIO
> console (`https://<minio-public-url>:9001`) and creating a bucket named
> `attest-warm` with public read access, **or** SSH into the service and run:
> ```bash
> mc alias set local http://localhost:9000 $MINIO_ROOT_USER $MINIO_ROOT_PASSWORD
> mc mb local/attest-warm
> mc policy set public local/attest-warm
> ```

---

## Step 3 — Add application services (Dockerfile build)

For each application service, create a new Railway service pointing at this
GitHub repository with the settings below. Leave "Root Directory" as `/`
(repo root) and set the Dockerfile path in the service's build settings.

The JSON files in this directory (`infra/railway/*.json`) serve as the
canonical environment variable reference for each service. Copy the
`environmentVariables` block into Railway's variable editor, filling in any
`<secret>` placeholders.

### collector

| Setting | Value |
|---------|-------|
| **Dockerfile path** | `infra/docker/collector.Dockerfile` |
| **Port** | `4000` |
| **Public domain** | not required |
| **Config file** | [`infra/railway/collector.json`](collector.json) |

### control-plane

| Setting | Value |
|---------|-------|
| **Dockerfile path** | `infra/docker/control-plane.Dockerfile` |
| **Port** | `8080` |
| **Public domain** | **required** (WebSocket clients connect here) |
| **Config file** | [`infra/railway/control-plane.json`](control-plane.json) |

### storage-iceberg

| Setting | Value |
|---------|-------|
| **Dockerfile path** | `infra/docker/storage-iceberg.Dockerfile` |
| **Port** | none |
| **Public domain** | not required |
| **Config file** | [`infra/railway/storage-iceberg.json`](storage-iceberg.json) |

### detection-runtime

| Setting | Value |
|---------|-------|
| **Dockerfile path** | `infra/docker/detection-runtime.Dockerfile` |
| **Port** | none |
| **Public domain** | not required |
| **Config file** | [`infra/railway/detection-runtime.json`](detection-runtime.json) |

> Detection rules are **baked into the Docker image** at build time
> (`COPY detections/ /rules/`). To update rules, push a new commit and
> Railway will rebuild the image automatically.

### workbench

| Setting | Value |
|---------|-------|
| **Builder** | Nixpacks (uses repo-root `nixpacks.toml`) |
| **Port** | `3000` |
| **Public domain** | **required** |
| **Config file** | [`infra/railway/workbench.json`](workbench.json) |

---

## Step 4 — Set environment variables

For each service, open "Variables" in the Railway dashboard and paste the
contents of the corresponding `infra/railway/*.json` `environmentVariables`
object.

Railway variable references (`${{ServiceName.VARIABLE}}`) are resolved
automatically within the same project. The key references are:

| Variable | Value pattern |
|----------|---------------|
| `KAFKA_BROKERS` | `${{Redpanda.RAILWAY_PRIVATE_DOMAIN}}:9092` |
| `RISINGWAVE_HOST` | `${{Risingwave.RAILWAY_PRIVATE_DOMAIN}}` |
| `CLICKHOUSE_URL` | `http://${{Clickhouse.RAILWAY_PRIVATE_DOMAIN}}:8123` |
| `S3_ENDPOINT` | `http://${{Minio.RAILWAY_PRIVATE_DOMAIN}}:9000` |
| `CONTROL_PLANE_URL` | `https://${{ControlPlane.RAILWAY_PUBLIC_DOMAIN}}` |
| `NEXT_PUBLIC_CP_WS_URL` | `wss://${{ControlPlane.RAILWAY_PUBLIC_DOMAIN}}` |

> **WebSocket note**: Railway terminates TLS at the edge, so the browser
> connects over `wss://` even though the control-plane listens on plain
> `ws://` internally. Always use `wss://` for `NEXT_PUBLIC_CP_WS_URL` in
> production.

---

## Step 5 — Configure Dockerfile paths (dashboard only)

For each Rust application service, open the Railway dashboard and navigate
to **Settings → Build → Builder**. Change from "Nixpacks" to "Dockerfile"
and enter the path shown below:

| Service | Dockerfile path |
|---------|----------------|
| `collector` | `infra/docker/collector.Dockerfile` |
| `control-plane` | `infra/docker/control-plane.Dockerfile` |
| `storage-iceberg` | `infra/docker/storage-iceberg.Dockerfile` |
| `detection-runtime` | `infra/docker/detection-runtime.Dockerfile` |
| `workbench` | *(leave as Nixpacks — uses repo-root `railway.json`)* |

Leave the **Root Directory** as `/` (repo root) for all services.

---

## Step 6 — First deploy

After Dockerfile paths and env vars are set:

```bash
# Upload local source and trigger builds for all services
make railway-deploy

# Or deploy a single service
railway up --service control-plane --detach --ci
```

For all subsequent deploys (e.g. after a `git push`):

```bash
# Trigger redeploy from the latest commit already in Railway
make railway-redeploy

# Or redeploy a single service
railway service redeploy --service control-plane --yes
```

---

## Startup order

Railway starts all services simultaneously. The detection-runtime has a
built-in startup readiness check (`wait_for_schema`) that polls
`cloudtrail_events` every 5 s (up to 120 s) before deploying materialized
views, so the exact startup order does not matter.

---

## Makefile reference

```
make railway-login      # Log in to Railway CLI
make railway-setup      # Create all services with railway add (run once)
make railway-deploy     # railway up --service for all services (first deploy)
make railway-redeploy   # railway service redeploy for all services (after first deploy)
make prod down          # Stop everything except workbench (marketing + waitlist)
make prod up            # Redeploy infra, then apps. Workbench stays as-is.
make prod status        # Deployment state per service
```

`make prod down` / `up` keep **workbench** live (`https://attest.homaid.dev`).
Postgres, Kafka, RisingWave, ClickHouse, MinIO, and the Rust apps go down.
`down` also pins those GitHub services to `watchPatterns=[".railway-manual-only"]`
so a push to `main` does not wake them. `up` is an explicit redeploy.

Same script: `./scripts/prod-railway.sh up|down|status` (`pause`/`resume` are aliases).

Volumes still bill while allocated, even when services are down.

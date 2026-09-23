# 15 — Streaming Engine Decision: Arroyo vs. Apache Flink

**Document type:** Architecture decision record (ADR)
**Status:** Accepted
**Decision:** Attest uses **Arroyo + RisingWave** (both Rust-native) as its streaming runtime. Apache Flink is explicitly not used.
**Purpose:** Explains what Flink would replace, why Arroyo was chosen, what would have to change to use Flink instead, and the full trade-off comparison — including the cases where Flink is the better choice.

---

## 1. The landscape in one sentence

All three systems — Flink, Arroyo, and RisingWave — solve the same core problem: **continuously process a stream of events, maintain state, and emit results**. They differ in runtime model, operational philosophy, and the trade-offs they make between expressiveness, latency, and complexity.

---

## 2. Where each system fits in the Attest architecture

### Current architecture (Arroyo + RisingWave)

```
Redpanda (Kafka)
      │
      ├──► RisingWave ──────────────────────────────────► Materialized views
      │    (streaming SQL, PostgreSQL protocol)            entity_baselines
      │    Primary engine for ~85% of detections          recent_events
      │    Temporal joins, windowed aggregations           det_* detection views
      │    Feeds: control-plane API, detection-runtime
      │
      ├──► Arroyo ─────────────────────────────────────► Multi-sink ETL pipelines
      │    (Rust-native streaming SQL)                     Kafka → Iceberg (warm tier)
      │    Stateful CEP — n-event sequence patterns        Kafka → enrichment tables
      │    Handles ~10% of pipeline workloads              Kafka → fan-out routing
      │
      └──► Custom Rust services (rdkafka + Tokio) ──────► Edge cases (~5%)
           Hand-written consumers for detection            Detection patterns that
           patterns neither engine expresses well          neither engine can express
```

### Where Flink would fit if used instead of Arroyo

```
Redpanda (Kafka)
      │
      ├──► RisingWave ──────────► unchanged
      │
      ├──► Apache Flink ────────► replaces Arroyo
      │    (JVM, Java/Scala/Python APIs)
      │    Stateful CEP via Flink CEP library
      │    Multi-sink pipelines via Flink connectors
      │    Complex sequence detections
      │
      └──► Custom Rust services ► unchanged (~5%)
```

**The substitution is clean: Arroyo and Flink occupy the same slot.** They both handle stateful, multi-step pipeline work that RisingWave's materialized-view model doesn't express well. You would not replace RisingWave with Flink — they solve different problems (see Section 5).

---

## 3. What would have to change to use Flink instead of Arroyo

This section answers the question: *"If this codebase had to migrate from Arroyo to Flink, what would change?"* A decision is only as good as the understanding of its reversal cost.

### 3.1 Add the JVM to your infrastructure

Arroyo is a Rust binary. Flink requires:
- JVM (Java 11 or 17) on every node
- Flink JobManager (coordinator) + TaskManagers (workers) — minimum 3 containers vs. Arroyo's 1
- A state backend: RocksDB (local, fast) or S3/HDFS (for savepoints)
- Checkpoint storage (S3 bucket or HDFS for fault-tolerant state snapshots)

**On Railway today:** add a `flink-jobmanager` service and 2+ `flink-taskmanager` services — at least 3 new long-running containers, plus JobManager HA if the pipelines are critical. Sizing is workload-dependent, but the fixed per-cluster overhead is higher than a single Arroyo controller.

### 3.2 Rewrite pipeline definitions from SQL to Flink API (or Flink SQL)

**Arroyo pipeline (SQL):**
```sql
INSERT INTO iceberg_sink
SELECT event_id, actor_user_name, cloud_region, event_time
FROM cloudtrail_events;
```

**Flink DataStream API equivalent (Java):**
```java
StreamExecutionEnvironment env = StreamExecutionEnvironment.getExecutionEnvironment();
env.enableCheckpointing(30_000); // 30s checkpoint interval

DataStream<OcsfEvent> stream = env
    .fromSource(KafkaSource.<OcsfEvent>builder()
        .setBootstrapServers("redpanda:9092")
        .setTopics("cloudtrail")
        .setGroupId("flink-iceberg-writer")
        .setValueOnlyDeserializer(new OcsfEventDeserializationSchema())
        .build(), WatermarkStrategy.noWatermarks(), "cloudtrail-source");

stream.sinkTo(IcebergSink.<OcsfEvent>forRowData()
    .tableLoader(tableLoader)
    .build());

env.execute("cloudtrail-to-iceberg");
```

**Flink SQL equivalent (simpler, closer to Arroyo):**
```sql
CREATE TABLE cloudtrail_source (
  event_id STRING,
  actor_user_name STRING,
  cloud_region STRING,
  event_time TIMESTAMP(3)
) WITH (
  'connector' = 'kafka',
  'topic' = 'cloudtrail',
  'properties.bootstrap.servers' = 'redpanda:9092',
  'format' = 'json'
);

CREATE TABLE iceberg_sink (
  event_id STRING,
  actor_user_name STRING,
  cloud_region STRING,
  event_time TIMESTAMP(3)
) WITH (
  'connector' = 'iceberg',
  'catalog-name' = 'attest',
  'warehouse' = 's3://attest-warm'
);

INSERT INTO iceberg_sink SELECT * FROM cloudtrail_source;
```

Flink SQL is close to Arroyo SQL in expressiveness. The migration from Arroyo SQL → Flink SQL is mostly mechanical. The hard part is the connector setup.

### 3.3 Add savepoint management to your upgrade process

Every Flink job upgrade requires:

```sh
# 1. Trigger savepoint (can take minutes for large state)
flink savepoint <job-id> s3://attest-checkpoints/savepoints/

# 2. Stop the job
flink cancel <job-id>

# 3. Deploy new version
flink run -s s3://attest-checkpoints/savepoints/<savepoint-id> \
  attest-pipeline-1.1.jar

# 4. Validate state was restored correctly
```

If the state schema changed (new field, renamed field), you need a state migration (`StateDescriptor` evolution or a state-processor job). For pipelines with large keyed state, this is where upgrades stretch from minutes into a multi-step runbook.

With Arroyo, the intended upgrade path is blue-green: start the new version, let it catch up from its S3 checkpoints, cut over. Incompatible state changes still require care, but there is no JVM-serialized state format to migrate.

### 3.4 Tune JVM memory settings (ongoing operational burden)

Every Flink deployment requires memory tuning:

```yaml
# flink-conf.yaml
taskmanager.memory.process.size: 4096m
taskmanager.memory.managed.fraction: 0.4
taskmanager.memory.network.fraction: 0.1
state.backend: rocksdb
state.backend.incremental: true
state.checkpoints.dir: s3://attest-checkpoints/
execution.checkpointing.interval: 30s
execution.checkpointing.mode: EXACTLY_ONCE
```

Getting these wrong causes:
- OOM crashes (managed memory too high, JVM heap starved)
- Checkpoint timeouts (network buffers too small for the event rate)
- GC pauses causing latency spikes (heap too large for G1GC to manage cleanly)

Arroyo doesn't have these knobs because there's no GC and no JVM.

### 3.5 Add Flink-specific connector dependencies

Flink's connector ecosystem requires JAR-level dependency management:

```xml
<!-- pom.xml for Flink job -->
<dependency>
    <groupId>org.apache.flink</groupId>
    <artifactId>flink-connector-kafka</artifactId>
    <version>3.2.0-1.19</version>
</dependency>
<dependency>
    <groupId>org.apache.iceberg</groupId>
    <artifactId>iceberg-flink-runtime-1.19</artifactId>
    <version>1.7.0</version>
</dependency>
```

Connector versions must be pinned to the exact Flink version. A Flink 1.18 → 1.19 upgrade means updating every connector JAR. This is another source of upgrade-day pain.

### 3.6 Summary of changes required

| Change | Effort | Notes |
|---|---|---|
| Add Flink cluster (JobManager + TaskManagers) | Medium | 3+ new services; sizing is workload-dependent |
| Rewrite Arroyo SQL → Flink SQL or DataStream API | Low–Medium | SQL is mostly portable; connector setup is the work |
| Add checkpoint storage (S3 bucket + lifecycle policy) | Low | One-time setup |
| Implement savepoint-based upgrade process | Medium | New runbook, new CI step |
| Tune JVM memory on every deploy | Ongoing | Never fully done |
| Update connector JARs on every Flink upgrade | Ongoing | Version matrix complexity |
| Add Flink monitoring (Flink Web UI, JMX metrics) | Low | New dashboards |
| Add Java/Scala to the team's required skills | High | Cultural / hiring impact |

---

## 4. Arroyo vs. Apache Flink — Full Comparison

### 4.1 Architecture

| Dimension | Apache Flink | Arroyo |
|---|---|---|
| **Runtime** | JVM (Java 11/17), GC-managed heap | Native Rust, no GC |
| **Process model** | JobManager (coordinator) + N TaskManagers | Single controller binary + N worker processes |
| **State backend** | RocksDB (local) or memory, checkpointed to S3/HDFS | Parquet files on S3 (primary), memory for in-flight |
| **Checkpoint format** | JVM-serialized binary blobs (opaque) | Parquet on S3 (readable by other tools) |
| **Checkpoint restore** | Requires compatible Flink version + state schema | Schema-on-read Parquet → more lenient |
| **Pipeline definition** | DataStream Java/Scala API, Table/SQL API, Python (PyFlink) | SQL only (Arroyo SQL, subset of standard) |
| **Execution model** | Push-based dataflow graph with backpressure | Push-based dataflow graph with backpressure |
| **Exactly-once** | Yes (two-phase commit, barrier-based) | Yes (epoch-based, Parquet commit) |

### 4.2 Operational

| Dimension | Apache Flink | Arroyo |
|---|---|---|
| **Upgrade path** | Stop job → savepoint → redeploy → restore | Blue-green deployment, cut over on checkpoint |
| **Memory management** | Manual JVM + managed memory tuning | Rust memory safety, no manual GC tuning |
| **Latency** | Low tens of ms achievable with tuning; tail latency sensitive to GC, buffer timeouts, and checkpoint alignment | No GC pauses; end-to-end latency dominated by batching and checkpoint (epoch) configuration |
| **Throughput ceiling** | Very high — proven at Alibaba (trillions/day) | High, but less battle-tested at extreme scale |
| **Cold-start time** | 30–120s (JVM startup + job initialization) | ~1-3s (native binary startup) |
| **Kubernetes footprint** | Large (JobManager + 3+ TaskManagers recommended) | Small (controller + workers, can run 2 total) |
| **Monitoring** | Flink Web UI, JMX, Prometheus via metrics reporter | Native Prometheus metrics, simpler |
| **Debugging** | Flink Web UI shows per-operator metrics; state inspection via Flink REST API | Less mature debugging tooling |
| **Upgrade complexity** | High — savepoints, connector JAR versions, JVM version | Low — binary swap, S3 checkpoint continuity |

### 4.3 Developer Experience

| Dimension | Apache Flink | Arroyo |
|---|---|---|
| **Language** | Java (primary), Scala, Python (PyFlink) | SQL only (Arroyo SQL) |
| **SQL coverage** | Flink SQL is very mature — most SQL patterns supported | Subset — complex joins and correlated subqueries limited |
| **CEP library** | FlinkCEP — rich, mature n-event pattern matching | Limited CEP support |
| **Connector ecosystem** | Huge — Kafka, Kinesis, Pulsar, JDBC, Iceberg, Hudi, Delta, Elasticsearch, ClickHouse... | Kafka, Kinesis, WebSockets, HTTP, Iceberg, Parquet |
| **Testing** | MiniCluster for unit tests; Flink Testing Utilities | Less mature test framework |
| **Learning curve** | Steep — DataStream API, Table API, state descriptors, watermarks, checkpointing | Gentler — SQL interface, fewer concepts |
| **Community** | Huge (Apache top-level project, 10+ years) | Small but growing (Rust community) |
| **Documentation** | Excellent | Good but thinner |

### 4.4 Scale and Maturity

| Dimension | Apache Flink | Arroyo |
|---|---|---|
| **Production maturity** | Battle-tested (2014+) at Alibaba, LinkedIn, Netflix, Uber | Production-ready but newer (~2023) |
| **Max proven scale** | Trillions of events/day (Alibaba) | Not publicly proven at Flink-scale |
| **Failure handling** | Very mature — automatic restart, partial recovery | Good — Parquet checkpoints, but fewer recovery modes |
| **State size** | Multi-TB state in RocksDB + incremental checkpoints | S3 Parquet — practically unlimited, but higher restore latency |
| **Latency at scale** | Increases as GC pressure grows at high throughput | Stays low — no GC |
| **Cost at scale** | JVM heap + managed memory typically needs more headroom than an equivalent native runtime | Smaller memory headroom → lower cloud cost (benchmark per workload before relying on a ratio) |

---

## 5. Why RisingWave is NOT a Flink replacement (and vice versa)

This is the most common confusion, and it drives the architecture: the two categories are complements, not substitutes.

```
Flink / Arroyo = Stream PROCESSOR
    "Take events, transform them, write results somewhere"
    Source → Operators → Sink
    You define a pipeline; results go to a target (Kafka, S3, DB)
    You cannot query the in-flight state directly

RisingWave = Streaming DATABASE
    "Maintain a query result that stays up to date as data arrives"
    CREATE MATERIALIZED VIEW → always-current result
    You query it like a database (PostgreSQL protocol)
    The state IS the result; you read it directly
```

**The analogy:**
- Flink/Arroyo is like an assembly line: raw materials in, products out, you don't stop the line to inspect the middle
- RisingWave is like a whiteboard that automatically updates: you write a formula once, it always shows the current answer

**In Attest, you need both because:**

| Need | Right tool | Why |
|---|---|---|
| "What are alice's last 30d login regions?" | RisingWave | Point-read on maintained view, sub-millisecond |
| "Write all events to Iceberg in batches of 1000" | Arroyo | Pipeline that reads Kafka, buffers, writes S3 |
| "Detect: user logged in from 3 countries in 5 minutes" | RisingWave | Windowed aggregation over a materialized view |
| "Fan out events to both MinIO and ClickHouse" | Arroyo | Multi-sink pipeline |
| "Is this user's behavior anomalous vs. baseline?" | RisingWave | Correlated subquery against entity_baselines view |
| "Complex CEP: A then B then C within 10 minutes" | Arroyo (or Flink) | Stateful sequence pattern matching |

**This is exactly why neither Arroyo nor Flink replaces RisingWave.** They are complementary. Using only Flink would mean re-implementing RisingWave's materialized view semantics yourself — a huge amount of work. Using only RisingWave would mean hand-writing multi-sink pipeline logic in Rust.

---

## 6. Current Architecture + Arroyo — Full Pro/Con

### Pros

**1. All-Rust runtime — no JVM anywhere in the hot path**
The collector, RisingWave, Arroyo, orchestrator, MCP gateway, and control-plane are all Rust. There are no GC pauses in the data path, which keeps tail latency predictable without GC tuning.

**2. Operational simplicity**
- No Flink cluster to manage (no JobManager HA, no TaskManager memory tuning)
- Arroyo upgrades are blue-green: start new version, let it catch up from S3 checkpoints, cut over
- No JVM-serialized state format to migrate on upgrade

**3. Memory efficiency → lower cost**
A native runtime without a managed heap needs less memory headroom than a JVM at comparable throughput. The size of the saving is workload-specific; measure it before quoting a ratio.

**4. S3 Parquet checkpoints are queryable**
Arroyo checkpoints state as Parquet files on S3. If something goes wrong, you can open the checkpoint in DuckDB or ClickHouse and inspect it. Flink's RocksDB checkpoints are opaque binary blobs.

**5. One language for the team**
The team speaks Rust and SQL. Adopting Flink adds Java/Scala and JVM operations to the required skill set, for one component.

**6. Natural fit with the Iceberg writer**
Arroyo can write Iceberg directly from SQL. `attest-storage-iceberg` (the custom Rust writer) is already structured like an Arroyo pipeline — the migration is more `DELETE FILE` than `REWRITE FILE`.

### Cons

**1. Arroyo's SQL coverage is narrower than Flink SQL**
Flink SQL has a decade of development. Some complex detection patterns that would be a single Flink SQL query may require falling back to custom Rust services in Arroyo. The correlated subquery `NOT IN baseline(user, 90d)` that RisingWave handles natively is one such example — Arroyo may not support it yet.

**2. Less battle-tested at extreme scale**
Flink has been proven at Alibaba running trillions of events per day. Arroyo has not been publicly proven at that scale. For a mid-market security product handling millions of events/day, this is irrelevant — but it is a legitimate concern for the long-term roadmap.

**3. Smaller connector ecosystem**
Flink has community-maintained connectors for dozens of systems (Pulsar, Delta, Hudi, Elasticsearch, JDBC, dozens of cloud services). Arroyo's connector set is smaller. If Attest needs to fan out to a customer's Elasticsearch cluster or a legacy Oracle database, custom connector work is required.

**4. Less mature CEP library**
Flink's `FlinkCEP` library is the gold standard for complex event processing — n-event sequences with rich conditional logic, non-deterministic patterns, time constraints, negation. Arroyo's CEP support is less expressive. For the `~10%` of complex sequence detections, this matters.

**5. Smaller community = slower help when things break**
Flink has a massive community, mature Stack Overflow presence, paid support from Alibaba Cloud (Ververica), Cloudera, and AWS (Kinesis Data Analytics). When Arroyo breaks at 3am, your options are GitHub issues and the Arroyo Discord.

**6. Arroyo is younger — API stability is not guaranteed**
Arroyo is a younger project and its API surface has changed across releases. Production-grade features (0.12+) are stable, but the development pace means minor version upgrades can require pipeline changes. Pin versions and test upgrades in CI.

---

## 7. Revisit triggers

This decision should be reopened if any of the following becomes true:

- A detection requirement needs complex event processing that neither RisingWave views nor Arroyo SQL can express, and it is too central to leave in a custom Rust consumer.
- Sustained throughput reaches a scale where Arroyo has no public production track record and load tests show it falling behind.
- A deployment must fan out to a sink that only has a mature Flink connector.
- The team gains existing Flink expertise and operational tooling that removes most of its operating cost.

---

## 8. The decision framework: when to choose Flink vs. Arroyo vs. RisingWave

A quick-reference guide for making the same call on other systems.

```
Question 1: Do you need a query interface where you ask "what is the current state"?
  YES → RisingWave (or Materialize)
  NO  → continue

Question 2: Do you need complex n-event sequence patterns (A then B then C within T)?
  YES, at massive scale (>1B events/day) → Apache Flink (FlinkCEP)
  YES, at moderate scale               → Arroyo (or Flink if team knows Java)
  NO → continue

Question 3: Do you have a Java/Scala team and existing Flink expertise?
  YES → Apache Flink (don't fight the team's skills)
  NO  → Arroyo

Question 4: Do you need a massive connector ecosystem (Pulsar, Oracle, SAP, Elasticsearch, etc.)?
  YES → Apache Flink
  NO  → Arroyo

Question 5: Is very low, predictable tail latency a hard requirement?
  YES → Prefer a native runtime (Arroyo or RisingWave), and benchmark
  NO  → Either works

Question 6: Do you operate Kubernetes with a large ops team?
  YES → Flink (the ops complexity is manageable with the right team)
  NO  → Arroyo (simpler operational model)
```

**For a security data platform like Attest** (Rust team, sub-second detection requirement, moderate scale at MVP, need both "current state queries" and "pipeline ETL"):
→ **RisingWave for streaming queries + Arroyo for pipelines.** No Flink.

---

## 9. One-page summary (for quick reference)

```
┌─────────────────────────────────────────────────────────────────────┐
│                    STREAMING ENGINE COMPARISON                       │
├────────────────┬────────────────┬─────────────────┬─────────────────┤
│                │  Apache Flink  │     Arroyo      │   RisingWave    │
├────────────────┼────────────────┼─────────────────┼─────────────────┤
│ Runtime        │ JVM            │ Rust (no GC)    │ Rust (no GC)   │
│ GC pauses      │ Yes (JVM)      │ None            │ None           │
│ Model          │ Pipeline       │ Pipeline        │ Database       │
│ Query method   │ Sink to DB     │ Sink to DB      │ PostgreSQL SQL │
│ State storage  │ RocksDB + S3   │ S3 Parquet      │ Hummock on S3  │
│ Upgrades       │ Savepoint dance│ Blue-green       │ Online DDL     │
│ CEP            │ Excellent      │ Limited         │ Not designed   │
│ SQL coverage   │ Very mature    │ Growing subset  │ PostgreSQL-like│
│ Connectors     │ 50+ official   │ ~10 official    │ ~20 official   │
│ Scale proven   │ Trillions/day  │ Not at Flink-   │ Millions/day   │
│                │ (Alibaba)      │ scale publicly  │ (growing)      │
│ Ops complexity │ Very high      │ Low             │ Low            │
│ Team language  │ Java/Scala     │ SQL             │ SQL            │
│ In Attest      │ NOT USED       │ ETL pipelines   │ Detection views│
│                │                │ (~10% of work)  │ (~85% of work) │
└────────────────┴────────────────┴─────────────────┴─────────────────┘
```

---

## 10. References

- Arroyo documentation: https://doc.arroyo.dev
- RisingWave vs. Flink: https://risingwave.com/blog/risingwave-vs-flink
- Flink architecture: https://nightlies.apache.org/flink/flink-docs-stable/docs/concepts/flink-architecture
- Attest architecture: `docs/02_Architecture.md`
- Attest architecture diagrams: `docs/03_Architecture_Diagrams.md`

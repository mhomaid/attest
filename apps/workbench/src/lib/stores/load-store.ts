'use client';

import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';

// ── Types ──────────────────────────────────────────────────────────────────

export interface MetricsSnapshot {
  ts: string;
  events_per_sec: number;
  consumer_lag: number;
  detections_per_sec: number;
  clickhouse_rows_per_sec: number;
  triage_p95_ms: number;
  total_events: number;
  total_detections: number;
  active_load_gen: boolean;
  current_load_gen_rate: number;
}

export interface LoadConfig {
  rate: number;
  duration_secs: number;
  scenario: 'mixed' | 'attack' | 'benign';
  tenants: number;
  seed_baselines: boolean;
  /**
   * Percentage of load events also sent to the orchestrator for ML triage (0–100).
   * 0 = streaming rules only (fastest, no triage latency data).
   * 5 = 5% sampled — real triage p95 without overwhelming the orchestrator.
   */
  sampled_triage_pct: number;
}

export type LoadStatus =
  | 'idle'
  | 'starting'
  | 'running'
  | 'stopping'
  | 'completed'
  | 'error';

export interface LoadState {
  status: LoadStatus;
  config: LoadConfig;
  startedAt: number | null;
  /** Ring buffer — last 300 snapshots (5 min at 1 Hz). Never mutated in-place. */
  history: MetricsSnapshot[];
  current: MetricsSnapshot | null;
  errorMsg: string | null;

  setConfig: (c: Partial<LoadConfig>) => void;
  startRun: () => Promise<void>;
  stopRun: () => Promise<void>;
  ingestSnapshot: (s: MetricsSnapshot) => void;
  reset: () => void;
}

// ── Defaults ───────────────────────────────────────────────────────────────

const DEFAULT_CONFIG: LoadConfig = {
  rate: 10_000,
  duration_secs: 30,
  scenario: 'mixed',
  tenants: 3,
  seed_baselines: true,
  sampled_triage_pct: 0,
};

const HISTORY_MAX = 300;

// ── Store ──────────────────────────────────────────────────────────────────

export const useLoadStore = create<LoadState>()(
  subscribeWithSelector((set, get) => ({
    status: 'idle',
    config: DEFAULT_CONFIG,
    startedAt: null,
    history: [],
    current: null,
    errorMsg: null,

    setConfig(c) {
      set(s => ({ config: { ...s.config, ...c } }));
    },

    async startRun() {
      set({ status: 'starting', errorMsg: null, startedAt: Date.now(), history: [], current: null });
      try {
        const res = await fetch('/api/load/start', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(get().config),
        });
        if (!res.ok) {
          const body = await res.json().catch(() => ({})) as { error?: string };
          throw new Error(body.error ?? `HTTP ${res.status}`);
        }
        set({ status: 'running' });
      } catch (e) {
        set({ status: 'error', errorMsg: e instanceof Error ? e.message : String(e) });
      }
    },

    async stopRun() {
      set({ status: 'stopping' });
      try {
        const res = await fetch('/api/load/stop', { method: 'POST' });
        // 404 means load-gen already stopped on its own — treat as completed.
        if (!res.ok && res.status !== 404) {
          const body = await res.json().catch(() => ({})) as { error?: string };
          throw new Error(body.error ?? `HTTP ${res.status}`);
        }
        set({ status: 'completed' });
      } catch (e) {
        set({ status: 'error', errorMsg: e instanceof Error ? e.message : String(e) });
      }
    },

    ingestSnapshot(s) {
      set(state => {
        // Immutable ring buffer — no in-place mutation.
        const prev = state.history;
        const history =
          prev.length >= HISTORY_MAX
            ? [...prev.slice(-(HISTORY_MAX - 1)), s]
            : [...prev, s];

        // Auto-transition: run finished when load-gen goes inactive after a few ticks.
        const status =
          state.status === 'running' && !s.active_load_gen && prev.length > 5
            ? 'completed'
            : state.status;

        return { current: s, history, status };
      });
    },

    reset() {
      set({
        status: 'idle',
        config: DEFAULT_CONFIG,
        startedAt: null,
        history: [],
        current: null,
        errorMsg: null,
      });
    },
  })),
);

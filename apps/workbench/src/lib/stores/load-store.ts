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
  /** Ring buffer — last 300 snapshots (= 5 min at 1 Hz). */
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
      set({ status: 'starting', errorMsg: null, startedAt: Date.now(), history: [] });
      try {
        const res = await fetch('/api/load/start', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(get().config),
        });
        if (!res.ok) {
          const { error } = await res.json().catch(() => ({ error: `HTTP ${res.status}` }));
          throw new Error(error ?? `HTTP ${res.status}`);
        }
        set({ status: 'running' });
      } catch (e) {
        set({ status: 'error', errorMsg: e instanceof Error ? e.message : String(e) });
      }
    },

    async stopRun() {
      set({ status: 'stopping' });
      try {
        await fetch('/api/load/stop', { method: 'POST' });
        set({ status: 'completed' });
      } catch (e) {
        set({ status: 'error', errorMsg: e instanceof Error ? e.message : String(e) });
      }
    },

    ingestSnapshot(s) {
      set(state => {
        const next = [...state.history, s];
        if (next.length > HISTORY_MAX) next.shift();

        // Auto-transition to completed when the run finishes
        const status =
          state.status === 'running' && !s.active_load_gen && state.history.length > 5
            ? 'completed'
            : state.status;

        return { current: s, history: next, status };
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

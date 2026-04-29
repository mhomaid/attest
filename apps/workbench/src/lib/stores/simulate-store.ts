'use client';

import { create } from 'zustand';

// ── Types ──────────────────────────────────────────────────────────────────

export interface SimRun {
  ts: number;
  scenarioId: string;
  verdict: string;
  latencyMs: number;
  confidence?: number;
}

interface SimulateState {
  /** Last 10 runs — persists across page navigation. */
  history: SimRun[];
  pushRun: (entry: SimRun) => void;
  clearHistory: () => void;
}

// ── Store ──────────────────────────────────────────────────────────────────

export const useSimulateStore = create<SimulateState>()(set => ({
  history: [],

  pushRun(entry) {
    set(s => {
      const next = [entry, ...s.history].slice(0, 10);
      return { history: next };
    });
  },

  clearHistory() {
    set({ history: [] });
  },
}));

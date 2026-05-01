"use client";

/**
 * Unified case store — persisted to sessionStorage so events, verdicts, and
 * trace steps survive navigation, hard refresh, and "event no longer in hot
 * tier" errors.
 *
 * Architecture rules:
 *  • One store entry per case (keyed by event_id / UUID).
 *  • Maximum 50 cases retained; oldest entries are evicted automatically.
 *  • Use `skipHydration: true` so SSR and client produce identical HTML.
 *    Call `useCaseStore.persist.rehydrate()` inside a useEffect to hydrate.
 */

import { create } from "zustand";
import { createJSONStorage, persist } from "zustand/middleware";
import type { OcsfEvent } from "@/lib/ocsf-to-alert";
import type { KafkaTraceStep } from "@/hooks/use-case-trace-ws";

// ── Types ─────────────────────────────────────────────────────────────────────

export type Disposition = "none" | "approved" | "override";
export type TriageStatus = "idle" | "running" | "complete" | "failed";

export type TriageVerdict = {
  action_id: string;
  case_id: string;
  verdict: string;
  execution_path: string;
  calibrated_confidence: number;
  novelty_score: number;
  escalated: boolean;
  escalation_reason?: string;
  latency_ms: number;
  classifier_evidence?: {
    input_features: Record<string, number>;
    shap_values: Record<string, number>;
    raw_prediction: number;
    calibrated_confidence: number;
    novelty_score: number;
  };
};

export type CaseEntry = {
  /** Snapshot of the OCSF event fetched on first open. */
  event: OcsfEvent;
  /** User baseline fetched on first open — may be null. */
  baseline: { regions_seen_30d?: string[] } | null;
  /** Set when triage completes. */
  verdict: TriageVerdict | null;
  /** Persisted job state so refresh/navigation does not start duplicate LLM work. */
  triageStatus: TriageStatus;
  triageStartedAt?: number;
  triageError?: string;
  /** Kafka trace steps streamed from ws-gateway. */
  traceSteps: KafkaTraceStep[];
  /** Analyst disposition. */
  disposition: Disposition;
  overrideLabel?: string;
  /** Epoch ms when the entry was last written — used for eviction. */
  updatedAt: number;
};

const MAX_CASES = 50;

type CaseStore = {
  cases: Record<string, CaseEntry>;
  // ── Writes ──────────────────────────────────────────────────────────────────
  /** Upsert event + baseline on first case open. Skips if already present. */
  initCase: (caseId: string, event: OcsfEvent, baseline: CaseEntry["baseline"]) => void;
  /** Mark triage as running before POSTing to the orchestrator. */
  startTriage: (caseId: string) => void;
  /** Store the triage verdict when it arrives. */
  setVerdict: (caseId: string, verdict: TriageVerdict) => void;
  /** Store a triage failure. */
  failTriage: (caseId: string, error: string) => void;
  /** Append a live trace step (deduped by agent_action_id:step_kind:ts). */
  appendTraceStep: (caseId: string, step: KafkaTraceStep) => void;
  /** Clear trace steps (on new case session). */
  clearTraceSteps: (caseId: string) => void;
  /** Record analyst approval. */
  setApproved: (caseId: string) => void;
  /** Record analyst override. */
  setOverride: (caseId: string, label: string) => void;
  /** Evict a single entry. */
  evict: (caseId: string) => void;
};

// ── Store ─────────────────────────────────────────────────────────────────────

export const useCaseStore = create<CaseStore>()(
  persist(
    (set, get) => ({
      cases: {},

      initCase: (caseId, event, baseline) => {
        const existing = get().cases[caseId];
        // If already initialised, just update baseline/event snapshot but keep
        // verdict and trace steps.
        set((s) => {
          const cases = evictIfNeeded({ ...s.cases }, caseId);
          return {
            cases: {
              ...cases,
              [caseId]: {
                event,
                baseline,
                verdict: existing?.verdict ?? null,
                triageStatus: existing?.triageStatus ?? "idle",
                triageStartedAt: existing?.triageStartedAt,
                triageError: existing?.triageError,
                traceSteps: existing?.traceSteps ?? [],
                disposition: existing?.disposition ?? "none",
                overrideLabel: existing?.overrideLabel,
                updatedAt: Date.now(),
              },
            },
          };
        });
      },

      startTriage: (caseId) =>
        set((s) => {
          const entry = s.cases[caseId];
          if (!entry) return s;
          return {
            cases: {
              ...s.cases,
              [caseId]: {
                ...entry,
                triageStatus: "running",
                triageStartedAt: Date.now(),
                triageError: undefined,
                updatedAt: Date.now(),
              },
            },
          };
        }),

      setVerdict: (caseId, verdict) =>
        set((s) => {
          const entry = s.cases[caseId];
          if (!entry) return s;
          return {
            cases: {
              ...s.cases,
              [caseId]: {
                ...entry,
                verdict,
                triageStatus: "complete",
                triageError: undefined,
                updatedAt: Date.now(),
              },
            },
          };
        }),

      failTriage: (caseId, error) =>
        set((s) => {
          const entry = s.cases[caseId];
          if (!entry) return s;
          return {
            cases: {
              ...s.cases,
              [caseId]: {
                ...entry,
                triageStatus: "failed",
                triageError: error,
                updatedAt: Date.now(),
              },
            },
          };
        }),

      appendTraceStep: (caseId, step) =>
        set((s) => {
          const entry = s.cases[caseId];
          if (!entry) return s;
          const key = `${step.agent_action_id}:${step.step_kind}:${step.ts}`;
          if (entry.traceSteps.some((t) => `${t.agent_action_id}:${t.step_kind}:${t.ts}` === key))
            return s;
          return {
            cases: {
              ...s.cases,
              [caseId]: {
                ...entry,
                traceSteps: [step, ...entry.traceSteps],
                updatedAt: Date.now(),
              },
            },
          };
        }),

      clearTraceSteps: (caseId) =>
        set((s) => {
          const entry = s.cases[caseId];
          if (!entry) return s;
          return {
            cases: {
              ...s.cases,
              [caseId]: { ...entry, traceSteps: [], updatedAt: Date.now() },
            },
          };
        }),

      setApproved: (caseId) =>
        set((s) => {
          const entry = s.cases[caseId];
          if (!entry) return s;
          return {
            cases: {
              ...s.cases,
              [caseId]: { ...entry, disposition: "approved", updatedAt: Date.now() },
            },
          };
        }),

      setOverride: (caseId, label) =>
        set((s) => {
          const entry = s.cases[caseId];
          if (!entry) return s;
          return {
            cases: {
              ...s.cases,
              [caseId]: {
                ...entry,
                disposition: "override",
                overrideLabel: label,
                updatedAt: Date.now(),
              },
            },
          };
        }),

      evict: (caseId) =>
        set((s) => {
          const next = { ...s.cases };
          delete next[caseId];
          return { cases: next };
        }),
    }),
    {
      name: "attest-cases-v1",
      storage: createJSONStorage(() => {
        // Gracefully fall back when sessionStorage is unavailable (SSR, private mode).
        if (typeof window === "undefined") return noopStorage;
        return sessionStorage;
      }),
      // Never hydrate during SSR — call rehydrate() inside useEffect.
      skipHydration: true,
    },
  ),
);

// ── Selectors ─────────────────────────────────────────────────────────────────

// Stable fallbacks — NEVER return a new literal from a selector, it causes
// Zustand's snapshot comparison to see a change on every render (infinite loop).
const EMPTY_STEPS: KafkaTraceStep[] = [];

export const selectCase = (caseId: string) => (s: CaseStore) => s.cases[caseId] ?? null;
export const selectVerdict = (caseId: string) => (s: CaseStore) => s.cases[caseId]?.verdict ?? null;
/** Use with useShallow() to avoid infinite re-render loops on array comparisons. */
export const selectTraceSteps = (caseId: string) => (s: CaseStore) =>
  s.cases[caseId]?.traceSteps ?? EMPTY_STEPS;
export const selectDisposition = (caseId: string) => (s: CaseStore) =>
  s.cases[caseId]?.disposition ?? ("none" as const);

// ── Helpers ───────────────────────────────────────────────────────────────────

function evictIfNeeded(
  cases: Record<string, CaseEntry>,
  incomingId: string,
): Record<string, CaseEntry> {
  if (Object.keys(cases).length < MAX_CASES) return cases;
  // Remove the oldest entry that isn't the incoming case.
  const sorted = Object.entries(cases)
    .filter(([id]) => id !== incomingId)
    .sort(([, a], [, b]) => a.updatedAt - b.updatedAt);
  const toEvict = sorted[0];
  if (!toEvict) return cases;
  const next = { ...cases };
  delete next[toEvict[0]];
  return next;
}

/** No-op storage used during SSR. */
const noopStorage = {
  getItem: () => null,
  setItem: () => undefined,
  removeItem: () => undefined,
};

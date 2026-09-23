"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { env } from "@/lib/env";
import { useCaseStore } from "@/lib/stores/case-store";

export type WsTraceStatus = "connecting" | "connected" | "disconnected";

export type KafkaTraceStep = {
  case_id: string;
  tenant_id: string;
  agent_action_id: string;
  agent_id: string;
  execution_path: string;
  step_kind: string;
  summary: string;
  ts: number;
};

function wsBase(): string {
  if (typeof window === "undefined") return "ws://localhost:4500";
  return env.NEXT_PUBLIC_WS_GATEWAY_URL;
}

/**
 * Live tail of `agent.trace_steps` via ws-gateway (JWT from `/api/auth/ws-token`).
 *
 * Steps are written into the Zustand case store so they:
 *   • Survive navigation away and back (persisted in sessionStorage).
 *   • Are deduplicated across reconnects.
 *   • Drive all trace-related UI through a single source of truth.
 */
export function useCaseTraceWs(caseId: string | null) {
  const appendTraceStep = useCaseStore((s) => s.appendTraceStep);
  // useShallow does a shallow-equal check on the array so Zustand only
  // re-renders when the contents actually change, not on every selector call.
  const liveSteps = useCaseStore(
    useShallow((s) => (caseId ? s.cases[caseId]?.traceSteps : null) ?? []),
  );

  const [wsStatus, setWsStatus] = useState<WsTraceStatus>("connecting");
  const wsRef = useRef<WebSocket | null>(null);
  const retryMs = useRef(500);
  const retryTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const connectRef = useRef<() => void>(() => {});

  const connect = useCallback(() => {
    if (!caseId) {
      setWsStatus("disconnected");
      return;
    }

    void (async () => {
      // Fetch a short-lived JWT for this case.
      let token: string;
      try {
        const res = await fetch("/api/auth/ws-token", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ case_id: caseId }),
        });
        if (!res.ok) {
          setWsStatus("disconnected");
          retryTimer.current = setTimeout(() => {
            retryMs.current = Math.min(retryMs.current * 2, 32_000);
            connectRef.current();
          }, retryMs.current);
          return;
        }
        const json = (await res.json()) as { token?: string };
        if (!json.token) { setWsStatus("disconnected"); return; }
        token = json.token;
      } catch {
        setWsStatus("disconnected");
        retryTimer.current = setTimeout(() => {
          retryMs.current = Math.min(retryMs.current * 2, 32_000);
          connectRef.current();
        }, retryMs.current);
        return;
      }

      const url = `${wsBase().replace(/\/$/, "")}/ws/cases/${encodeURIComponent(caseId)}/trace?token=${encodeURIComponent(token)}`;
      const ws = new WebSocket(url);
      wsRef.current = ws;

      ws.onopen = () => {
        setWsStatus("connected");
        retryMs.current = 500;
      };

      ws.onmessage = (evt) => {
        try {
          const raw = typeof evt.data === "string" ? evt.data : "";
          const parsed = JSON.parse(raw) as Record<string, unknown>;
          if (parsed.kind === "backpressure_dropped") return;
          const step = parsed as unknown as KafkaTraceStep;
          if (!step.step_kind || !step.agent_action_id) return;
          // Dedup + persist — the store selector drives all trace UI.
          appendTraceStep(caseId, step);
        } catch { /* ignore */ }
      };

      ws.onclose = () => {
        setWsStatus("disconnected");
        retryTimer.current = setTimeout(() => {
          retryMs.current = Math.min(retryMs.current * 2, 32_000);
          connectRef.current();
        }, retryMs.current);
      };

      ws.onerror = () => ws.close();
    })();
  }, [caseId, appendTraceStep]);

  useEffect(() => { connectRef.current = connect; }, [connect]);

  useEffect(() => {
    retryMs.current = 500;
    queueMicrotask(() => {
      setWsStatus("connecting");
      connect();
    });
    return () => {
      if (retryTimer.current) clearTimeout(retryTimer.current);
      retryMs.current = 500;
      wsRef.current?.close();
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [caseId]);

  return { liveSteps, wsStatus };
}

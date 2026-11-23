"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { Alert } from "@/lib/mock-data";
import { firedDetectionToAlert, type FiredDetection } from "@/lib/detection-to-alert";
import { env } from "@/lib/env";

export type WsStatus = "connecting" | "connected" | "disconnected";

/**
 * Connects to the control-plane WebSocket at NEXT_PUBLIC_CP_WS_URL.
 * A short-lived service token is fetched from the workbench first; the secret
 * never reaches the browser. Reconnects with exponential back-off (1s → 32s).
 */
export function useAlertsWs() {
  const [newAlerts, setNewAlerts] = useState<Alert[]>([]);
  const [wsStatus, setWsStatus] = useState<WsStatus>("connecting");
  const wsRef = useRef<WebSocket | null>(null);
  const retryMs = useRef(1000);
  const retryTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const seenIds = useRef<Set<string>>(new Set());
  const connectRef = useRef<() => void>(() => {});
  const cancelled = useRef(false);

  const connect = useCallback(() => {
    const wsUrl = env.NEXT_PUBLIC_CP_WS_URL;
    void (async () => {
      const issued = await fetch("/api/auth/service-token?audience=control-plane").then(
        async (res) => (res.ok ? ((await res.json()) as { token?: string }) : null),
      );
      if (cancelled.current) return;
      const token = issued?.token;
      const url = token
        ? `${wsUrl}/v1/ws/alerts?access_token=${encodeURIComponent(token)}`
        : `${wsUrl}/v1/ws/alerts`;
      const ws = new WebSocket(url);
      wsRef.current = ws;

      ws.onopen = () => {
        setWsStatus("connected");
        retryMs.current = 1000;
      };

      ws.onmessage = (evt) => {
        try {
          const fired = JSON.parse(evt.data as string) as FiredDetection;
          const key = `${fired.detection_id}:${fired.event_id}`;
          if (seenIds.current.has(key)) return;
          seenIds.current.add(key);
          setNewAlerts((prev) => [firedDetectionToAlert(fired), ...prev]);
        } catch {
          // ignore malformed messages
        }
      };

      ws.onclose = () => {
        setWsStatus("disconnected");
        if (cancelled.current) return;
        retryTimer.current = setTimeout(() => {
          retryMs.current = Math.min(retryMs.current * 2, 32_000);
          connectRef.current();
        }, retryMs.current);
      };

      ws.onerror = () => {
        ws.close();
      };
    })();
  }, []);

  useEffect(() => {
    connectRef.current = connect;
  }, [connect]);

  useEffect(() => {
    cancelled.current = false;
    connect();
    return () => {
      cancelled.current = true;
      if (retryTimer.current) clearTimeout(retryTimer.current);
      wsRef.current?.close();
    };
  }, [connect]);

  return { newAlerts, wsStatus };
}

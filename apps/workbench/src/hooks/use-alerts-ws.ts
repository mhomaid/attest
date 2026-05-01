"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import type { Alert } from "@/lib/mock-data";
import { firedDetectionToAlert, type FiredDetection } from "@/lib/detection-to-alert";

export type WsStatus = "connecting" | "connected" | "disconnected";

/**
 * Connects to the control-plane WebSocket at NEXT_PUBLIC_CP_WS_URL.
 * Each message from the server is a JSON-serialised FiredDetection.
 * The hook returns the accumulated list of new alerts received since mount,
 * plus a status indicator.
 *
 * Reconnects automatically with exponential back-off (1s → 32s).
 */
export function useAlertsWs() {
  const [newAlerts, setNewAlerts] = useState<Alert[]>([]);
  const [wsStatus, setWsStatus]   = useState<WsStatus>("connecting");
  const wsRef      = useRef<WebSocket | null>(null);
  const retryMs    = useRef(1000);
  const retryTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Dedup within the session to avoid showing the same event twice if the
  // server reconnects and replays.
  const seenIds = useRef<Set<string>>(new Set());
  const connectRef = useRef<() => void>(() => {});

  const connect = useCallback(() => {
    const wsUrl =
      (typeof window !== "undefined" &&
        process.env.NEXT_PUBLIC_CP_WS_URL) ||
      "ws://localhost:8080";

    const ws = new WebSocket(`${wsUrl}/v1/ws/alerts`);
    wsRef.current = ws;

    ws.onopen = () => {
      setWsStatus("connected");
      retryMs.current = 1000; // reset back-off on successful connect
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
      // Exponential back-off: 1s, 2s, 4s … 32s
      retryTimer.current = setTimeout(() => {
        retryMs.current = Math.min(retryMs.current * 2, 32_000);
        connectRef.current();
      }, retryMs.current);
    };

    ws.onerror = () => {
      ws.close(); // triggers onclose → retry
    };
  }, []);

  useEffect(() => {
    connectRef.current = connect;
  }, [connect]);

  useEffect(() => {
    connect();
    return () => {
      if (retryTimer.current) clearTimeout(retryTimer.current);
      wsRef.current?.close();
    };
  }, [connect]);

  return { newAlerts, wsStatus };
}

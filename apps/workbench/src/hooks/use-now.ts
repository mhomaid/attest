"use client";

import { useCallback, useSyncExternalStore } from "react";

/**
 * Current time in ms, refreshed every `intervalMs` and rounded down to it so the
 * snapshot stays stable between ticks. `null` during SSR and hydration, which keeps
 * server and client HTML identical for time-relative labels.
 */
export function useNow(intervalMs: number): number | null {
  const subscribe = useCallback(
    (onChange: () => void) => {
      const id = setInterval(onChange, intervalMs);
      return () => clearInterval(id);
    },
    [intervalMs],
  );
  const getSnapshot = useCallback(
    () => Math.floor(Date.now() / intervalMs) * intervalMs,
    [intervalMs],
  );
  return useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
}

const getServerSnapshot = () => null;

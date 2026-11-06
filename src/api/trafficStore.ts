import type { TrafficTick } from "../bindings";

/** Number of total-rate points kept for the overview sparklines. */
export const HISTORY_LEN = 60;

export interface TrafficHistory {
  up: number[];
  down: number[];
}

const EMPTY: TrafficHistory = { up: [], down: [] };

let latest: TrafficTick | null = null;
// Replaced (never mutated) on each push so useSyncExternalStore sees a new snapshot.
let history: TrafficHistory = EMPTY;
const subs = new Set<() => void>();

/** Corrections update the cards without inventing a sample in the chart. */
export function setTraffic(t: TrafficTick | null, recordHistory = true): void {
  latest = t;
  if (t !== null && recordHistory) {
    history = {
      up: [...history.up, t.total_rate.up].slice(-HISTORY_LEN),
      down: [...history.down, t.total_rate.down].slice(-HISTORY_LEN),
    };
  }
  subs.forEach((f) => f());
}

export function getTraffic(): TrafficTick | null {
  return latest;
}

export function getTrafficHistory(): TrafficHistory {
  return history;
}

export function subscribeTraffic(f: () => void): () => void {
  subs.add(f);
  return () => {
    subs.delete(f);
  };
}

/** Test helper: forget all ticks. */
export function resetTrafficStore(): void {
  latest = null;
  history = EMPTY;
}

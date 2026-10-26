import type { TrafficTick } from "../bindings";

let latest: TrafficTick | null = null;
const subs = new Set<() => void>();

export function setTraffic(t: TrafficTick | null): void {
  latest = t;
  subs.forEach((f) => f());
}

export function getTraffic(): TrafficTick | null {
  return latest;
}

export function subscribeTraffic(f: () => void): () => void {
  subs.add(f);
  return () => {
    subs.delete(f);
  };
}

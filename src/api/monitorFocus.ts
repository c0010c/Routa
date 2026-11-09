import { useSyncExternalStore } from "react";
import type { MonitorFocus } from "../bindings";

// A tiny store: the focus comes from a backend event (toast click) and is consumed by
// whichever page is showing, which may mount only after the navigation it triggers.
let focus: MonitorFocus | null = null;
const subs = new Set<() => void>();

export function setMonitorFocus(next: MonitorFocus): void {
  focus = next;
  subs.forEach((f) => f());
}

export function clearMonitorFocus(): void {
  if (focus === null) return;
  focus = null;
  subs.forEach((f) => f());
}

export function useMonitorFocus(): MonitorFocus | null {
  return useSyncExternalStore(
    (f) => {
      subs.add(f);
      return () => subs.delete(f);
    },
    () => focus,
  );
}

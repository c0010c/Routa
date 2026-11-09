// Prototype state for node monitoring. Lives only in the browser preview.
import { useSyncExternalStore } from "react";
import type { ExitMember, Model } from "../../src/bindings";

export type Scope = "inUse" | "custom";

export interface NodeState { fails: number; down: boolean; downSince?: number; lastMs: number | null; lastError?: string }

export interface Toast { id: number; title: string; body: string; proxy?: string; name?: string; kind: "down" | "up" | "network" }

export interface MonitorState {
  enabled: boolean;
  scope: Scope;
  members: ExitMember[];
  intervalSecs: number;
  failThreshold: number;
  latencyLimit: number | null;
  notifyRecovery: boolean;
  lastRun: number | null;
  probing: boolean;
  nodes: Record<string, NodeState>;
  /** Prototype controls: proxies forced to fail, and whole-network outage. */
  broken: string[];
  offline: boolean;
  toasts: Toast[];
  /** Simulated wall clock in seconds, advanced by "快进". */
  clock: number;
}

/** Nodes the fixture's listeners and process rules currently use. */
export const IN_USE: { proxy: string; usedBy: string }[] = [
  { proxy: "示例订阅 A/新加坡 · 狮城 01", usedBy: "工作代理 :7890" },
  { proxy: "示例订阅 B/香港 · 中环 01", usedBy: "进程分流 · 微信" },
  { proxy: "手动/办公 SOCKS5", usedBy: "进程分流 · 企业微信" },
];

let state: MonitorState = {
  enabled: false,
  scope: "inUse",
  members: [],
  intervalSecs: 300,
  failThreshold: 2,
  latencyLimit: 1000,
  notifyRecovery: true,
  lastRun: null,
  probing: false,
  nodes: {},
  broken: [],
  offline: false,
  toasts: [],
  clock: Math.floor(Date.now() / 1000),
};
const listeners = new Set<() => void>();

export function getMonitor() { return state; }
export function setMonitor(patch: Partial<MonitorState> | ((s: MonitorState) => Partial<MonitorState>)) {
  state = { ...state, ...(typeof patch === "function" ? patch(state) : patch) };
  listeners.forEach((l) => l());
}
export function useMonitor() {
  return useSyncExternalStore((l) => { listeners.add(l); return () => listeners.delete(l); }, () => state);
}

export function proxyName(model: Model, m: ExitMember): string[] {
  if ("subscription_id" in m) {
    const s = model.subscriptions.find((x) => x.id === m.subscription_id);
    return s ? [`${s.name}/${m.name}`] : [];
  }
  if ("manual_id" in m) {
    const n = model.manual_nodes.find((x) => x.id === m.manual_id);
    return n ? [`手动/${n.name}`] : [];
  }
  if ("group" in m) {
    const g = m.group;
    if ("Subscription" in g) {
      const s = model.subscriptions.find((x) => x.id === g.Subscription);
      return s ? s.nodes.map((n) => `${s.name}/${n.name}`) : [];
    }
    const grp = model.node_groups.find((x) => x.id === g.Custom);
    return grp ? grp.members.flatMap((r) => proxyName(model, r as ExitMember)) : [];
  }
  return [];
}

export function memberOf(model: Model, proxy: string): ExitMember | null {
  const [source, ...rest] = proxy.split("/");
  const name = rest.join("/");
  if (source === "手动") {
    const n = model.manual_nodes.find((x) => x.name === name);
    return n ? { manual_id: n.id } : null;
  }
  const s = model.subscriptions.find((x) => x.name === source);
  return s ? { subscription_id: s.id, name } : null;
}

export function monitoredProxies(model: Model | undefined, s: MonitorState = state): string[] {
  if (!s.enabled || !model) return [];
  if (s.scope === "inUse") return IN_USE.map((x) => x.proxy);
  return [...new Set(s.members.flatMap((m) => proxyName(model, m)))];
}

export function displayName(proxy: string) { return proxy.split("/").slice(1).join("/"); }

let toastId = 1;
export function pushToast(t: Omit<Toast, "id">) {
  const toast = { ...t, id: toastId++ };
  setMonitor((s) => ({ toasts: [...s.toasts, toast].slice(-3) }));
  setTimeout(() => dismissToast(toast.id), 8000);
}
export function dismissToast(id: number) {
  setMonitor((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
}

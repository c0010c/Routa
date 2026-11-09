import type {
  ExitMember,
  MonitorFailure,
  MonitorStatus,
  MonitorTarget,
  NodeMonitorSettings,
  NodeRef,
} from "../bindings";
import { memberKey, nodeKey } from "./refs";

export const MONITOR_INTERVALS = [10, 30, 60, 180, 300, 600, 1800, 3600];

export function intervalLabel(secs: number): string {
  if (secs < 60) return `${secs} 秒`;
  if (secs < 3600) return `${secs / 60} 分钟`;
  return `${secs / 3600} 小时`;
}

/**
 * The currently monitored targets as single-node members. Several paths to one node collapse
 * into one member; auto-selected exits (no node) cannot be members and are reported as dropped.
 */
export function customMembersFrom(status: MonitorStatus): { members: ExitMember[]; dropped: string[] } {
  const members: ExitMember[] = [];
  const dropped: string[] = [];
  const seen = new Set<string>();
  for (const t of status.targets) {
    if (!t.node) {
      if (!dropped.includes(t.label)) dropped.push(t.label);
      continue;
    }
    const key = nodeKey(t.node);
    if (seen.has(key)) continue;
    seen.add(key);
    members.push(t.node);
  }
  return { members, dropped };
}

export function withNode(
  s: NodeMonitorSettings,
  status: MonitorStatus,
  node: NodeRef,
): { settings: NodeMonitorSettings; dropped: string[] } {
  if (!s.enabled) {
    return { settings: { ...s, enabled: true, scope: "Custom", members: [node] }, dropped: [] };
  }
  const base = s.scope === "InUse" ? customMembersFrom(status) : { members: s.members, dropped: [] };
  const members = base.members.some((m) => memberKey(m) === nodeKey(node)) ? base.members : [...base.members, node];
  return { settings: { ...s, scope: "Custom", members }, dropped: base.dropped };
}

export function withoutNode(
  s: NodeMonitorSettings,
  status: MonitorStatus,
  node: NodeRef,
): { settings: NodeMonitorSettings; dropped: string[] } {
  const { members, dropped } = customMembersFrom(status);
  const key = nodeKey(node);
  return {
    settings: { ...s, scope: "Custom", members: members.filter((m) => memberKey(m) !== key) },
    dropped,
  };
}

export function targetsForNode(status: MonitorStatus, node: NodeRef): MonitorTarget[] {
  const key = nodeKey(node);
  return status.targets.filter((t) => t.node && nodeKey(t.node) === key);
}

export function failureText(f: MonitorFailure): string {
  if (f === "Timeout") return "超时";
  if (f === "Failed") return "连接失败";
  return `延迟 ${f.OverLimit} ms 超过上限`;
}

export function droppedMessage(dropped: string[]): string {
  return `自动选择的出口${dropped.map((n) => `「${n}」`).join("、")}不能加入指定节点，已不再监测`;
}

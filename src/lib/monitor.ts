import type {
  ExitMember,
  InUseNodes,
  Model,
  MonitorFailure,
  MonitorStatus,
  MonitorTarget,
  NodeMonitorSettings,
  NodeRef,
} from "../bindings";
import { commands } from "../bindings";
import { unwrap } from "../api/client";
import { expandMembers, isGroupMember, memberKey, nodeKey } from "./refs";

export const MONITOR_INTERVALS = [10, 30, 60, 180, 300, 600, 1800, 3600];
export const MONITOR_THRESHOLDS = [1, 2, 3, 5];
export const EMPTY_MONITOR_STATUS: MonitorStatus = {
  last_run: null, next_run: null, probing: false, paused_reason: null, network_down: false, targets: [],
};

export function intervalLabel(secs: number): string {
  if (secs < 60) return `${secs} 秒`;
  if (secs < 3600) return `${secs / 60} 分钟`;
  return `${secs / 3600} 小时`;
}

/** The nodes proxies and process routing use now; nothing when the backend cannot say. */
export async function fetchInUseNodes(): Promise<InUseNodes> {
  try {
    return unwrap(await commands.monitorInUseNodes());
  } catch {
    return { nodes: [], auto_exits: [] };
  }
}

/**
 * Members after adding `node`. Turning monitoring on with no members starts from the
 * nodes in use (`inUse`), so adding one node does not leave the rest unwatched.
 */
export function withNode(s: NodeMonitorSettings, node: NodeRef, inUse: NodeRef[]): NodeMonitorSettings {
  const base: ExitMember[] = !s.enabled && s.members.length === 0 ? inUse : s.members;
  const members = base.some((m) => memberKey(m) === nodeKey(node)) ? base : [...base, node];
  return { ...s, enabled: true, members };
}

/**
 * Members after removing `node`, edited in place: a node member is dropped; a group that
 * contains the node is replaced by its other nodes (a group cannot exclude one node).
 * Every other member is kept as it is.
 */
export function withoutNode(s: NodeMonitorSettings, model: Model, node: NodeRef): NodeMonitorSettings {
  const key = nodeKey(node);
  const members: ExitMember[] = [];
  const push = (m: ExitMember) => {
    if (!members.some((x) => memberKey(x) === memberKey(m))) members.push(m);
  };
  for (const m of s.members) {
    if (!isGroupMember(m)) {
      if (memberKey(m) !== key) push(m);
      continue;
    }
    const nodes = expandMembers(model, [m]);
    if (!nodes.some((n) => nodeKey(n) === key)) push(m);
    else nodes.filter((n) => nodeKey(n) !== key).forEach(push);
  }
  return { ...s, members };
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

/** Auto-selecting exits in use have no single node, so they are not prefilled. */
export function skippedMessage(exits: string[]): string {
  return `自动选择的出口${exits.map((n) => `「${n}」`).join("、")}没有固定节点，未加入监测`;
}

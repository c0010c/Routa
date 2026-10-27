import type { ApplyOutcome, ExitStatus, ListenerId, ListenerStatus, RuntimeStatus } from "../bindings";

export type LabelColor = "success" | "default" | "warning" | "error";
export interface Label {
  text: string;
  color: LabelColor;
}

const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

export function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0 B";
  let i = 0;
  let v = n;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  if (i === 0) return `${Math.round(v)} B`;
  return `${parseFloat(v.toFixed(1))} ${UNITS[i]}`;
}

export function formatRate(n: number): string {
  return `${formatBytes(n)}/s`;
}

export function exitStatusLabel(s: ExitStatus): Label {
  switch (s) {
    case "Ok":
      return { text: "正常", color: "success" };
    case "NodeMissing":
      return { text: "节点已失效", color: "error" };
    case "AllMembersMissing":
      return { text: "组内节点全部失效", color: "error" };
    case "InterfaceUnavailable":
      return { text: "网卡不可用", color: "error" };
  }
}

/**
 * True when listener `id` as saved is not what mihomo runs: core not running,
 * the last apply closed everything, or mihomo's current config lacks it (e.g.
 * a retry from the fail-closed empty config was rejected again).
 */
export function listenerNotEffective(status: RuntimeStatus, id: ListenerId): boolean {
  if (status.core !== "Running") return true;
  const a = status.last_apply;
  if (a !== null && ("FailedClosed" in a || "FailedCoreStopped" in a)) return true;
  return !status.effective_listeners.includes(id);
}

export function listenerStatusLabel(s: ListenerStatus, isPortFailed: boolean, notEffective = false): Label {
  if (notEffective && s === "Active") return { text: "未生效", color: "warning" };
  if (isPortFailed) return { text: "端口被占用", color: "error" };
  if (s === "Active") return { text: "运行中", color: "success" };
  if (s === "DisabledByUser") return { text: "已停用", color: "default" };
  if (s === "NoExit") return { text: "未接出口", color: "warning" };
  switch (s.ExitBroken.exit_status) {
    case "NodeMissing":
      return { text: "节点已失效，请重选", color: "error" };
    case "AllMembersMissing":
      return { text: "组内节点全部失效", color: "error" };
    case "InterfaceUnavailable":
      return { text: "网卡不可用", color: "error" };
    case "Ok":
      // Should not happen: a broken listener always has a non-Ok exit.
      return { text: "出口异常", color: "error" };
  }
}

/** User-facing lines for an apply outcome; empty means nothing to report. */
export function applyOutcomeMessages(o: ApplyOutcome): string[] {
  const uncutLine = "部分旧连接未能断开，正在重启 mihomo";
  if ("Reloaded" in o) {
    const lines = o.Reloaded.failed_listeners.map(
      (f) => `端口 ${f.port} 已被其他程序占用，Listener「${f.name}」未启动`,
    );
    if (o.Reloaded.uncut > 0) lines.push(uncutLine);
    return lines;
  }
  if ("FailedKeptPrevious" in o) {
    return [`已保存，但未生效：${o.FailedKeptPrevious.error}`];
  }
  if ("FailedClosed" in o) {
    const lines = [`已保存，但未能生效，为防止泄露已关闭所有端口：${o.FailedClosed.error}`];
    if (o.FailedClosed.uncut > 0) lines.push(uncutLine);
    return lines;
  }
  if ("NotRunning" in o) return [`已保存，但未生效：${o.NotRunning.error}`];
  return [`已保存，但未能生效，mihomo 已停止：${o.FailedCoreStopped.error}`];
}

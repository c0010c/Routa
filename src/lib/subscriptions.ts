import type { ApplyOutcome, ExitMember, Model, NodeRef, RuntimeStatus, Subscription } from "../bindings";
import { ruleLabel } from "./processRule";

export function subscriptionHost(url: string): string {
  try { return new URL(url).hostname; } catch { return "地址无效"; }
}

export function subscriptionTime(time: number | null | undefined): string {
  return time == null ? "尚无记录" : new Date(time * 1000).toLocaleString("zh-CN", { month: "numeric", day: "numeric", hour: "2-digit", minute: "2-digit" });
}

export function applicationProblem(outcome: ApplyOutcome | null | undefined): string | null {
  if (!outcome || "NotRunning" in outcome) return null;
  if ("Reloaded" in outcome) return outcome.Reloaded.failed_listeners.length || outcome.Reloaded.uncut ? "配置未完全生效，请检查监听端口和现有连接" : null;
  if ("FailedKeptPrevious" in outcome) return `新配置未生效，仍使用原配置：${outcome.FailedKeptPrevious.error}`;
  if ("FailedClosed" in outcome) return `新配置未生效，相关代理已关闭：${outcome.FailedClosed.error}`;
  return `内核已停止：${outcome.FailedCoreStopped.error}`;
}

export function subscriptionProblem(sub: Subscription, runtime?: Pick<RuntimeStatus, "core" | "last_apply">): string | null {
  if (sub.last_result && "Failed" in sub.last_result) return sub.last_result.Failed.reason;
  // A successful reload applies the latest model, including this subscription.
  // Keep history intact; it describes the earlier attempt, not current health.
  if (runtime?.core === "Running" && runtime.last_apply && "Reloaded" in runtime.last_apply && !applicationProblem(runtime.last_apply)) return null;
  return applicationProblem(sub.info?.history[0]?.apply);
}

/** Includes disabled consumers: deleting a source must not break a saved rule. */
export function subscriptionUsages(model: Model, id: string): string[] {
  const node = (n: NodeRef) => "subscription_id" in n && n.subscription_id === id;
  const member = (m: ExitMember) => "group" in m ? "Subscription" in m.group && m.group.Subscription === id : node(m);
  const used = new Set<string>();
  for (const exit of model.exits) {
    if (exit.hidden) continue;
    const k = exit.kind;
    if (typeof k === "object" && ("Node" in k ? node(k.Node.node) : k.Group.members.some(member) || !!k.Group.selected && node(k.Group.selected))) {
      for (const l of model.listeners.filter(l => l.exit === exit.id)) used.add(l.name);
    }
  }
  model.process_routing.rules.filter(r => r.node && node(r.node)).forEach(r => used.add(ruleLabel(r)));
  model.node_groups.filter(g => g.members?.some(member)).forEach(g => used.add(`节点组「${g.name}」`));
  if (model.settings.node_monitor.members.some(member)) used.add("节点监测");
  return [...used];
}

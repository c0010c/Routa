import type { Exit, Listener, Model, ProcessRule } from "../bindings";
import { isSubRef, nodeKey } from "./refs";

/** 进程分流自动管理的隐藏监听器端口区间，与后端 HIDDEN_PORT_RANGE 保持一致。 */
export const HIDDEN_PORT_MIN = 37800;
export const HIDDEN_PORT_MAX = 37999;

/** 缺少 hidden 字段视为 false。 */
export function visibleListeners(model: Model): Listener[] {
  return model.listeners.filter((l) => !l.hidden);
}

export function visibleExits(model: Model): Exit[] {
  return model.exits.filter((e) => !e.hidden);
}

export function isHiddenExitId(model: Model, id: string): boolean {
  return model.exits.some((e) => e.id === id && !!e.hidden);
}

/** 规则实际走的隐藏出口（与后端 `rule_listener` 同一查找规则）；没有节点或尚未建出时返回 undefined。 */
export function ruleExit(model: Model, rule: ProcessRule): Exit | undefined {
  const node = rule.node;
  if (!node) return undefined;
  const manual = isSubRef(node) ? undefined : model.manual_nodes.find((n) => n.id === node.manual_id);
  const kind = manual?.kind;
  const iface =
    kind && typeof kind === "object" && "Tailscale" in kind
      ? (kind.Tailscale.interface ?? null)
      : (rule.interface ?? null);
  const key = nodeKey(node);
  return model.exits.find(
    (e) =>
      !!e.hidden &&
      typeof e.kind === "object" &&
      "Node" in e.kind &&
      nodeKey(e.kind.Node.node) === key &&
      (e.interface ?? null) === iface &&
      model.listeners.some((l) => !!l.hidden && l.exit === e.id),
  );
}

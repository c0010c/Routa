import type { ExitMember, Model, NodeRef } from "../bindings";

export const MANUAL_PREFIX = "手动";

export type SubNodeRef = Extract<NodeRef, { subscription_id: string }>;
export type ManualNodeRef = Extract<NodeRef, { manual_id: string }>;
export type GroupMember = Extract<ExitMember, { group: unknown }>;

export const isSubRef = (r: NodeRef): r is SubNodeRef => "subscription_id" in r;
export const isGroupMember = (m: ExitMember): m is GroupMember => "group" in m;
export const isNodeRef = (m: ExitMember): m is NodeRef => !isGroupMember(m);

export function nodeKey(r: NodeRef): string {
  return isSubRef(r) ? `s:${r.subscription_id}/${r.name}` : `m:${r.manual_id}`;
}

export function memberKey(m: ExitMember): string {
  if (isGroupMember(m)) {
    return "Subscription" in m.group ? `gs:${m.group.Subscription}` : `gc:${m.group.Custom}`;
  }
  return nodeKey(m);
}

export function nodeLabel(model: Model, r: NodeRef): string {
  if (isSubRef(r)) {
    const s = model.subscriptions.find((x) => x.id === r.subscription_id);
    return s?.nodes.some((n) => n.name === r.name) ? r.name : `${r.name}（已删除）`;
  }
  return model.manual_nodes.find((x) => x.id === r.manual_id)?.name ?? "（已删除）";
}

/** Key into the delay table: `<订阅>/<节点>` or `手动/<名>`. */
export function proxyNameOf(model: Model, r: NodeRef): string | null {
  if (isSubRef(r)) {
    const s = model.subscriptions.find((x) => x.id === r.subscription_id);
    return s ? `${s.name}/${r.name}` : null;
  }
  const n = model.manual_nodes.find((x) => x.id === r.manual_id);
  return n ? `${MANUAL_PREFIX}/${n.name}` : null;
}

function resolves(model: Model, r: NodeRef): boolean {
  if (isSubRef(r)) {
    return !!model.subscriptions.find((x) => x.id === r.subscription_id)?.nodes.some((n) => n.name === r.name);
  }
  return model.manual_nodes.some((x) => x.id === r.manual_id);
}

/** Same semantics as the Rust expand: keep order, dedupe by first occurrence, drop unresolvable. */
export function expandMembers(model: Model, members: ExitMember[]): NodeRef[] {
  const out: NodeRef[] = [];
  const seen = new Set<string>();
  const push = (r: NodeRef) => {
    const k = nodeKey(r);
    if (seen.has(k)) return;
    seen.add(k);
    out.push(r);
  };
  for (const m of members) {
    if (!isGroupMember(m)) {
      if (resolves(model, m)) push(m);
    } else if ("Subscription" in m.group) {
      const id = m.group.Subscription;
      const s = model.subscriptions.find((x) => x.id === id);
      for (const n of s?.nodes ?? []) push({ subscription_id: id, name: n.name });
    } else {
      const id = m.group.Custom;
      if (!model.node_groups.some((g) => g.id === id)) continue;
      for (const n of model.manual_nodes) if (n.groups.includes(id)) push({ manual_id: n.id });
    }
  }
  return out;
}

export function isTailscaleRef(model: Model, r: NodeRef): boolean {
  if (isSubRef(r)) return false;
  const n = model.manual_nodes.find((x) => x.id === r.manual_id);
  return !!n && typeof n.kind === "object" && "Tailscale" in n.kind;
}

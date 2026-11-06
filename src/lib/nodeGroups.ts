import type { Model, NodeRef } from "../bindings";
import { expandMembers, nodeKey } from "./refs";

/** Manual nodes outside every real custom group remain available as individual choices. */
export function ungroupedManualNodes(model: Model): NodeRef[] {
  const grouped = new Set(model.node_groups.flatMap((group) =>
    expandMembers(model, [{ group: { Custom: group.id } }]).map(nodeKey)));
  return model.manual_nodes.map((node) => ({ manual_id: node.id }))
    .filter((ref) => !grouped.has(nodeKey(ref)));
}

/** Match the node page: custom groups, remaining subscription nodes, then ungrouped manual nodes. */
export function candidateNodeGroups(model: Model, candidates: NodeRef[]) {
  const candidateKeys = new Set(candidates.map(nodeKey));
  const customGroups = model.node_groups.map((group) => ({
    id: `custom:${group.id}`,
    name: group.name,
    keys: new Set(expandMembers(model, [{ group: { Custom: group.id } }]).map(nodeKey)),
  }));
  const customKeys = new Set(customGroups.flatMap((group) => [...group.keys]));
  const groups = [
    ...customGroups,
    ...model.subscriptions.map((sub) => ({
      id: `subscription:${sub.id}`,
      name: sub.name,
      keys: new Set(sub.nodes.map((node) => nodeKey({ subscription_id: sub.id, name: node.name })).filter((key) => !customKeys.has(key))),
    })),
    {
      id: "ungrouped",
      name: "未分组",
      keys: new Set(ungroupedManualNodes(model).map(nodeKey)),
    },
  ];
  return groups.map((group) => ({ ...group, keys: new Set([...group.keys].filter((key) => candidateKeys.has(key))) }))
    .filter((group) => group.keys.size > 0);
}

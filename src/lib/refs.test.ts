import type { Model } from "../bindings";
import { expandMembers, isTailscaleRef, memberKey, nodeKey, nodeLabel, proxyNameOf } from "./refs";

const model = {
  subscriptions: [{ id: "s1", name: "机场A", nodes: [{ name: "n1" }, { name: "n2" }] }],
  exits: [],
  listeners: [],
  settings: {},
  manual_nodes: [
    { id: "m1", name: "a", groups: ["g1"], kind: { Proxy: { raw: {} } } },
    { id: "m2", name: "ts", groups: [], kind: { Tailscale: {} } },
  ],
  node_groups: [{ id: "g1", name: "家" }],
} as unknown as Model;

describe("refs", () => {
  it("expandMembers dedupes a manual node also in a custom group, keeping order", () => {
    const r = expandMembers(model, [{ manual_id: "m1" }, { group: { Custom: "g1" } }, { group: { Subscription: "s1" } }]);
    expect(r.map(nodeKey)).toEqual(["m:m1", "s:s1/n1", "s:s1/n2"]);
  });

  it("expandMembers drops unresolvable refs", () => {
    expect(
      expandMembers(model, [{ manual_id: "x" }, { subscription_id: "s1", name: "zz" }, { group: { Custom: "none" } }]),
    ).toEqual([]);
  });

  it("proxyNameOf", () => {
    expect(proxyNameOf(model, { manual_id: "m1" })).toBe("手动/a");
    expect(proxyNameOf(model, { subscription_id: "s1", name: "n1" })).toBe("机场A/n1");
    expect(proxyNameOf(model, { manual_id: "x" })).toBeNull();
  });

  it("keys do not collide and labels fall back", () => {
    expect(nodeKey({ manual_id: "m1" })).not.toBe(nodeKey({ subscription_id: "m1", name: "" }));
    expect(memberKey({ group: { Subscription: "s1" } })).not.toBe(memberKey({ group: { Custom: "s1" } }));
    expect(nodeLabel(model, { manual_id: "m1" })).toBe("a");
    expect(nodeLabel(model, { manual_id: "x" })).toBe("（已删除）");
  });

  it("isTailscaleRef", () => {
    expect(isTailscaleRef(model, { manual_id: "m2" })).toBe(true);
    expect(isTailscaleRef(model, { manual_id: "m1" })).toBe(false);
  });
});

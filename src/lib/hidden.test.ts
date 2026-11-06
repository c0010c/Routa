import { describe, expect, it } from "vitest";
import type { Exit, Listener, Model } from "../bindings";
import { HIDDEN_PORT_MAX, HIDDEN_PORT_MIN, isHiddenExitId, visibleExits, visibleListeners } from "./hidden";

const l = (id: string, hidden?: boolean): Listener => ({
  id, name: id, listener_type: "Socks", listen: "127.0.0.1", port: 1, auth: null, exit: null, user_enabled: true, hidden,
});
const e = (id: string, hidden?: boolean): Exit => ({ id, name: id, kind: "Direct", interface: null, hidden });
const model = {
  listeners: [l("user"), l("__proc_x", true), l("plain", false)],
  exits: [e("e1"), e("__proc_e", true)],
} as unknown as Model;

describe("hidden", () => {
  it("exposes the hidden port range", () => {
    expect([HIDDEN_PORT_MIN, HIDDEN_PORT_MAX]).toEqual([37800, 37999]);
  });
  it("visibleListeners drops hidden listeners", () => {
    expect(visibleListeners(model).map((x) => x.id)).toEqual(["user", "plain"]);
  });
  it("visibleExits drops hidden exits", () => {
    expect(visibleExits(model).map((x) => x.id)).toEqual(["e1"]);
  });
  it("isHiddenExitId", () => {
    expect(isHiddenExitId(model, "__proc_e")).toBe(true);
    expect(isHiddenExitId(model, "e1")).toBe(false);
    expect(isHiddenExitId(model, "nope")).toBe(false);
  });
});

import {
  applyOutcomeMessages,
  exitStatusLabel,
  formatBytes,
  formatRate,
  listenerNotEffective,
  listenerStatusLabel,
} from "./format";
import type { RuntimeStatus } from "../bindings";

describe("formatBytes", () => {
  it("formats zero", () => expect(formatBytes(0)).toBe("0 B"));
  it("formats bytes", () => expect(formatBytes(512)).toBe("512 B"));
  it("formats KB", () => expect(formatBytes(1536)).toBe("1.5 KB"));
  it("drops trailing .0", () => expect(formatBytes(1024)).toBe("1 KB"));
  it("formats MB", () => expect(formatBytes(5 * 1024 * 1024)).toBe("5 MB"));
  it("formats GB", () => expect(formatBytes(1.5 * 1024 ** 3)).toBe("1.5 GB"));
  it("formats rate", () => expect(formatRate(2048)).toBe("2 KB/s"));
});

describe("listenerStatusLabel", () => {
  it("Active", () =>
    expect(listenerStatusLabel("Active", false)).toEqual({ text: "运行中", color: "success" }));
  it("DisabledByUser", () =>
    expect(listenerStatusLabel("DisabledByUser", false)).toEqual({ text: "已停用", color: "default" }));
  it("NoExit", () =>
    expect(listenerStatusLabel("NoExit", false)).toEqual({ text: "未选择出口", color: "warning" }));
  it("NodeMissing", () =>
    expect(listenerStatusLabel({ ExitBroken: { exit_status: "NodeMissing" } }, false)).toEqual({
      text: "节点已失效，请重选",
      color: "error",
    }));
  it("AllMembersMissing", () =>
    expect(listenerStatusLabel({ ExitBroken: { exit_status: "AllMembersMissing" } }, false)).toEqual({
      text: "组内节点全部失效",
      color: "error",
    }));
  it("InterfaceUnavailable", () =>
    expect(listenerStatusLabel({ ExitBroken: { exit_status: "InterfaceUnavailable" } }, false)).toEqual({
      text: "网卡不可用",
      color: "error",
    }));
  it("port failure overrides Active", () =>
    expect(listenerStatusLabel("Active", true)).toEqual({ text: "端口被占用", color: "error" }));
  it("not-in-effect overrides Active and port failure, but not other states", () => {
    expect(listenerStatusLabel("Active", false, true)).toEqual({ text: "未生效", color: "warning" });
    expect(listenerStatusLabel("Active", true, true)).toEqual({ text: "未生效", color: "warning" });
    expect(listenerStatusLabel("DisabledByUser", false, true)).toEqual({ text: "已停用", color: "default" });
  });
});

describe("exitStatusLabel", () => {
  it("covers every status", () => {
    expect(exitStatusLabel("Ok")).toEqual({ text: "正常", color: "success" });
    expect(exitStatusLabel("NodeMissing")).toEqual({ text: "节点已失效", color: "error" });
    expect(exitStatusLabel("AllMembersMissing")).toEqual({ text: "组内节点全部失效", color: "error" });
    expect(exitStatusLabel("InterfaceUnavailable")).toEqual({ text: "网卡不可用", color: "error" });
  });
});

describe("applyOutcomeMessages", () => {
  it("is empty for a clean reload", () =>
    expect(applyOutcomeMessages({ Reloaded: { cut: 1, uncut: 0, failed_listeners: [] } })).toEqual([]));
  it("lists failed listeners one per line and uncut", () =>
    expect(
      applyOutcomeMessages({
        Reloaded: {
          cut: 0,
          uncut: 2,
          failed_listeners: [
            { id: "a", name: "办公", port: 7890 },
            { id: "b", name: "下载", port: 7891 },
          ],
        },
      }),
    ).toEqual([
      "端口 7890 已被其他程序占用，Listener「办公」未启动",
      "端口 7891 已被其他程序占用，Listener「下载」未启动",
      "部分旧连接未能断开，正在重启 mihomo",
    ]));
  it("failure variants", () => {
    expect(applyOutcomeMessages({ FailedKeptPrevious: { error: "x" } })).toEqual(["已保存，但未生效：x"]);
    expect(applyOutcomeMessages({ NotRunning: { error: "x" } })).toEqual(["已保存，但未生效：x"]);
    expect(applyOutcomeMessages({ FailedClosed: { error: "x", uncut: 1 } })).toEqual([
      "已保存，但未能生效，为防止泄露已关闭所有端口：x",
      "部分旧连接未能断开，正在重启 mihomo",
    ]);
    expect(applyOutcomeMessages({ FailedCoreStopped: { error: "x" } })).toEqual([
      "已保存，但未能生效，mihomo 已停止：x",
    ]);
  });
});

describe("listenerNotEffective", () => {
  const base: RuntimeStatus = {
    core: "Running",
    last_apply: null,
    health: { exits: {}, listeners: {} },
    delays: {},
    traffic: null,
    interfaces: [],
    failed_listeners: [],
    effective_listeners: ["l1"],
    startup_error: null,
    save_error: null,
  };
  it("is false for a listener in the config a running core holds", () => {
    expect(listenerNotEffective(base, "l1")).toBe(false);
    expect(listenerNotEffective({ ...base, last_apply: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } }, "l1")).toBe(false);
    expect(listenerNotEffective({ ...base, last_apply: { FailedKeptPrevious: { error: "x" } } }, "l1")).toBe(false);
  });
  it("is true when the core is not running or the last apply closed everything", () => {
    expect(listenerNotEffective({ ...base, core: "Stopped" }, "l1")).toBe(true);
    expect(listenerNotEffective({ ...base, core: "Starting" }, "l1")).toBe(true);
    expect(listenerNotEffective({ ...base, core: { Crashed: { exit_code: null } } }, "l1")).toBe(true);
    expect(listenerNotEffective({ ...base, last_apply: { FailedClosed: { error: "x", uncut: 0 } } }, "l1")).toBe(true);
    expect(listenerNotEffective({ ...base, last_apply: { FailedCoreStopped: { error: "x" } } }, "l1")).toBe(true);
  });
  it("is true when mihomo's current config lacks the listener", () => {
    // Retry from the fail-closed empty config rejected again.
    expect(
      listenerNotEffective({ ...base, last_apply: { FailedKeptPrevious: { error: "x" } }, effective_listeners: [] }, "l1"),
    ).toBe(true);
    // A newly added listener whose apply was rejected.
    expect(listenerNotEffective({ ...base, last_apply: { FailedKeptPrevious: { error: "x" } } }, "l2")).toBe(true);
  });
});

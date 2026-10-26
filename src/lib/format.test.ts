import {
  applyOutcomeMessages,
  exitStatusLabel,
  formatBytes,
  formatRate,
  listenerStatusLabel,
} from "./format";

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
    expect(applyOutcomeMessages({ FailedKeptPrevious: { error: "x" } })).toEqual([
      "保存失败，已保留之前的配置：x",
    ]);
    expect(applyOutcomeMessages({ FailedClosed: { error: "x", uncut: 1 } })).toEqual([
      "保存失败，为防止泄露已关闭所有端口：x",
      "部分旧连接未能断开，正在重启 mihomo",
    ]);
    expect(applyOutcomeMessages({ FailedCoreStopped: { error: "x" } })).toEqual([
      "保存失败，mihomo 已停止：x",
    ]);
  });
});

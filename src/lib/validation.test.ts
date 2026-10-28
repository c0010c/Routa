import { fieldErrors } from "./validation";

describe("fieldErrors", () => {
  it("places port conflicts on the port field, including the controller port", () => {
    expect(fieldErrors([{ PortConflict: { port: 7891, with: "浏览器" } }]).port).toBe(
      "端口 7891 已被「浏览器」占用",
    );
    expect(fieldErrors([{ PortConflict: { port: 19097, with: "控制端口" } }]).port).toBe(
      "端口 19097 已被「控制端口」占用",
    );
  });

  it("places name errors on the name field", () => {
    expect(fieldErrors([{ DuplicateName: { kind: "listener", name: "a" } }]).name).toContain("a");
    expect(fieldErrors([{ InvalidName: { name: "", reason: "名称不能为空" } }]).name).toBe("名称不能为空");
  });

  it("puts dangling refs and interval errors at form level", () => {
    const r = fieldErrors([
      { DanglingRef: { what: "Listener「办公」" } },
      { IntervalOutOfRange: { field: "测速间隔", value: 0 } },
    ]);
    expect(r.name).toBeUndefined();
    expect(r.port).toBeUndefined();
    expect(r.form).toHaveLength(2);
    expect(r.form[0]).toContain("办公");
    expect(r.form[1]).toContain("测速间隔");
  });

  it("shows a config rejected by mihomo at form level", () => {
    const r = fieldErrors([{ CoreRejected: { message: "proxy 0: '' has unset fields: cipher" } }]);
    expect(r.name).toBeUndefined();
    expect(r.form).toEqual(["mihomo 拒绝了该配置：proxy 0: '' has unset fields: cipher"]);
  });

  it("puts process rule errors at form level", () => {
    const r = fieldErrors([
      { DuplicateProcessRule: { exe_path: "C:\a.exe" } },
      { InvalidExePath: { exe_path: "a.exe", reason: "必须是绝对路径" } },
      { ListenerTypeNotAllowed: { listener: "办公" } },
      { TooManyProcessRules: { max: 32 } },
    ]);
    expect(r.name).toBeUndefined();
    expect(r.form).toEqual([
      "程序「C:\a.exe」已有分流规则",
      "程序路径「a.exe」无效：必须是绝对路径",
      "Listener「办公」是 HTTP 类型，不能用于进程分流",
      "启用的分流规则最多 32 条",
    ]);
  });

  it("puts interface mismatch and invalid proxy at form level", () => {
    expect(fieldErrors([{ InterfaceMismatch: { exit: "E", node: "ts" } }]).form).toEqual([
      "出口「E」与 Tailscale「ts」的网卡不一致",
    ]);
    expect(fieldErrors([{ InvalidProxy: { name: "a", reason: "缺少 type" } }]).form).toEqual(["节点「a」：缺少 type"]);
  });

  it("returns an empty form list when there are no errors", () => {
    expect(fieldErrors([])).toEqual({ form: [] });
  });
});

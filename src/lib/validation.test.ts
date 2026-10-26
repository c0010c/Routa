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

  it("returns an empty form list when there are no errors", () => {
    expect(fieldErrors([])).toEqual({ form: [] });
  });
});

import type { ValidationError } from "../bindings";

export interface FieldErrors {
  name?: string;
  port?: string;
  form: string[];
}

/** Maps backend validation errors onto form fields; anything unplaceable goes to `form`. */
export function fieldErrors(errors: ValidationError[]): FieldErrors {
  const out: FieldErrors = { form: [] };
  for (const e of errors) {
    if ("PortConflict" in e) {
      out.port ??= `端口 ${e.PortConflict.port} 已被「${e.PortConflict.with}」占用`;
    } else if ("DuplicateName" in e) {
      out.name ??= `名称「${e.DuplicateName.name}」已存在`;
    } else if ("InvalidName" in e) {
      out.name ??= e.InvalidName.reason;
    } else if ("DanglingRef" in e) {
      out.form.push(`仍被引用：${e.DanglingRef.what}`);
    } else if ("CoreRejected" in e) {
      out.form.push(`mihomo 拒绝了该配置：${e.CoreRejected.message}`);
    } else if ("InterfaceMismatch" in e) {
      out.form.push(`出口「${e.InterfaceMismatch.exit}」与 Tailscale「${e.InterfaceMismatch.node}」的网卡不一致`);
    } else if ("InvalidProxy" in e) {
      out.form.push(`节点「${e.InvalidProxy.name}」：${e.InvalidProxy.reason}`);
    } else if ("DuplicateProcessRule" in e) {
      out.form.push(`${e.DuplicateProcessRule.exe_path} 已在其他规则中，一个程序只能属于一条规则`);
    } else if ("InvalidExePath" in e) {
      out.form.push(`「${e.InvalidExePath.exe_path}」无效：${e.InvalidExePath.reason}`);
    } else if ("EmptyProcessRule" in e) {
      out.form.push("至少添加一个进程");
    } else if ("ProcessNameCannotBlock" in e) {
      out.form.push("按进程名匹配时，组件故障时只能直连；需要阻断请在高级选项里打开「按完整路径匹配」");
    } else if ("TooManyProcessRules" in e) {
      out.form.push(`启用的规则里最多一共 ${e.TooManyProcessRules.max} 个进程`);
    } else {
      out.form.push(`${e.IntervalOutOfRange.field} 的值 ${e.IntervalOutOfRange.value} 超出允许范围`);
    }
  }
  return out;
}

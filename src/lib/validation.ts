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
      out.form.push(`程序「${e.DuplicateProcessRule.exe_path}」已有分流规则`);
    } else if ("InvalidExePath" in e) {
      out.form.push(`程序路径「${e.InvalidExePath.exe_path}」无效：${e.InvalidExePath.reason}`);
    } else if ("ListenerTypeNotAllowed" in e) {
      out.form.push(`Listener「${e.ListenerTypeNotAllowed.listener}」是 HTTP 类型，不能用于进程分流`);
    } else if ("ListenerAddressNotAllowed" in e) {
      const { listener, listen } = e.ListenerAddressNotAllowed;
      out.form.push(`Listener「${listener}」的监听地址「${listen}」不能用于进程分流：仅支持 IPv4 入口，请修改监听地址或选择其他 Listener`);
    } else if ("TooManyProcessRules" in e) {
      out.form.push(`启用的分流规则最多 ${e.TooManyProcessRules.max} 条`);
    } else {
      out.form.push(`${e.IntervalOutOfRange.field} 的值 ${e.IntervalOutOfRange.value} 超出允许范围`);
    }
  }
  return out;
}

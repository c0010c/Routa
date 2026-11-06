import type { CoreState, Model, RuntimeStatus } from "../bindings";
import { applyOutcomeMessages, exitStatusLabel } from "./format";

function coreText(c: CoreState): string {
  if (c === "Stopped") return "已停止";
  if (c === "Starting") return "启动中";
  if (c === "Running") return "运行中";
  return `已崩溃（退出码 ${c.Crashed.exit_code ?? "未知"}）`;
}

export function collectProblems(model: Model, status: RuntimeStatus): string[] {
  const out: string[] = [];
  if (status.core === "Stopped" || typeof status.core === "object") out.push(`mihomo ${coreText(status.core)}`);
  const a = status.last_apply;
  if (a !== null && !("Reloaded" in a)) out.push(...applyOutcomeMessages(a));
  for (const [id, h] of Object.entries(status.health.exits)) {
    if (!h || h.status === "Ok") continue;
    const name = model.exits.find((e) => e.id === id)?.name ?? id;
    out.push(`出口「${name}」${exitStatusLabel(h.status).text}`);
  }
  for (const f of status.failed_listeners) {
    out.push(`监听器「${f.name}」端口 ${f.port} 被占用`);
  }
  for (const s of model.subscriptions) {
    if (s.last_result && "Failed" in s.last_result) {
      out.push(`订阅「${s.name}」更新失败：${s.last_result.Failed.reason}`);
    }
  }
  if (status.startup_error) out.push(`启动时发现问题：${status.startup_error}`);
  if (status.save_error) out.push(`保存状态文件失败：${status.save_error}`);
  return out;
}

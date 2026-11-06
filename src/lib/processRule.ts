import type { ProcessRule } from "../bindings";

/** 进程（进程名或完整路径）的文件名部分。 */
export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

/** 规则的显示名：名称为空时用进程名，多个进程时写成「a.exe 等 3 个」。与后端 `process_rule_label` 的首项一致。 */
export function ruleLabel(rule: Pick<ProcessRule, "name" | "processes">): string {
  if (rule.name.trim()) return rule.name.trim();
  const [first] = rule.processes;
  if (!first) return "未命名规则";
  return rule.processes.length === 1 ? fileName(first) : `${fileName(first)} 等 ${rule.processes.length} 个`;
}

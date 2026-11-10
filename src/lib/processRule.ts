import type { ProcessEntry, ProcessKind, ProcessRule } from "../bindings";

/** 进程（进程名或完整路径）的文件名部分。 */
export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

/** 由「AI 应用」页管理的规则：进程分流页与简洁版不显示、不可编辑。 */
export const isAiRule = (r: ProcessRule) => r.ai_app != null;

const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** 输入框里的原始文本 -> 条目值：去引号、`/` 换成 `\`、去首尾空白。 */
export function cleanEntry(raw: string): string {
  return raw.trim().replace(/^"|"$/g, "").replace(/\//g, "\\").trim();
}

/** 看起来像路径就按路径，否则按进程名。 */
export function defaultKind(raw: string): ProcessKind {
  return /[\\/:]/.test(raw) ? "Path" : "Name";
}

/** 一个程序（按文件名）在规则里的所有条目。 */
export interface ProcessGroup {
  key: string;
  name: string;
  hasName: boolean;
  paths: string[];
}

export function groupsOf(entries: ProcessEntry[]): ProcessGroup[] {
  const out: ProcessGroup[] = [];
  for (const e of entries) {
    const name = fileName(e.value);
    const key = name.toLowerCase();
    let g = out.find((x) => x.key === key);
    if (!g) out.push((g = { key, name, hasName: false, paths: [] }));
    if (e.kind === "Name") g.hasName = true;
    else g.paths.push(e.value);
  }
  return out;
}

export function chipLabel(g: ProcessGroup): string {
  if (g.paths.length === 0) return g.name;
  if (g.hasName) return `${g.name} + ${g.paths.length} 个位置`;
  return g.paths.length > 1 ? `${g.name} · ${g.paths.length} 个位置` : g.name;
}

/** 规则的显示名：名称为空时按程序分组，多个程序时写成「a.exe 等 3 个」。 */
export function ruleLabel(rule: Pick<ProcessRule, "name" | "processes">): string {
  if (rule.name.trim()) return rule.name.trim();
  const gs = groupsOf(rule.processes);
  if (gs.length === 0) return "未命名规则";
  return gs.length === 1 ? gs[0].name : `${gs[0].name} 等 ${gs.length} 个`;
}

/** 冲突：进程名和进程名、同一个路径和同一个路径。进程名和路径不冲突（路径优先）。 */
const overlaps = (a: ProcessEntry, b: ProcessEntry) =>
  a.kind === b.kind && (a.kind === "Path" ? same(a.value, b.value) : same(fileName(a.value), fileName(b.value)));

/** 校验一个条目。`entries` 是本规则全部条目（可含 `e` 自身，按引用排除）；`others` 是其他规则（含停用）。 */
export function entryError(e: ProcessEntry, entries: ProcessEntry[], others: ProcessRule[]): string | undefined {
  const v = e.value.trim();
  // 只收本地盘符路径：UNC（\\server\share）与 \\?\ 之类的设备路径服务端也会拒收。
  if (e.kind === "Path" && !/^[a-zA-Z]:\\/.test(v)) return "不是完整路径（应以 C:\\ 之类的盘符开头）";
  if (!/\.exe$/i.test(v)) return `${fileName(v)} 不是 .exe 程序`;
  if (entries.some((x) => x !== e && overlaps(x, e))) return "已经在这条规则里";
  const owner = others.find((r) => r.processes.some((x) => overlaps(x, e)));
  if (owner)
    return e.kind === "Name"
      ? `已在规则「${ruleLabel(owner)}」中按进程名匹配；要单独分流某个位置，请勾选它的路径`
      : `这个位置已在规则「${ruleLabel(owner)}」中`;
  return undefined;
}

const isActive = (r: ProcessRule, routingEnabled: boolean) => routingEnabled && r.user_enabled;

/** 其他启用规则里，用路径单独接管了这个程序某些位置的规则（路径优先）。 */
export function pathOwners(
  name: string,
  others: ProcessRule[],
  routingEnabled: boolean,
): { rule: ProcessRule; path: string }[] {
  return others
    .filter((r) => isActive(r, routingEnabled))
    .flatMap((r) =>
      r.processes
        .filter((x) => x.kind === "Path" && same(fileName(x.value), name))
        .map((x) => ({ rule: r, path: x.value })),
    );
}

/** 按进程名阻断时会自动记录位置的程序名。 */
export function learningNames(rule: Pick<ProcessRule, "on_fault" | "processes">): string[] {
  return rule.on_fault === "Block" ? groupsOf(rule.processes).filter((g) => g.hasName).map((g) => g.name) : [];
}

/** 不会记录的位置：属于其他启用规则的路径。 */
export function skippedOf(rule: ProcessRule, others: ProcessRule[], routingEnabled: boolean) {
  return learningNames(rule).flatMap((n) => pathOwners(n, others, routingEnabled));
}

/** 保存后新增的"会记录位置"的程序名（用于保存前提示）。 */
export function newlyLearning(before: ProcessRule | undefined, after: ProcessRule): string[] {
  const prev = before ? learningNames(before) : [];
  return learningNames(after).filter((n) => !prev.some((x) => same(x, n)));
}

/** 启用且故障时阻断的规则。 */
export function blockingRules(rules: ProcessRule[], routingEnabled: boolean): ProcessRule[] {
  return rules.filter((r) => isActive(r, routingEnabled) && r.on_fault === "Block");
}

import { describe, expect, it } from "vitest";
import type { ProcessEntry, ProcessRule } from "../bindings";
import {
  blockingRules,
  chipLabel,
  cleanEntry,
  defaultKind,
  entryError,
  groupsOf,
  learningNames,
  newlyLearning,
  pathOwners,
  ruleLabel,
  skippedOf,
} from "./processRule";

const n = (value: string): ProcessEntry => ({ kind: "Name", value });
const p = (value: string): ProcessEntry => ({ kind: "Path", value });
const rule = (over: Partial<ProcessRule>): ProcessRule => ({
  id: "r" as ProcessRule["id"],
  name: "",
  processes: [],
  node: null,
  interface: null,
  on_fault: "Direct",
  reset_on_change: false,
  learned: [],
  user_enabled: true,
  ai_app: null,
  ...over,
});

describe("cleanEntry / defaultKind", () => {
  it("strips quotes, flips slashes, trims", () => {
    expect(cleanEntry('  "C:/A/b.exe" ')).toBe("C:\\A\\b.exe");
  });
  it("path-like input defaults to Path", () => {
    expect(defaultKind("C:\\A\\b.exe")).toBe("Path");
    expect(defaultKind("b.exe")).toBe("Name");
  });
});

describe("groupsOf / chipLabel", () => {
  it("merges a name with its paths", () => {
    const gs = groupsOf([n("a.exe"), p("C:\\dx\\A.exe"), p("D:\\dy\\a.exe"), p("C:\\dz\\b.exe")]);
    expect(gs).toHaveLength(2);
    expect(gs[0]).toMatchObject({ name: "a.exe", hasName: true });
    expect(gs[0].paths).toEqual(["C:\\dx\\A.exe", "D:\\dy\\a.exe"]);
    expect(gs[1]).toMatchObject({ name: "b.exe", hasName: false });
  });
  it("labels the three shapes", () => {
    expect(chipLabel({ key: "a.exe", name: "a.exe", hasName: true, paths: [] })).toBe("a.exe");
    expect(chipLabel({ key: "a.exe", name: "a.exe", hasName: true, paths: ["x", "y"] })).toBe("a.exe + 2 个位置");
    expect(chipLabel({ key: "a.exe", name: "a.exe", hasName: false, paths: ["x", "y"] })).toBe("a.exe · 2 个位置");
    expect(chipLabel({ key: "a.exe", name: "a.exe", hasName: false, paths: ["x"] })).toBe("a.exe");
  });
});

describe("ruleLabel", () => {
  it("uses name, then grouped program names", () => {
    expect(ruleLabel({ name: " X ", processes: [] })).toBe("X");
    expect(ruleLabel({ name: "", processes: [] })).toBe("未命名规则");
    expect(ruleLabel({ name: "", processes: [n("a.exe"), p("C:\\q\\a.exe")] })).toBe("a.exe");
    expect(ruleLabel({ name: "", processes: [n("a.exe"), n("b.exe"), p("C:\\q\\a.exe")] })).toBe("a.exe 等 2 个");
  });
});

describe("entryError", () => {
  const other = rule({ name: "Other", processes: [n("a.exe"), p("C:\\g\\g.exe")], user_enabled: false });
  it("rejects incomplete paths", () => {
    expect(entryError(p("a.exe"), [], [])).toBe("不是完整路径（应以 C:\\ 之类的盘符开头）");
    for (const unc of ["\\\\srv\\share\\a.exe", "\\\\?\\C:\\a\\a.exe"])
      expect(entryError(p(unc), [], [])).toBe("不是完整路径（应以 C:\\ 之类的盘符开头）");
  });
  it("rejects non-exe", () => {
    expect(entryError(n("a.dll"), [], [])).toBe("a.dll 不是 .exe 程序");
  });
  it("rejects duplicates within the rule", () => {
    const e = n("A.exe");
    expect(entryError(e, [n("a.exe"), e], [])).toBe("已经在这条规则里");
    expect(entryError(e, [e], [])).toBeUndefined();
  });
  it("rejects name taken by another rule, including disabled", () => {
    expect(entryError(n("a.exe"), [], [other])).toBe(
      "已在规则「Other」中按进程名匹配；要单独分流某个位置，请勾选它的路径",
    );
  });
  it("rejects path taken by another rule", () => {
    expect(entryError(p("C:\\g\\g.exe"), [], [other])).toBe("这个位置已在规则「Other」中");
  });
  it("name does not conflict with another rule's path", () => {
    expect(entryError(n("g.exe"), [], [other])).toBeUndefined();
    expect(entryError(p("C:\\a\\a.exe"), [], [other])).toBeUndefined();
  });
});

describe("pathOwners / learningNames / skippedOf", () => {
  const on = rule({ name: "On", processes: [p("C:\\g\\a.exe")] });
  const off = rule({ name: "Off", processes: [p("D:\\h\\a.exe")], user_enabled: false });
  it("ignores disabled rules", () => {
    expect(pathOwners("A.exe", [on, off], true)).toEqual([{ rule: on, path: "C:\\g\\a.exe" }]);
  });
  it("ignores everything when routing is off", () => {
    expect(pathOwners("a.exe", [on], false)).toEqual([]);
  });
  it("learns only Block rules' names", () => {
    expect(learningNames(rule({ on_fault: "Direct", processes: [n("a.exe")] }))).toEqual([]);
    expect(learningNames(rule({ on_fault: "Block", processes: [n("a.exe"), p("C:\\dz\\b.exe")] }))).toEqual(["a.exe"]);
  });
  it("skips locations owned by other rules", () => {
    const r = rule({ on_fault: "Block", processes: [n("a.exe")] });
    expect(skippedOf(r, [on, off], true)).toEqual([{ rule: on, path: "C:\\g\\a.exe" }]);
  });
});

describe("newlyLearning", () => {
  const direct = rule({ processes: [n("a.exe")] });
  const block = rule({ on_fault: "Block", processes: [n("a.exe")] });
  it("Direct to Block returns the name", () => {
    expect(newlyLearning(direct, block)).toEqual(["a.exe"]);
    expect(newlyLearning(undefined, block)).toEqual(["a.exe"]);
  });
  it("already Block, node change only", () => {
    expect(newlyLearning(block, { ...block, interface: "g" as ProcessRule["interface"] })).toEqual([]);
  });
  it("adding a Name returns it", () => {
    expect(newlyLearning(block, { ...block, processes: [n("a.exe"), n("b.exe")] })).toEqual(["b.exe"]);
  });
});

describe("blockingRules", () => {
  it("only enabled Block rules", () => {
    const a = rule({ on_fault: "Block" });
    const b = rule({ on_fault: "Block", user_enabled: false });
    const c = rule({ on_fault: "Direct" });
    expect(blockingRules([a, b, c], true)).toEqual([a]);
    expect(blockingRules([a, b, c], false)).toEqual([]);
  });
});

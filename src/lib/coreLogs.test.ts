import { filterCoreLogs, logTime, parseCoreLogs } from "./coreLogs";

it("parses the actual logrus format and decodes multiline errors without corrupting paths", () => {
  const raw = String.raw`time="2026-10-30T20:13:53.123456700+08:00" level=warning msg="[TCP] dial 机场 127.0.0.1:4001 --> example.com:443 error: i/o timeout\nretry failed\npath C:\\Apps\\core.exe says \"no\""`;
  const [entry] = parseCoreLogs(raw);
  expect(entry.timestamp).toBe("2026-10-30T20:13:53.123456700+08:00");
  expect(entry.level).toBe("warning");
  expect(entry.message).toBe('[TCP] dial 机场 127.0.0.1:4001 --> example.com:443 error: i/o timeout\nretry failed\npath C:\\Apps\\core.exe says "no"');
  expect(entry.raw).toBe(raw);
  expect(logTime(entry.timestamp)).toMatch(/^10-30 \d{2}:13:53$/);
});

it("accepts JSON, Go escapes, ANSI colors, plain messages, and unknown levels", () => {
  const entries = parseCoreLogs([
    JSON.stringify({ time: "2026-10-30T20:00:00+08:00", level: "error", msg: "JSON错误\n详情" }),
    '\x1b[32m[INFO] mihomo 已启动\x1b[0m',
    String.raw`level=debug msg="quote=\"ok\" tab=\t hex=\x41 unicode=\u4e2d wide=\U0001F600"`,
    'time="t" level=new msg="未来级别" field="extra"',
    "未识别的原始输出",
  ].join("\n"));
  expect(entries.map((entry) => entry.level)).toEqual(["error", "info", "debug", "unknown", "unknown"]);
  expect(entries[0].message).toBe("JSON错误\n详情");
  expect(entries[1].message).toBe("mihomo 已启动");
  expect(entries[2].message).toBe('quote="ok" tab=\t hex=A unicode=中 wide=😀');
  expect(entries[3].message).toContain("field=extra");
  expect(entries[4].message).toBe("未识别的原始输出");
});

it("keeps raw malformed records and attaches indented continuations without losing content", () => {
  const entries = parseCoreLogs('[INFO] 启动\n  后续详情\ntime="broken" level=error msg="未结束\n普通输出');
  expect(entries).toHaveLength(3);
  expect(entries[0].message).toBe("启动\n  后续详情");
  expect(entries[1].message).toContain('msg="未结束');
  expect(entries[1].level).toBe("unknown");
  expect(parseCoreLogs("")).toEqual([]);
  expect(parseCoreLogs("暂无内核日志")).toEqual([]);
});

it("has stable occurrence IDs and searches details as well as message summaries", () => {
  const text = '[INFO] old\n[WARN] warning\n[ERROR] failure\n[INFO] old';
  const before = parseCoreLogs(text);
  expect(new Set(before.map((entry) => entry.id)).size).toBe(4);
  expect(parseCoreLogs(text + '\n[INFO] new').slice(0, 4).map((entry) => entry.id)).toEqual(before.map((entry) => entry.id));
  expect(filterCoreLogs(before, "problems", "").map((entry) => entry.message)).toEqual(["warning", "failure"]);
  expect(filterCoreLogs(before, "error", "FAILURE")).toHaveLength(1);
  expect(filterCoreLogs(before, "warning", "failure")).toHaveLength(0);
  expect(logTime("")).toBe("时间未知");
  expect(logTime("invalid")).toBe("invalid");
});

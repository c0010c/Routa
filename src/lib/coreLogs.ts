export type LogLevel = "error" | "warning" | "info" | "debug" | "unknown";
export type LogFilter = "all" | "problems" | LogLevel;

export interface CoreLogEntry {
  id: string;
  timestamp: string;
  level: LogLevel;
  message: string;
  raw: string;
}

export const LOG_LEVEL_LABELS: Record<LogLevel, string> = {
  error: "错误", warning: "警告", info: "信息", debug: "调试", unknown: "未知",
};

function levelOf(value: string): LogLevel {
  if (/^(error|fatal|panic)$/i.test(value)) return "error";
  if (/^warn(ing)?$/i.test(value)) return "warning";
  if (/^info$/i.test(value)) return "info";
  if (/^(debug|trace)$/i.test(value)) return "debug";
  return "unknown";
}

/** Logrus uses Go quoted strings, including escapes that JSON.parse doesn't accept. */
function unquote(value: string): string {
  if (!value.startsWith('"') || !value.endsWith('"')) return value;
  const escapes: Record<string, string> = {
    a: "\x07", b: "\b", f: "\f", n: "\n", r: "\r", t: "\t", v: "\v", '"': '"', "\\": "\\",
  };
  return value.slice(1, -1).replace(
    /\\(?:x([\da-fA-F]{2})|u([\da-fA-F]{4})|U([\da-fA-F]{8})|([0-7]{3})|([\s\S]))/g,
    (original, hex: string, unicode: string, wide: string, octal: string, char: string) => {
      if (char !== undefined) return escapes[char] ?? original;
      const point = parseInt(hex ?? unicode ?? wide ?? octal, octal ? 8 : 16);
      return point <= 0x10ffff ? String.fromCodePoint(point) : original;
    },
  );
}

function stripAnsi(text: string) {
  return text.replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "");
}

function parseLine(raw: string): Omit<CoreLogEntry, "id"> {
  const clean = stripAnsi(raw);
  try {
    const json: unknown = JSON.parse(clean);
    if (json && typeof json === "object") {
      const fields = json as Record<string, unknown>;
      const message = fields.msg ?? fields.message ?? fields.payload;
      if (typeof message === "string") return {
        timestamp: String(fields.time ?? fields.timestamp ?? ""),
        level: levelOf(String(fields.level ?? fields.type ?? "")),
        message: stripAnsi(message), raw,
      };
    }
  } catch { /* Non-JSON logs are expected. */ }

  if (/^(?:time|level|msg)=/.test(clean)) {
    const fields = new Map([...clean.matchAll(/([\w.-]+)=("(?:\\.|[^"\\])*"|[^"\s]+)/g)]
      .map((match) => [match[1], unquote(match[2])]));
    if (fields.has("msg")) {
      const extra = [...fields].filter(([key]) => !["time", "level", "msg"].includes(key))
        .map(([key, value]) => `${key}=${value}`);
      return {
        timestamp: fields.get("time") ?? "", level: levelOf(fields.get("level") ?? ""),
        message: stripAnsi([fields.get("msg"), ...extra].join("\n")), raw,
      };
    }
  }
  const bracket = clean.match(/^(?:(\d{4}[-/]\d{2}[-/]\d{2}[T ][\d:.]+(?:Z|[+-]\d{2}:\d{2})?)\s+)?\[(TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL|PANIC)\]\s*(.*)$/i);
  if (bracket) return { timestamp: bracket[1] ?? "", level: levelOf(bracket[2]), message: bracket[3], raw };
  return { timestamp: "", level: "unknown", message: clean, raw };
}

/** File order is authoritative; don't invent timestamps for unstructured messages. */
export function parseCoreLogs(text: string): CoreLogEntry[] {
  if (!text.trim() || text.trim() === "暂无内核日志") return [];
  const entries: CoreLogEntry[] = [];
  const occurrences = new Map<string, number>();
  for (const raw of text.split(/\r?\n/)) {
    if (!raw.trim()) continue;
    const parsed = parseLine(raw);
    const previous = entries[entries.length - 1];
    if (parsed.level === "unknown" && /^\s+/.test(raw) && previous) {
      previous.message += `\n${parsed.message}`;
      previous.raw += `\n${raw}`;
      continue;
    }
    const count = occurrences.get(raw) ?? 0;
    occurrences.set(raw, count + 1);
    entries.push({ ...parsed, id: `${raw}\0${count}` });
  }
  return entries;
}

export function filterCoreLogs(entries: CoreLogEntry[], filter: LogFilter, query: string) {
  const search = query.trim().toLocaleLowerCase();
  return entries.filter((entry) =>
    (filter === "all" || (filter === "problems" ? ["warning", "error"].includes(entry.level) : entry.level === filter)) &&
    (!search || `${entry.message}\n${entry.timestamp}\n${entry.raw}`.toLocaleLowerCase().includes(search)),
  );
}

export function logTime(timestamp: string) {
  if (!timestamp) return "时间未知";
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return timestamp;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

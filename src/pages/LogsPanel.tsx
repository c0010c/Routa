import { useId, useMemo, useState } from "react";
import ChevronRight from "@mui/icons-material/ChevronRight";
import PauseOutlined from "@mui/icons-material/PauseOutlined";
import PlayArrowOutlined from "@mui/icons-material/PlayArrowOutlined";
import RefreshOutlined from "@mui/icons-material/RefreshOutlined";
import {
  Alert, Box, Button, ButtonBase, MenuItem, Paper, Stack, TextField, Typography,
} from "@mui/material";
import { SearchField } from "../components/SearchField";
import {
  filterCoreLogs, LOG_LEVEL_LABELS, logTime, parseCoreLogs,
  type CoreLogEntry, type LogFilter, type LogLevel,
} from "../lib/coreLogs";

const LEVEL_COLORS: Record<LogLevel, string> = {
  error: "error.main", warning: "warning.main", info: "info.main",
  debug: "text.secondary", unknown: "text.secondary",
};
const ROW_GRID = "124px 52px minmax(0, 1fr) 18px";

function LogRow({ entry, onInspect }: { entry: CoreLogEntry; onInspect: () => void }) {
  const [open, setOpen] = useState(false);
  const id = useId();
  const summary = entry.message.split("\n").find((line) => line.trim()) || "（空消息）";
  return (
    <Box component="li" sx={{ borderBottom: 1, borderColor: "divider" }}>
      <ButtonBase
        aria-expanded={open}
        aria-controls={id}
        aria-label={`${open ? "收起" : "展开"}日志：${summary.slice(0, 80)}`}
        onClick={() => { if (!open) onInspect(); setOpen(!open); }}
        sx={{
          display: "grid", gridTemplateColumns: ROW_GRID, gap: 1.25,
          width: "100%", px: 2, py: 1.25, minHeight: 58, alignItems: "start", textAlign: "left",
          "&:hover": { bgcolor: "action.hover" },
          "&.Mui-focusVisible": { outline: "2px solid", outlineColor: "primary.main", outlineOffset: -2 },
        }}
      >
        <Typography component="span" variant="caption" color="text.secondary" title={entry.timestamp || "日志未提供时间"}
          sx={{ pt: 0.25, fontVariantNumeric: "tabular-nums", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          {logTime(entry.timestamp)}
        </Typography>
        <Box component="span" sx={{
          color: LEVEL_COLORS[entry.level], fontSize: 11, lineHeight: "20px", fontWeight: 600,
          border: 1, borderColor: "currentColor", borderRadius: 0.75, textAlign: "center",
        }}>{LOG_LEVEL_LABELS[entry.level]}</Box>
        <Typography component="span" sx={{
          fontSize: 13, lineHeight: 1.65, overflowWrap: "anywhere", display: "-webkit-box",
          WebkitLineClamp: 2, WebkitBoxOrient: "vertical", overflow: "hidden",
        }}>{summary}</Typography>
        <ChevronRight sx={{
          fontSize: 18, mt: 0.25, color: "text.secondary", transform: open ? "rotate(90deg)" : "none",
          transition: "transform 140ms ease", "@media (prefers-reduced-motion: reduce)": { transition: "none" },
        }} />
      </ButtonBase>
      {open && <Box id={id} role="region" aria-label="日志详情" sx={{ mx: 2, mb: 1.5, p: 1.5, bgcolor: "action.hover", borderRadius: 1 }}>
        <Box component="pre" sx={{ m: 0, fontSize: 12, lineHeight: 1.8, fontFamily: "Consolas, monospace", whiteSpace: "pre-wrap", overflowWrap: "anywhere", userSelect: "text" }}>
          {entry.message}
        </Box>
        <Box component="details" sx={{ mt: 1.25, color: "text.secondary", fontSize: 12, "& summary": { cursor: "pointer" } }}>
          <summary>原始日志</summary>
          <Box component="pre" sx={{ mb: 0, whiteSpace: "pre-wrap", overflowWrap: "anywhere", userSelect: "text" }}>{entry.raw}</Box>
        </Box>
      </Box>}
    </Box>
  );
}

export function LogsPanel({ text, loading, fetching, error, paused, onPausedChange, onRefresh }: {
  text: string; loading: boolean; fetching: boolean; error?: string;
  paused: boolean; onPausedChange: (value: boolean) => void; onRefresh: () => void;
}) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<LogFilter>("all");
  const entries = useMemo(() => parseCoreLogs(text), [text]);
  const visible = useMemo(() => filterCoreLogs(entries, filter, query).reverse(), [entries, filter, query]);
  return (
    <Paper sx={{ overflow: "hidden" }}>
      <Stack direction="row" sx={{ px: 2, height: 54, alignItems: "center", justifyContent: "space-between", borderBottom: 1, borderColor: "divider" }}>
        <Stack direction="row" spacing={1} sx={{ alignItems: "baseline", minWidth: 0 }}>
          <Typography sx={{ fontWeight: 600, whiteSpace: "nowrap" }}>内核日志</Typography>
          <Typography variant="caption" color="text.secondary" sx={{ whiteSpace: "nowrap" }}>
            {visible.length} 条 · {paused ? "已暂停更新" : "最新在前"}
          </Typography>
        </Stack>
        <Stack direction="row" spacing={0.5}>
          <Button size="small" startIcon={paused ? <PlayArrowOutlined /> : <PauseOutlined />}
            title="阅读历史日志或展开详情时自动暂停更新"
            onClick={() => onPausedChange(!paused)}>{paused ? "继续更新" : "暂停更新"}</Button>
          <Button size="small" startIcon={<RefreshOutlined />} disabled={fetching} onClick={onRefresh}>
            {fetching ? "刷新中…" : "刷新"}
          </Button>
        </Stack>
      </Stack>
      <Box sx={{ height: "var(--overview-content-height, 480px)", display: "flex", flexDirection: "column" }}>
        <Stack direction="row" spacing={1} sx={{ px: 2, py: 1.25, flexShrink: 0 }}>
          <SearchField label="搜索日志消息" value={query} onChange={setQuery} sx={{ flex: 1, minWidth: 0 }} />
          <TextField select size="small" value={filter} onChange={(e) => setFilter(e.target.value as LogFilter)}
            slotProps={{ select: { inputProps: { "aria-label": "日志级别" } } }} sx={{ width: 140 }}>
            <MenuItem value="all">全部级别</MenuItem>
            <MenuItem value="problems">警告与错误</MenuItem>
            {Object.entries(LOG_LEVEL_LABELS).map(([value, label]) => <MenuItem key={value} value={value}>{label}</MenuItem>)}
          </TextField>
        </Stack>
        {error && <Alert severity="error">{error}</Alert>}
        <Box sx={{ display: "grid", gridTemplateColumns: ROW_GRID, gap: 1.25, px: 2, py: 0.75, bgcolor: "action.hover", borderBottom: 1, borderColor: "divider", fontSize: 12, color: "text.secondary", flexShrink: 0 }}>
          <span>时间</span><span>级别</span><span>消息</span><span />
        </Box>
        <Box role="region" aria-label="内核日志列表" tabIndex={0}
          onScroll={(event) => { if (!paused && event.currentTarget.scrollTop > 24) onPausedChange(true); }}
          sx={{ flex: 1, minHeight: 0, overflow: "auto", scrollbarGutter: "stable" }}>
          {visible.length ? <Box component="ul" aria-label="日志记录" sx={{ m: 0, p: 0, listStyle: "none" }}>
            {visible.map((entry) => <LogRow key={entry.id} entry={entry} onInspect={() => onPausedChange(true)} />)}
          </Box> : <Typography sx={{ p: 5, textAlign: "center", color: "text.secondary", fontSize: 13 }}>
            {loading ? "正在读取日志…" : error && !entries.length ? "日志读取失败，请刷新重试" : entries.length ? "没有匹配的日志" : "暂无内核日志"}
          </Typography>}
        </Box>
      </Box>
    </Paper>
  );
}

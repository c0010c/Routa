import NotificationsActiveOutlined from "@mui/icons-material/NotificationsActiveOutlined";
import NotificationsOffOutlined from "@mui/icons-material/NotificationsOffOutlined";
import { Box, Tooltip } from "@mui/material";
import type { MonitorTarget } from "../bindings";

/** Card mark for a monitored node; one node can be monitored through several paths. */
export function MonitorMark({ targets }: { targets: MonitorTarget[] }) {
  if (!targets.length) return null;
  const down = targets.filter((t) => t.down);
  const title = down.length
    ? down.length === 1 && targets.length === 1
      ? `不可用 · 连续失败 ${down[0].fails} 次`
      : ["不可用 · 连续失败", ...down.map((t) => `${t.label}：${t.fails} 次`)].join("\n")
    : "监测中";
  return (
    <Tooltip title={<span style={{ whiteSpace: "pre-line" }}>{title}</span>}>
      <Box
        component="span"
        aria-label={down.length ? "监测：不可用" : "监测中"}
        sx={{ display: "inline-flex", alignItems: "center", gap: "2px", color: down.length ? "error.main" : "primary.main" }}
      >
        {down.length ? <NotificationsOffOutlined sx={{ fontSize: 13 }} /> : <NotificationsActiveOutlined sx={{ fontSize: 13 }} />}
        {down.length ? "不可用" : "监测"}
      </Box>
    </Tooltip>
  );
}

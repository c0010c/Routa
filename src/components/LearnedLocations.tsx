import Close from "@mui/icons-material/Close";
import ExpandMore from "@mui/icons-material/ExpandMore";
import { Box, Button, IconButton, Stack, Tooltip, Typography } from "@mui/material";
import { useState } from "react";
import type { ProcessRule, RunningProcess } from "../bindings";
import { learningNames, ruleLabel, skippedOf } from "../lib/processRule";

const mono = { fontFamily: "Consolas, monospace", fontSize: 12, wordBreak: "break-all" } as const;

/** 按进程名阻断时系统自动记录的位置；可展开查看、移除。不在学习时不渲染。 */
export function LearnedLocations({
  rule,
  others,
  routingEnabled,
  running,
  onRemove,
}: {
  rule: ProcessRule;
  others: ProcessRule[];
  routingEnabled: boolean;
  running: RunningProcess[];
  onRemove?: (path: string) => void;
}) {
  const [open, setOpen] = useState(false);
  if (learningNames(rule).length === 0) return null;
  const skipped = skippedOf(rule, others, routingEnabled);
  // A learned path another enabled rule claims by path is not guarded by this rule (the backend
  // doesn't send it): it shows up only as skipped and isn't counted.
  const learned = rule.learned.filter((p) => !skipped.some((s) => s.path.toLowerCase() === p.toLowerCase()));
  const n = learned.length;
  const isRunning = (p: string) => running.some((r) => r.exe_path.toLowerCase() === p.toLowerCase());
  return (
    <Box sx={{ fontSize: 12 }}>
      <Button
        size="small"
        color={n === 0 ? "warning" : "inherit"}
        onClick={() => setOpen(!open)}
        endIcon={<ExpandMore sx={{ transform: open ? "rotate(180deg)" : undefined, transition: "transform .15s" }} />}
        sx={{ p: "0 4px", minWidth: 0, fontSize: 12, fontWeight: 400, color: n === 0 ? undefined : "text.secondary" }}
      >
        {n === 0 ? "尚未记录位置" : `已记录 ${n} 个位置`}
        {skipped.length > 0 && `，${skipped.length} 个位置属于其他规则`}
      </Button>
      {open && (
        <Box sx={{ mt: 0.5, pl: 1, borderLeft: 2, borderColor: "divider" }}>
          {n === 0 && (
            <Typography variant="caption" color="warning.main" component="div" sx={{ mb: 0.5 }}>
              程序联网后自动记录。在此之前，分流服务停止时拦不住。
            </Typography>
          )}
          {learned.map((p) => (
            <Stack key={p} direction="row" sx={{ alignItems: "center", gap: 0.5, flexDirection: "row !important" }}>
              <Typography sx={{ ...mono, flex: 1 }}>{p}</Typography>
              {!isRunning(p) && (
                <Typography variant="caption" color="text.secondary" sx={{ flexShrink: 0 }}>
                  未运行
                </Typography>
              )}
              {onRemove && (
                <Tooltip title="移除这个位置（程序再次联网时会重新记录）">
                  <IconButton size="small" aria-label={`移除 ${p}`} onClick={() => onRemove(p)}>
                    <Close sx={{ fontSize: 14 }} />
                  </IconButton>
                </Tooltip>
              )}
            </Stack>
          ))}
          {skipped.map((s) => (
            <Box key={s.path} sx={{ mt: 0.5 }}>
              <Typography sx={{ ...mono, color: "text.disabled" }}>{s.path}</Typography>
              <Typography variant="caption" color="text.secondary">
                不记录：这个位置属于规则「{ruleLabel(s.rule)}」，不受这条规则阻断
              </Typography>
            </Box>
          ))}
        </Box>
      )}
    </Box>
  );
}

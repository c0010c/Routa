import { Box, CircularProgress, Tooltip, Typography } from "@mui/material";
import type { DelayResult } from "../bindings";
import { delayColor, delayFailure, delayText } from "../lib/format";

export interface DelayTextProps {
  result?: DelayResult;
  hint?: string;
  kind?: "node" | "proxy";
  target?: string;
  testUrl?: string;
  interfaceName?: string;
  pending?: boolean;
  running?: boolean;
  variant?: "body" | "card";
  tooltip?: boolean;
  emptyText?: string;
}

/** Completion time and measurement scope stay on demand. */
export function delayDescription({ result, hint, kind = "node", target, testUrl, interfaceName, pending, running = true }: DelayTextProps): string {
  const label = kind === "proxy" ? "当前代理延迟" : "节点延迟";
  const state = pending ? "测量中…" : !running ? "内核未运行" : result?.stale ? "待重测" : result ? delayText(result) : "未测试";
  return [
    `${label}：${state}`,
    target && `测试对象：${target}`,
    (pending || !running || result?.stale) && result && `上次结果：${delayText(result)}`,
    result && result.tested_at > 0 && `测试完成：${new Date(result.tested_at * 1000).toLocaleString("zh-CN", { hour12: false })}`,
    result?.ms === null && delayFailure(result.error).detail,
    testUrl && `测试地址：${testUrl}`,
    interfaceName && `出口网卡：${interfaceName}`,
    result?.stale && "测量后路径或测试配置已变更，请重新测延迟。",
    kind === "proxy" && "按当前代理的出口路径测量，与节点延迟独立记录。",
    hint,
  ].filter(Boolean).join("\n");
}

export function DelayText(props: DelayTextProps) {
  const { result, pending = false, running = true, variant = "body", tooltip = true, emptyText = "—" } = props;
  const numeric = !pending && running && !result?.stale && result?.ms != null;
  const text = pending ? "测量中…" : !running ? "内核未运行" : result?.stale ? "待重测" : result ? delayText(result) : emptyText;
  const color = pending || !running || result?.stale || !result ? "text.secondary" : `${delayColor(result.ms)}.main`;
  const content = (
    <Typography component="span" aria-label={text} tabIndex={tooltip ? 0 : undefined}
      sx={{
        display: "inline-flex", alignItems: "baseline", justifyContent: "flex-end", gap: "3px",
        whiteSpace: "nowrap", fontVariantNumeric: "tabular-nums", color,
        fontSize: variant === "card" ? numeric ? 16 : 12 : 14, fontWeight: numeric ? 600 : 500,
        lineHeight: "20px", minHeight: 20,
        "&:focus-visible": { outline: "2px solid", outlineColor: "primary.main", outlineOffset: 2 },
        "@keyframes routa-delay-result": { from: { opacity: 0.25 }, to: { opacity: 1 } },
        "@media (prefers-reduced-motion: reduce)": { "& *": { animation: "none !important" } },
      }}>
      {pending && <CircularProgress size={10} color="inherit" sx={{ alignSelf: "center" }} />}
      {numeric && variant === "card" ? (
        <Box component="span" key={`${result.tested_at}:${result.ms}`} sx={{ display: "inline-flex", alignItems: "baseline", gap: "3px", animation: "routa-delay-result 150ms ease-out" }}>
          {result.ms}{" "}<Box component="span" sx={{ fontSize: 11, lineHeight: 1, fontWeight: 400, color: "text.secondary" }}>ms</Box>
        </Box>
      ) : text}
    </Typography>
  );
  return tooltip ? <Tooltip title={<Box sx={{ whiteSpace: "pre-line" }}>{delayDescription(props)}</Box>}>{content}</Tooltip> : content;
}

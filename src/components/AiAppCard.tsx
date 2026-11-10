import { Box, Button, ButtonBase, CircularProgress, Link, Switch, Tooltip, Typography } from "@mui/material";
import { useState } from "react";
import type { AiAppView, Model, NodeRef } from "../bindings";
import { commands } from "../bindings";
import { useAiAppDelay } from "../api/aiApps";
import { useProcRouteStatus } from "../api/procroute";
import { ruleExit } from "../lib/hidden";
import { nodeLabel } from "../lib/refs";
import { useApplyResult } from "./ApplyResultSnackbar";
import { DelayText } from "./DelayText";
import { NodePickerDialog } from "./NodePickerDialog";

const FADED = { filter: "grayscale(1)", opacity: 0.55 } as const;

const DOT_SX = { position: "absolute", top: 10, right: 10, width: 8, height: 8, borderRadius: "50%" } as const;

function StatusDot({ app, paused, busy, onTransfer }: { app: AiAppView; paused: boolean; busy: boolean; onTransfer(): void }) {
  const status = useProcRouteStatus();
  const view = app.rule_id ? status.data?.actions[app.rule_id] : undefined;
  const live = app.enabled && !paused;
  // "Pending" (waiting for the service to confirm) carries a reason too, but is not a failure.
  const failed = live && view?.action !== "Pending" && !!view?.reason;
  // Some of the app's processes are still routed by a manual rule (skipped when the catalog grew).
  const held = live && !failed && app.held.length > 0;
  const state = failed ? "red" : held ? "yellow" : live && view?.action === "Proxy" ? "green" : "gray";
  const color = { green: "success.main", red: "error.main", yellow: "warning.main", gray: "action.disabled" }[state];
  if (held) {
    const lines = app.held.map((h) => `${h.process} 由进程分流规则「${h.rule_name}」管理`);
    return (
      <Tooltip title={<Box sx={{ whiteSpace: "pre-line" }}>{[...lines, "点击移交给 AI 应用"].join("\n")}</Box>}>
        <ButtonBase
          aria-label={`把 ${app.name} 的进程移交给 AI 应用`}
          data-testid="ai-dot"
          data-state={state}
          disabled={busy}
          onClick={onTransfer}
          sx={{ ...DOT_SX, bgcolor: color }}
        />
      </Tooltip>
    );
  }
  return (
    <Tooltip title={live && view?.reason ? view.reason : ""}>
      <Box component="span" data-testid="ai-dot" data-state={state} sx={{ ...DOT_SX, bgcolor: color }} />
    </Tooltip>
  );
}

export function AiAppCard({
  app,
  model,
  paused,
  busy,
  onToggle,
  onLaunch,
  onPickNode,
  onTransfer,
}: {
  app: AiAppView;
  model: Model;
  paused: boolean;
  /** A flow (enable / disable / launch / change node) is running for this app. */
  busy: boolean;
  onToggle(on: boolean): void;
  onLaunch(): void;
  onPickNode(node: NodeRef): void;
  /** Hand the app's processes still held by manual rules over to the AI rule. */
  onTransfer(): void;
}) {
  const [picking, setPicking] = useState(false);
  const { showError } = useApplyResult();
  const rule = model.process_routing.rules.find((r) => r.id === app.rule_id);
  const proxyName = rule ? ruleExit(model, rule)?.name : undefined;
  const delay = useAiAppDelay(app.id, proxyName);
  const cardSx = {
    position: "relative", p: 2, display: "flex", flexDirection: "column", alignItems: "center", gap: 1,
    border: 1, borderColor: "divider", borderRadius: 2, bgcolor: "background.paper",
  } as const;

  if (!app.installed) {
    return (
      <Box sx={{ ...cardSx, opacity: 0.45 }}>
        <img src={app.icon} alt="" width={48} height={48} />
        <Typography variant="subtitle2">{app.name}</Typography>
        <Link
          component="button"
          variant="caption"
          onClick={async () => {
            const r = await commands.openAiAppDownload(app.id);
            if (r.status === "error") showError(r.error);
          }}
        >
          未安装 · 下载
        </Link>
      </Box>
    );
  }

  const fade = paused ? FADED : undefined;
  const sw = (
    <Switch
      size="small"
      checked={app.enabled}
      disabled={busy}
      color={paused ? "default" : "primary"}
      onChange={(e) => onToggle(e.target.checked)}
      slotProps={{ input: { "aria-label": `${app.name} 代理开关` } }}
    />
  );
  return (
    <Box sx={cardSx}>
      <Box sx={{ display: "flex", ...fade }}>
        <img src={app.icon} alt="" width={48} height={48} />
      </Box>
      <StatusDot app={app} paused={paused} busy={busy} onTransfer={onTransfer} />
      <Typography variant="subtitle2">{app.name}</Typography>
      <Box sx={{ ...fade, display: "flex", flexDirection: "column", alignItems: "center", gap: 0.5 }}>
        <Button size="small" variant="outlined" color="inherit" disabled={busy} onClick={() => setPicking(true)} sx={{ textTransform: "none", maxWidth: "100%" }}>
          <Box component="span" sx={{ overflow: "hidden", textOverflow: "ellipsis" }}>
            {app.node ? nodeLabel(model, app.node) : "选择节点"}
          </Box>
        </Button>
        {proxyName && (
          <ButtonBase aria-label={`测量 ${app.name} 的延迟`} onClick={delay.measure} sx={{ borderRadius: 0.5, px: 0.5 }}>
            <DelayText result={delay.result} pending={delay.pending} kind="proxy" variant="card" tooltip={false} emptyText="未测试" />
          </ButtonBase>
        )}
      </Box>
      <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
        {app.enabled && paused ? <Tooltip title="进程分流已关闭，开启后恢复"><span>{sw}</span></Tooltip> : sw}
        <Button
          size="small"
          variant={app.running ? "outlined" : "contained"}
          disabled={busy}
          startIcon={busy ? <CircularProgress size={10} color="inherit" /> : undefined}
          onClick={onLaunch}
        >
          {app.running ? "运行中" : "启动"}
        </Button>
      </Box>
      {picking && (
        <NodePickerDialog
          title={`选择节点 · ${app.name}`}
          value={app.node}
          onClose={() => setPicking(false)}
          onPick={(node) => {
            setPicking(false);
            onPickNode(node);
          }}
        />
      )}
    </Box>
  );
}

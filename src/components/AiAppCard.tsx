import { Box, Button, ButtonBase, Link, Switch, Tooltip, Typography } from "@mui/material";
import { useState } from "react";
import type { AiAppView, Model, NodeRef } from "../bindings";
import { commands } from "../bindings";
import { useAiAppDelay } from "../api/aiApps";
import { useProcRouteStatus } from "../api/procroute";
import { ruleExit } from "../lib/hidden";
import { nodeLabel } from "../lib/refs";
import { DelayText } from "./DelayText";
import { NodePickerDialog } from "./NodePickerDialog";

const FADED = { filter: "grayscale(1)", opacity: 0.55 } as const;

function StatusDot({ app, paused }: { app: AiAppView; paused: boolean }) {
  const status = useProcRouteStatus();
  const view = app.rule_id ? status.data?.actions[app.rule_id] : undefined;
  const live = app.enabled && !paused;
  const color = live && view?.action === "Proxy" ? "success.main" : live && view?.reason ? "error.main" : "action.disabled";
  return (
    <Tooltip title={live && view?.reason ? view.reason : ""}>
      <Box component="span" data-testid="status-dot" sx={{ width: 8, height: 8, borderRadius: "50%", bgcolor: color, flexShrink: 0 }} />
    </Tooltip>
  );
}

export function AiAppCard({
  app,
  model,
  paused,
  onToggle,
  onLaunch,
  onPickNode,
}: {
  app: AiAppView;
  model: Model;
  paused: boolean;
  onToggle(on: boolean): void;
  onLaunch(): void;
  onPickNode(node: NodeRef): void;
}) {
  const [picking, setPicking] = useState(false);
  const rule = model.process_routing.rules.find((r) => r.id === app.rule_id);
  const proxyName = rule ? ruleExit(model, rule)?.name : undefined;
  const delay = useAiAppDelay(app.id, proxyName);
  const cardSx = {
    p: 2, display: "flex", flexDirection: "column", alignItems: "center", gap: 1,
    border: 1, borderColor: "divider", borderRadius: 2, bgcolor: "background.paper",
  } as const;

  if (!app.installed) {
    return (
      <Box sx={{ ...cardSx, opacity: 0.45 }}>
        <img src={app.icon} alt="" width={48} height={48} />
        <Typography variant="subtitle2">{app.name}</Typography>
        <Link component="button" variant="caption" onClick={() => void commands.openAiAppDownload(app.id)}>
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
      <Box sx={{ display: "flex", alignItems: "center", gap: 0.75 }}>
        <StatusDot app={app} paused={paused} />
        <Typography variant="subtitle2">{app.name}</Typography>
      </Box>
      <Box sx={{ ...fade, display: "flex", flexDirection: "column", alignItems: "center", gap: 0.5 }}>
        <Button size="small" variant="outlined" color="inherit" onClick={() => setPicking(true)} sx={{ textTransform: "none", maxWidth: "100%" }}>
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
        <Button size="small" variant={app.running ? "outlined" : "contained"} onClick={onLaunch}>
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

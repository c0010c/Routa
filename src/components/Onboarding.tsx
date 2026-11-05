// Full-view first-run card, shown above the runtime summary until capture is on.
import { Box, Button, Paper, Stack, Tooltip, Typography } from "@mui/material";
import { Link } from "react-router-dom";
import type { ReactNode } from "react";

function Step({ n, done, current, title, desc, children }: {
  n: number; done: boolean; current: boolean; title: string; desc: string; children?: ReactNode;
}) {
  return (
    <Stack direction="row" spacing={1.5} sx={{ alignItems: "center", py: 1, minHeight: 52 }}>
      <Box
        aria-label={done ? `第 ${n} 步已完成` : `第 ${n} 步`}
        sx={{
          width: 24, height: 24, borderRadius: "50%", flex: "none", fontSize: 12,
          display: "flex", alignItems: "center", justifyContent: "center",
          color: done ? "success.contrastText" : current ? "primary.contrastText" : "text.secondary",
          bgcolor: done ? "success.main" : current ? "primary.main" : "action.selected",
        }}
      >
        {done ? "✓" : n}
      </Box>
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography sx={{ color: done || current ? "text.primary" : "text.secondary" }}>{title}</Typography>
        <Typography variant="caption" color="text.secondary">{desc}</Typography>
      </Box>
      {current && <Stack direction="row" spacing={1}>{children}</Stack>}
    </Stack>
  );
}

export function Onboarding({ nodes, proxyReady, systemReason, busy, onNewProxy, onQuickStart, onCapture }: {
  nodes: number;
  proxyReady: boolean;
  systemReason: string;
  busy: boolean;
  onNewProxy: () => void;
  onQuickStart: () => void;
  onCapture: () => void;
}) {
  // Nodes are optional: a direct proxy works without them, so step 2 is reachable either way.
  const step = proxyReady ? 3 : nodes > 0 ? 2 : 1;
  return (
    <Paper sx={{ p: 2.5 }} component="section" aria-label="开始使用">
      <Stack direction="row" sx={{ justifyContent: "space-between", alignItems: "baseline" }}>
        <Typography sx={{ fontWeight: 600 }}>开始使用</Typography>
        <Typography variant="caption" color="text.secondary">{step - 1} / 3</Typography>
      </Stack>
      <Step n={1} done={nodes > 0} current={step === 1} title="添加节点" desc={nodes > 0 ? `已有 ${nodes} 个节点` : step > 1 ? "已跳过，只用直连；以后可在节点页添加" : "导入订阅或手动添加；只用直连可跳过"}>
        <Button variant="contained" size="small" component={Link} to="/nodes">去添加节点</Button>
        <Button size="small" disabled={busy} onClick={onQuickStart}>只用直连</Button>
      </Step>
      <Step n={2} done={proxyReady} current={step === 2} title="新增代理" desc="设定本地端口，以及流量走哪个节点">
        <Button variant="contained" size="small" disabled={busy} onClick={onNewProxy}>新增代理</Button>
      </Step>
      <Step n={3} done={false} current={step === 3} title="开启接管" desc="让系统里的应用走这个代理">
        <Tooltip title={systemReason}>
          <span>
            <Button variant="contained" size="small" disabled={busy || !!systemReason} onClick={onCapture}>开启系统代理</Button>
          </span>
        </Tooltip>
      </Step>
    </Paper>
  );
}

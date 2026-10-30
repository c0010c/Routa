import { useRef, useState, type ReactNode } from "react";
import DeleteOutline from "@mui/icons-material/DeleteOutlined";
import ExpandMore from "@mui/icons-material/ExpandMore";
import Refresh from "@mui/icons-material/Refresh";
import Tune from "@mui/icons-material/Tune";
import { Box, Button, ButtonBase, Chip, IconButton, Paper, Stack, Switch, Tooltip, Typography } from "@mui/material";
import { useDeleteListener, useModel, useSaveListener, useStatus, useTestDelays, useTraffic } from "../api/client";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { useInterface } from "../components/InterfaceProvider";
import { desktopApi, useDesktopMutation } from "../api/desktop";
import type { Listener } from "../bindings";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { FixedCardGrid } from "../components/FixedCardGrid";
import { ListenerStatusChip } from "../components/StatusChips";
import { DelayText, delayDescription } from "../components/DelayText";
import { SearchField } from "../components/SearchField";
import { formatBytes, listenerNotEffective } from "../lib/format";
import { nodeLabel } from "../lib/refs";
import { ListenerDialog, newListener } from "./ListenerDialog";
import { ListenerNodeDialog } from "./ListenerNodeDialog";

const clickFeedback = {
  borderRadius: 0.5,
  transition: "background-color 120ms ease, color 120ms ease",
  "&:hover": { bgcolor: "action.hover" },
  "&.Mui-focusVisible": { outline: "2px solid", outlineColor: "primary.main", outlineOffset: 2 },
  "@media (prefers-reduced-motion: reduce)": { transition: "none" },
} as const;

function Metric({ label, children }: { label: string; children: ReactNode }) {
  return <Box sx={{ minWidth: 0 }}>
    <Typography component="dt" variant="caption" color="text.secondary" sx={{ mb: 0.5 }}>{label}</Typography>
    <Box component="dd" sx={{ m: 0, minHeight: 28, display: "flex", alignItems: "center", fontSize: 16, fontWeight: 600, fontVariantNumeric: "tabular-nums", whiteSpace: "nowrap" }}>{children}</Box>
  </Box>;
}

export function Listeners() {
  const model = useModel(), status = useStatus(), save = useSaveListener(), del = useDeleteListener(), test = useTestDelays();
  const traffic = useTraffic(), feedback = useApplyResult();
  const desktop = useInterface()?.state;
  const system = useDesktopMutation(async (id: string | null) => id ? desktopApi.systemListener(id) : desktopApi.connect(null));
  const [editing, setEditing] = useState<Listener | null>(null), [deleting, setDeleting] = useState<Listener | null>(null), [search, setSearch] = useState("");
  const [choosing, setChoosing] = useState<{ exitId: string; listenerName: string } | null>(null);
  const measurementLock = useRef(false);
  const measureProxy = async (name: string) => {
    if (measurementLock.current) return;
    measurementLock.current = true;
    try {
      await test.mutateAsync([name]);
    } catch {
      // The shared mutation reports measurement failures through the global feedback.
    } finally {
      measurementLock.current = false;
    }
  };
  if (!model.data || !status.data) return <Typography>正在加载代理…</Typography>;
  const m = model.data, s = status.data;
  const shown = m.listeners.filter((listener) => `${listener.name} ${listener.port}`.toLowerCase().includes(search.toLowerCase()));
  const failIds = new Set(s.failed_listeners.map((failure) => failure.id));
  return <Stack spacing={2}>
    <Box sx={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 2 }}>
      <Typography variant="h5">代理</Typography>
      <Button variant="contained" onClick={() => setEditing(newListener(m.listeners))}>新增代理</Button>
    </Box>
    <SearchField label="搜索名称或端口" value={search} onChange={setSearch} />
    <Box sx={{ height: "calc(100dvh - 210px)", minHeight: 300, overflowY: "auto", scrollbarGutter: "stable" }}>
      <FixedCardGrid width={760} stretch>
        {shown.map((listener) => {
          const target = m.exits.find((exit) => exit.id === listener.exit), health = s.health.listeners[listener.id], kind = target?.kind;
          const group = kind && typeof kind === "object" && "Group" in kind ? kind.Group : null;
          const manualGroup = group?.strategy === "Select";
          const strategy = group ? { UrlTest: "自动测速", Fallback: "故障转移", LoadBalance: "负载均衡", Select: "手动选择" }[group.strategy]
            : kind === "Direct" ? "直连" : kind ? "固定节点" : "未设置";
          const selected = (manualGroup ? group.selected : null) ?? (kind && typeof kind === "object" && "Node" in kind ? kind.Node.node : null);
          const name = selected ? nodeLabel(m, selected) : group ? manualGroup ? "尚未选择" : "由内核选择" : strategy;
          const source = selected && "subscription_id" in selected ? m.subscriptions.find((sub) => sub.id === selected.subscription_id)?.name : selected ? "手动节点" : "";
          const interfaceName = target?.interface ? s.interfaces.find((item) => item.guid === target.interface)?.friendly_name ?? "网卡不可用" : "系统默认网卡";
          const delay = target ? s.delays[target.name] : undefined;
          const pending = test.isPending && !!target && !!test.variables?.[0]?.includes(target.name);
          const rate = traffic?.per_listener_rate[listener.id], bytes = traffic?.totals.per_listener[listener.id];
          const host = listener.listen.includes(":") ? `[${listener.listen}]` : listener.listen;
          const delayProps = { result: delay, kind: "proxy" as const, target: target?.name, testUrl: m.settings.delay_test_url, running: s.core === "Running", interfaceName, pending };
          return <Paper component="article" aria-label={listener.name} key={listener.id} sx={{ minWidth: 0, overflow: "hidden", containerType: "inline-size" }}>
            <Box sx={{ px: 2, py: 1.5, display: "flex", alignItems: "center", justifyContent: "space-between", gap: 1.5, flexWrap: "wrap" }}>
              <Box sx={{ flex: 1, minWidth: 140 }}>
                <Typography noWrap title={listener.name} sx={{ fontWeight: 600, fontSize: 18 }}>{listener.name}</Typography>
                <Typography variant="caption" color="text.secondary" data-selectable="true">{host}:{listener.port} · {listener.listener_type === "Socks" ? "SOCKS5" : listener.listener_type === "Http" ? "HTTP" : "Mixed"}</Typography>
              </Box>
              <Box sx={{ display: "flex", alignItems: "center", gap: 1.25, flexWrap: "wrap" }}>
                {health ? <ListenerStatusChip status={health} isPortFailed={failIds.has(listener.id)} notEffective={listenerNotEffective(s, listener.id)} /> : <Chip label="未知" />}
                <Switch size="small" checked={listener.user_enabled} disabled={save.isPending}
                  slotProps={{ input: { "aria-label": `启用 ${listener.name}` } }} onChange={(_, enabled) => save.mutate({ ...listener, user_enabled: enabled })} />
                {desktop && listener.listener_type === "Mixed" && <Button size="small" disabled={system.isPending || !listener.user_enabled || !target}
                  onClick={async () => { try { await system.mutateAsync(desktop.connection === "System" && desktop.system_listener_id === listener.id ? null : listener.id); } catch (error) { feedback.showError(String(error)); } }}>
                  {desktop.connection === "System" && desktop.system_listener_id === listener.id ? "关闭系统代理" : "系统代理"}
                </Button>}
                <Box role="group" aria-label={`${listener.name} 代理操作`} sx={{ display: "flex", alignItems: "center", gap: 0.5, pl: 1, borderLeft: 1, borderColor: "divider" }}>
                  <Button size="small" startIcon={<Tune sx={{ fontSize: 16 }} />} aria-label={`编辑 ${listener.name}`} onClick={() => setEditing(listener)} sx={{ color: "text.secondary", minWidth: 0 }}>编辑</Button>
                  <Tooltip title="删除代理"><IconButton size="small" aria-label={`删除 ${listener.name}`} onClick={() => setDeleting(listener)} sx={{ color: "text.secondary", "&:hover": { color: "error.main" } }}><DeleteOutline sx={{ fontSize: 18 }} /></IconButton></Tooltip>
                </Box>
              </Box>
            </Box>
            <Box sx={{ borderTop: 1, borderColor: "divider", p: 2, display: "grid", gridTemplateColumns: "1fr", gap: 1.5,
              "@container (min-width: 680px)": { gridTemplateColumns: "minmax(190px,.95fr) minmax(0,1.8fr)", gap: 2.75, alignItems: "center" } }}>
              <Box sx={{ minWidth: 0 }}>
                <Box sx={{ display: "flex", alignItems: "center", gap: 1, mb: 0.5, minHeight: 24 }}>
                  <Typography variant="caption" color="text.secondary">当前节点</Typography>
                  {target && <Chip label={strategy} sx={{ height: 20, fontSize: 11 }} />}
                  {listener.routing && <Chip label="规则分流" sx={{ height: 20, fontSize: 11 }} />}
                </Box>
                {target && kind !== "Direct" ? <Tooltip title={manualGroup ? "点击切换节点" : "点击编辑节点配置"}>
                  <ButtonBase aria-label={manualGroup ? `切换 ${listener.name} 的节点` : `编辑 ${listener.name} 的节点配置`} aria-haspopup="dialog"
                    onClick={() => manualGroup ? setChoosing({ exitId: target.id, listenerName: listener.name }) : setEditing(listener)}
                    sx={{ ...clickFeedback, maxWidth: "100%", minHeight: 30, gap: 0.5, color: "primary.main", textAlign: "left", justifyContent: "flex-start" }}>
                    <Typography noWrap sx={{ fontWeight: 600, fontSize: 18 }}>{name}</Typography><ExpandMore sx={{ fontSize: 16 }} />
                  </ButtonBase>
                </Tooltip> : <Typography sx={{ fontSize: 18, fontWeight: 600, minHeight: 30 }}>{name}</Typography>}
                <Typography variant="caption" color="text.secondary" sx={{ display: "block", mt: 0.25, overflowWrap: "anywhere" }}>{[source, interfaceName].filter(Boolean).join(" · ")}</Typography>
              </Box>
              <Box component="dl" sx={{ m: 0, display: "grid", gap: 1.5, gridTemplateColumns: "repeat(2,minmax(0,1fr))", "@container (min-width: 480px)": { gridTemplateColumns: "repeat(4,minmax(0,1fr))" } }}>
                <Metric label="当前代理延迟">
                  <Tooltip title={<Box sx={{ whiteSpace: "pre-line" }}>{delayDescription(delayProps)}{"\n"}点击重新测延迟</Box>}>
                    <Box component="span" sx={{ display: "inline-flex" }}>
                      <ButtonBase aria-label={`测量 ${listener.name} 的代理延迟`} aria-busy={pending || undefined} disabled={!target || test.isPending || s.core !== "Running"}
                        onClick={() => target && void measureProxy(target.name)}
                        sx={{ ...clickFeedback, minHeight: 28, minWidth: 88, justifyContent: "flex-start", gap: 0.5 }}>
                        <DelayText {...delayProps} tooltip={false} emptyText="未测试" variant="card" />
                        {!pending && <Refresh sx={{ fontSize: 13, color: "text.secondary" }} />}
                      </ButtonBase>
                    </Box>
                  </Tooltip>
                </Metric>
                <Metric label="上传速率">{formatBytes(rate?.up ?? 0)}/s</Metric>
                <Metric label="下载速率">{formatBytes(rate?.down ?? 0)}/s</Metric>
                <Metric label="累计流量">{formatBytes((bytes?.up ?? 0) + (bytes?.down ?? 0))}</Metric>
              </Box>
            </Box>
          </Paper>;
        })}
      </FixedCardGrid>
      {!shown.length && <Typography sx={{ py: 8, textAlign: "center", color: "text.secondary" }}>{search ? "未找到匹配代理" : "暂无代理，新增代理配置端口与去向。"}</Typography>}
    </Box>
    <Typography variant="caption" color="text.secondary">{shown.length} / {m.listeners.length} 个代理</Typography>
    {editing && <ListenerDialog key={editing.id} listener={editing} onClose={() => setEditing(null)} />}
    {choosing && <ListenerNodeDialog key={choosing.exitId} {...choosing} onClose={() => setChoosing(null)} />}
    {deleting && <ConfirmDeleteDialog text={`确定删除代理「${deleting.name}」吗？引用它的订阅备用连接将取消；进程分流引用需先解除。`} onConfirm={() => del.mutateAsync(deleting.id)} onClose={() => setDeleting(null)} />}
  </Stack>;
}

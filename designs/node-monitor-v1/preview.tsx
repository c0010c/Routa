// Node monitoring prototype. Production Layout / Settings / ResourceCenter with
// fixture data; the monitor section, notifications and controls are prototype-only.
import { createRoot } from "react-dom/client";
import { createPortal } from "react-dom";
import { useEffect, useState } from "react";
import {
  Box, Button, Checkbox, Chip, Collapse, CssBaseline, FormControlLabel, IconButton, InputAdornment, MenuItem,
  Paper, Select, Stack, Switch, TextField, ThemeProvider, Typography,
} from "@mui/material";
import CloseOutlined from "@mui/icons-material/CloseOutlined";
import ScienceOutlined from "@mui/icons-material/ScienceOutlined";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Route, Routes, useNavigate } from "react-router-dom";
import { theme } from "../../src/theme";
import appIcon from "../../src-tauri/icons/32x32.png";
import { Layout } from "../../src/components/Layout";
import { ApplyResultProvider } from "../../src/components/ApplyResultSnackbar";
import { ResourceCenter } from "../../src/pages/ResourceCenter";
import { Settings } from "../../src/pages/Settings";
import { Row, Section } from "../../src/components/SettingsSection";
import { MemberPicker } from "../../src/components/MemberPicker";
import { useModel } from "../../src/api/client";
import { commands, type Model, type RuntimeStatus, type DelayResult } from "../../src/bindings";
import {
  IN_USE, dismissToast, memberOf, displayName, getMonitor, monitoredProxies, pushToast, setMonitor, useMonitor, type NodeState,
} from "./monitorStore";

// ---------- fixture (same shape as designs/node-compact-v5) ----------
const names = ["日本 · 东京 01", "新加坡 · 狮城 01", "美国 · 洛杉矶 02", "日本 · 大阪 02", "德国 · 法兰克福 01", "英国 · 伦敦 01", "日本 · 东京 IPLC 专线 03", "澳大利亚 · 悉尼 01", "香港 · 中环 01", "日本 · 东京 05", "新加坡 · 滨海 02", "加拿大 · 温哥华 01", "美国 · 圣何塞 01", "荷兰 · 阿姆斯特丹 01", "法国 · 巴黎 01", "新加坡 · 狮城 IEPL 03", "台湾 · 台北 01", "韩国 · 首尔 01", "美国 · 西雅图 03", "英国 · 曼彻斯特 02"];
const protocols = ["Shadowsocks", "VLESS", "Trojan", "Trojan", "VLESS", "Hysteria2", "Shadowsocks", "VMess"];
const values = [48, 187, 1280, 170, null, null, 63, undefined, 32, 118, 268, 361, 525, null, null, undefined, 89, 143, 601, 590];
const now = Math.floor(Date.now() / 1000);
let model = {
  subscriptions: [0, 1].map((i) => ({ id: `s${i + 1}`, name: `示例订阅 ${i ? "B" : "A"}`, url: "https://example.invalid/subscription", update_interval_mins: 60, fallback_exit: null, last_updated: null, last_result: null, nodes: names.slice(i ? 8 : 0, i ? 20 : 8).map((name, j) => ({ name, proxy_type: protocols[(j + (i ? 8 : 0)) % protocols.length], server: "example.invalid", raw: { name, type: "socks5", server: "192.0.2.20", port: 1080 } })) })),
  manual_nodes: [
    { id: "office", name: "办公 SOCKS5", groups: [], kind: { Proxy: { raw: { name: "办公 SOCKS5", type: "socks5", server: "192.0.2.30", port: 1080 } } } },
    { id: "backup", name: "备用 HTTP", groups: [], kind: { Proxy: { raw: { name: "备用 HTTP", type: "http", server: "192.0.2.31", port: 8080 } } } },
  ],
  node_groups: [{ id: "daily", name: "日常使用", members: names.slice(0, 8).map((name) => ({ subscription_id: "s1", name })) }],
  exits: [{ id: "work-exit", name: "工作出口", interface: null, kind: { Group: { strategy: "Select", members: [{ group: { Subscription: "s1" } }, { group: { Subscription: "s2" } }], selected: { subscription_id: "s1", name: names[1] } } } }],
  listeners: [{ id: "work", name: "工作代理", listener_type: "Mixed", listen: "127.0.0.1", port: 7890, auth: null, exit: "work-exit", user_enabled: true }],
  settings: { autostart: false, core_path: null, controller_port: 19097, delay_test_interval_mins: 10, delay_test_url: "https://www.gstatic.com/generate_204" },
  process_routing: { enabled: false, exit_prompt: true, rules: [] },
} as unknown as Model;

const baseDelays: Record<string, number | null> = {};
const delays: Record<string, DelayResult> = {};
names.forEach((name, i) => {
  const key = `示例订阅 ${i < 8 ? "A" : "B"}/${name}`;
  if (values[i] === undefined) return;
  baseDelays[key] = values[i]!;
  delays[key] = { ms: values[i]!, tested_at: now - 120, error: values[i] === null ? "Timeout" : null };
});
baseDelays["手动/办公 SOCKS5"] = 36; delays["手动/办公 SOCKS5"] = { ms: 36, tested_at: now - 120 };
let status = {
  core: "Running", last_apply: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } },
  health: { exits: { "work-exit": { status: "Ok", effective_members: [], missing_members: [], interface: null } }, listeners: { work: "Active" } },
  delays, traffic: null, interfaces: [], failed_listeners: [], effective_listeners: ["work"], startup_error: null, save_error: null,
  tailscale: {}, tailscale_exit_errors: {}, tailscale_exit_retrying: {}, core_stop_reason: null, core_recovering: false,
} as unknown as RuntimeStatus;

const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity }, mutations: { retry: false } } });
const pushStatus = () => client.setQueryData(["status"], structuredClone(status));
client.setQueryData(["model"], structuredClone(model));
pushStatus();
for (const name of Object.keys(commands)) (commands as any)[name] = async () => { throw new Error("原型页面：此操作未接入"); };
commands.getModel = async () => structuredClone(model);
commands.getStatus = async () => structuredClone(status);
commands.saveSettings = async (s) => {
  model = { ...model, settings: s };
  client.setQueryData(["model"], structuredClone(model));
  return { status: "ok", data: { Reloaded: { cut: 0, uncut: 0, failed_listeners: [] } } } as any;
};
commands.testDelays = async (targets) => {
  await new Promise((r) => setTimeout(r, 600));
  const result = targets.map((n) => [n, { ms: baseDelays[n] ?? null, tested_at: Math.floor(Date.now() / 1000), error: baseDelays[n] == null ? "Timeout" : null }] as [string, DelayResult]);
  status = { ...status, delays: { ...status.delays, ...Object.fromEntries(result) } };
  pushStatus();
  return { status: "ok", data: result };
};

// ---------- probe simulation ----------
let networkFails = 0;
let networkDown = false;

async function runProbe() {
  const s = getMonitor();
  const proxies = monitoredProxies(model, s);
  if (!s.enabled || proxies.length === 0 || s.probing) return;
  setMonitor({ probing: true });
  await new Promise((r) => setTimeout(r, 900));
  const cur = getMonitor();
  const clock = cur.clock;
  const results = proxies.map((p) => {
    const base = p in baseDelays ? baseDelays[p] : 120;
    if (cur.offline || cur.broken.includes(p) || base === null) return { p, ms: null as number | null, error: "超时" };
    if (cur.latencyLimit !== null && base! > cur.latencyLimit) return { p, ms: base, error: `延迟 ${base} ms 超过上限` };
    return { p, ms: base, error: undefined };
  });
  for (const r of results) {
    status.delays[r.p] = { ms: r.error === "超时" ? null : r.ms, tested_at: Math.floor(Date.now() / 1000), error: r.error === "超时" ? "Timeout" : null };
  }
  status = { ...status, delays: { ...status.delays } };
  pushStatus();

  const nodes: Record<string, NodeState> = { ...cur.nodes };
  const allFail = results.length >= 2 && results.every((r) => r.error === "超时");
  if (allFail) {
    // Every watched node failed at once: most likely this machine is offline, so report once.
    networkFails += 1;
    if (!networkDown && networkFails >= cur.failThreshold) {
      networkDown = true;
      pushToast({ kind: "network", title: "网络可能不可用", body: `${results.length} 个监测节点同时探测失败，请检查本机网络。节点状态暂不更新。` });
    }
    setMonitor({ probing: false, lastRun: clock });
    return;
  }
  if (networkDown && cur.notifyRecovery) pushToast({ kind: "up", title: "网络已恢复", body: "监测节点重新可以连通。" });
  networkDown = false; networkFails = 0;

  const newlyDown: string[] = [];
  for (const r of results) {
    const prev = nodes[r.p] ?? { fails: 0, down: false, lastMs: null };
    if (r.error) {
      const fails = prev.fails + 1;
      const down = prev.down || fails >= cur.failThreshold;
      if (down && !prev.down) newlyDown.push(r.p);
      nodes[r.p] = { fails, down, downSince: prev.down ? prev.downSince : down ? clock : undefined, lastMs: r.ms, lastError: r.error };
    } else {
      if (prev.down && cur.notifyRecovery) {
        const secs = clock - (prev.downSince ?? clock); const mins = Math.round(secs / 60);
        pushToast({ kind: "up", proxy: r.p, name: displayName(r.p), title: `节点已恢复：${displayName(r.p)}`, body: `${mins < 1 ? "不可用不到 1 分钟" : `不可用约 ${mins} 分钟`}，当前延迟 ${r.ms} ms` });
      }
      nodes[r.p] = { fails: 0, down: false, lastMs: r.ms };
    }
  }
  if (newlyDown.length >= 3) {
    pushToast({ kind: "down", proxy: newlyDown[0], name: displayName(newlyDown[0]), title: `${newlyDown.length} 个节点不可用`, body: newlyDown.map(displayName).join("、") });
  } else {
    for (const p of newlyDown) {
      const n = nodes[p];
      const used = IN_USE.find((x) => x.proxy === p);
      pushToast({ kind: "down", proxy: p, name: displayName(p), title: `节点不可用：${displayName(p)}`, body: `连续 ${n.fails} 次探测失败（${n.lastError}）${used ? ` · 正在被「${used.usedBy}」使用` : ""}` });
    }
  }
  setMonitor({ probing: false, lastRun: clock, nodes });
}

const hhmm = (t: number) => new Date(t * 1000).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
const intervalLabel = (v: number) => (v < 60 ? `${v} 秒` : v < 3600 ? `${v / 60} 分钟` : "1 小时");

// ---------- settings section (prototype of the real one) ----------
function MonitorSection() {
  const mon = useMonitor();
  const m = useModel().data;
  const proxies = monitoredProxies(m, mon);
  const [showAll, setShowAll] = useState(false);
  if (!m) return null;
  const downCount = proxies.filter((p) => mon.nodes[p]?.down).length;
  const listed = showAll ? proxies : proxies.slice(0, 5);

  const rows = [
    <Row key="enabled" label="节点监测" description="定时探测节点，不可用时发送系统通知">
      <Switch checked={mon.enabled} onChange={(_, v) => setMonitor({ enabled: v })} slotProps={{ input: { "aria-label": "节点监测" } }} />
    </Row>,
    mon.enabled && (
      <Row key="scope" label="监测范围"
        description={mon.scope === "inUse" ? `代理和进程分流当前使用的节点，随配置自动变化 · ${proxies.length} 个` : `${proxies.length} 个节点`}
        below={mon.scope === "custom" && <MemberPicker label="选择节点或整组" value={mon.members} onChange={(v) => setMonitor({ members: v })} />}>
        <Select size="small" value={mon.scope} onChange={(e) => {
          const scope = e.target.value as "inUse" | "custom";
          setMonitor((s) => ({ scope, members: scope === "custom" && s.members.length === 0 ? IN_USE.map((x) => memberOf(m, x.proxy)).filter((x) => x !== null) : s.members }));
        }} sx={{ width: 180 }} inputProps={{ "aria-label": "监测范围" }}>
          <MenuItem value="inUse">正在使用的节点</MenuItem>
          <MenuItem value="custom">指定节点</MenuItem>
        </Select>
      </Row>
    ),
    mon.enabled && (
      <Row key="interval" label="探测间隔">
        <Select size="small" value={mon.intervalSecs} onChange={(e) => setMonitor({ intervalSecs: Number(e.target.value) })} sx={{ width: 180 }} inputProps={{ "aria-label": "探测间隔" }}>
          {[10, 30, 60, 180, 300, 600, 1800, 3600].map((v) => <MenuItem key={v} value={v}>{intervalLabel(v)}</MenuItem>)}
        </Select>
      </Row>
    ),
    mon.enabled && (
      <Row key="threshold" label="判定为不可用" description="连续失败达到次数才通知，避免偶发抖动误报">
        <Select size="small" value={mon.failThreshold} onChange={(e) => setMonitor({ failThreshold: Number(e.target.value) })} sx={{ width: 180 }} inputProps={{ "aria-label": "判定为不可用" }}>
          {[1, 2, 3, 5].map((v) => <MenuItem key={v} value={v}>{v === 1 ? "失败 1 次即通知" : `连续失败 ${v} 次`}</MenuItem>)}
        </Select>
      </Row>
    ),
    mon.enabled && (
      <Row key="latency" label="延迟上限" description="超过上限也算异常；留空只检查能否连通">
        <TextField size="small" type="number" placeholder="不限" value={mon.latencyLimit ?? ""}
          onChange={(e) => setMonitor({ latencyLimit: e.target.value === "" ? null : Math.max(1, Number(e.target.value)) })}
          slotProps={{ input: { endAdornment: <InputAdornment position="end">ms</InputAdornment> }, htmlInput: { "aria-label": "延迟上限", min: 1 } }} sx={{ width: 180 }} />
      </Row>
    ),
    mon.enabled && (
      <Row key="recover" label="恢复时通知" description="不可用的节点重新连通后再通知一次">
        <Switch checked={mon.notifyRecovery} onChange={(_, v) => setMonitor({ notifyRecovery: v })} slotProps={{ input: { "aria-label": "恢复时通知" } }} />
      </Row>
    ),
  ];

  const footer = mon.enabled && (
    <Stack spacing={1} sx={{ px: 2, py: 1.5, borderTop: 1, borderColor: "divider", bgcolor: "action.hover" }}>
      <Stack direction="row" spacing={1.5} sx={{ alignItems: "center" }}>
        <Typography variant="body2" color="text.secondary" sx={{ flex: 1 }}>
          {mon.lastRun === null ? "尚未探测" : <>上次探测 {hhmm(mon.lastRun)} · 下次 {hhmm(mon.lastRun + mon.intervalSecs)} · {proxies.length - downCount} 正常{downCount > 0 && <Box component="span" sx={{ color: "error.main" }}> · {downCount} 不可用</Box>}</>}
        </Typography>
        <Button variant="outlined" size="small" disabled={mon.probing || proxies.length === 0} onClick={() => void runProbe()}>{mon.probing ? "探测中…" : "立即探测"}</Button>
      </Stack>
      {proxies.length > 0 && (
        <Box component="ul" aria-label="监测中的节点" sx={{ m: 0, p: 0, listStyle: "none" }}>
          {listed.map((p) => {
            const n = mon.nodes[p];
            const used = mon.scope === "inUse" ? IN_USE.find((x) => x.proxy === p)?.usedBy : p.split("/")[0];
            return (
              <Stack component="li" key={p} direction="row" spacing={1} sx={{ alignItems: "center", py: 0.25 }}>
                <Box sx={{ width: 6, height: 6, borderRadius: "50%", flexShrink: 0, bgcolor: !n ? "text.disabled" : n.down ? "error.main" : n.fails > 0 ? "warning.main" : "success.main" }} />
                <Typography variant="body2" noWrap sx={{ minWidth: 0 }}>{displayName(p)}</Typography>
                <Typography variant="caption" color="text.secondary" noWrap sx={{ flex: 1 }}>{used}</Typography>
                <Typography variant="caption" sx={{ color: !n ? "text.secondary" : n.down ? "error.main" : n.fails > 0 ? "warning.main" : "text.secondary" }}>
                  {!n ? "等待探测" : n.down ? `不可用 · ${n.lastError}` : n.fails > 0 ? `失败 ${n.fails}/${mon.failThreshold}` : `${n.lastMs} ms`}
                </Typography>
              </Stack>
            );
          })}
          {proxies.length > 5 && <Button size="small" onClick={() => setShowAll((x) => !x)} sx={{ mt: 0.5, px: 0.5 }}>{showAll ? "收起" : `查看全部 ${proxies.length} 个`}</Button>}
        </Box>
      )}
    </Stack>
  );
  return <Section title="节点监测" footer={footer || undefined}>{rows}</Section>;
}

/** Puts the prototype section right under the real "测速" section of Settings › 网络与内核. */
function MonitorSectionPortal() {
  const [host, setHost] = useState<HTMLElement | null>(null);
  useEffect(() => {
    const attach = () => {
      const speed = document.querySelector<HTMLElement>('#settings-panel-network section[aria-label="测速"]');
      if (!speed) return setHost(null);
      let el = speed.nextElementSibling as HTMLElement | null;
      if (!el?.dataset.monitorHost) {
        el = document.createElement("div");
        el.dataset.monitorHost = "1";
        speed.after(el);
      }
      setHost((h) => (h === el ? h : el));
    };
    attach();
    const ob = new MutationObserver(attach);
    ob.observe(document.body, { childList: true, subtree: true });
    return () => ob.disconnect();
  }, []);
  return host ? createPortal(<MonitorSection />, host) : null;
}

// ---------- simulated Windows notifications ----------
function ToastHost() {
  const mon = useMonitor();
  const navigate = useNavigate();
  const open = (t: (typeof mon.toasts)[number]) => {
    dismissToast(t.id);
    if (t.kind === "network") return navigate("/settings?tab=network");
    navigate("/nodes");
    setTimeout(() => {
      const card = document.querySelector<HTMLElement>(`article[aria-label="${t.name}"]`);
      if (!card) return;
      card.scrollIntoView({ block: "center", behavior: "smooth" });
      card.animate([{ boxShadow: "0 0 0 3px rgba(0,103,192,.6)" }, { boxShadow: "0 0 0 3px rgba(0,103,192,0)" }], { duration: 1600, iterations: 2 });
    }, 250);
  };
  return (
    <Stack spacing={1} sx={{ position: "fixed", right: 16, bottom: 16, width: 364, zIndex: 2000 }}>
      {mon.toasts.map((t) => (
        <Paper key={t.id} elevation={8} onClick={() => open(t)} role="alert"
          sx={{ p: "12px 14px", cursor: "pointer", borderRadius: "8px", border: 1, borderColor: "divider", "&:hover": { bgcolor: "action.hover" } }}>
          <Stack direction="row" sx={{ alignItems: "center", mb: 0.75 }}>
            <Box component="img" src={appIcon} alt="" sx={{ width: 16, height: 16, mr: 1 }} />
            <Typography variant="caption" color="text.secondary" sx={{ flex: 1 }}>Routa</Typography>
            <IconButton size="small" aria-label="关闭通知" onClick={(e) => { e.stopPropagation(); dismissToast(t.id); }} sx={{ p: 0.25 }}><CloseOutlined sx={{ fontSize: 14 }} /></IconButton>
          </Stack>
          <Typography variant="body2" sx={{ fontWeight: 600 }}>{t.title}</Typography>
          <Typography variant="body2" color="text.secondary">{t.body}</Typography>
        </Paper>
      ))}
    </Stack>
  );
}

// ---------- prototype-only controls ----------
function SimPanel() {
  const mon = useMonitor();
  const [open, setOpen] = useState(true);
  const proxies = monitoredProxies(model, mon);
  const forward = () => { setMonitor((s) => ({ clock: s.clock + s.intervalSecs })); void runProbe(); };
  return (
    <Paper elevation={6} sx={{ position: "fixed", left: 12, bottom: 12, width: 250, zIndex: 1900, border: 1, borderColor: "warning.main", borderStyle: "dashed" }}>
      <Stack direction="row" sx={{ alignItems: "center", px: 1.5, py: 0.75, cursor: "pointer" }} onClick={() => setOpen((x) => !x)}>
        <ScienceOutlined sx={{ fontSize: 16, mr: 0.75, color: "warning.main" }} />
        <Typography variant="caption" sx={{ flex: 1, fontWeight: 600 }}>原型控制台（不属于产品 UI）</Typography>
        <Typography variant="caption" color="text.secondary">{open ? "收起" : "展开"}</Typography>
      </Stack>
      <Collapse in={open}>
        <Stack spacing={0.75} sx={{ px: 1.5, pb: 1.5 }}>
          <Typography variant="caption" color="text.secondary">模拟时间 {hhmm(mon.clock)}</Typography>
          <Button size="small" variant="contained" disabled={!mon.enabled || mon.probing} onClick={forward}>快进 {intervalLabel(mon.intervalSecs)}（触发一次探测）</Button>
          <FormControlLabel control={<Checkbox size="small" checked={mon.offline} onChange={(_, v) => setMonitor({ offline: v })} />} label={<Typography variant="caption">模拟本机断网</Typography>} />
          <Typography variant="caption" color="text.secondary">让节点故障：</Typography>
          <Box sx={{ maxHeight: 160, overflow: "auto" }}>
            {proxies.length === 0 && <Typography variant="caption" color="text.secondary">没有监测中的节点</Typography>}
            {proxies.map((p) => (
              <FormControlLabel key={p} sx={{ display: "flex", m: 0 }}
                control={<Checkbox size="small" sx={{ p: 0.25 }} checked={mon.broken.includes(p)} onChange={(_, v) => setMonitor((s) => ({ broken: v ? [...s.broken, p] : s.broken.filter((x) => x !== p) }))} />}
                label={<Typography variant="caption" noWrap>{displayName(p)}{baseDelays[p] === null && <Chip label="本就超时" size="small" sx={{ ml: 0.5, height: 16, fontSize: 10 }} />}</Typography>} />
            ))}
          </Box>
        </Stack>
      </Collapse>
    </Paper>
  );
}

const start = new URLSearchParams(location.search).get("view") === "nodes" ? "/nodes" : "/settings?tab=network";
createRoot(document.getElementById("root")!).render(
  <ThemeProvider theme={theme} defaultMode="light" modeStorageKey="routa-node-monitor-proto">
    <CssBaseline />
    <QueryClientProvider client={client}><ApplyResultProvider><MemoryRouter initialEntries={[start]}>
      <Layout><Routes><Route path="/nodes" element={<ResourceCenter />} /><Route path="/settings" element={<Settings />} /><Route path="*" element={<ResourceCenter />} /></Routes></Layout>
      <MonitorSectionPortal />
      <ToastHost />
      <SimPanel />
    </MemoryRouter></ApplyResultProvider></QueryClientProvider>
  </ThemeProvider>,
);

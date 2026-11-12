import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { Alert, Box, Button, CircularProgress, DialogActions, DialogContent, DialogTitle, IconButton, LinearProgress, MenuItem, Paper, Stack, TextField, Tooltip, Typography } from "@mui/material";
import Add from "@mui/icons-material/Add";
import Refresh from "@mui/icons-material/Refresh";
import LanguageOutlined from "@mui/icons-material/LanguageOutlined";
import InfoOutlined from "@mui/icons-material/InfoOutlined";
import ArrowBackOutlined from "@mui/icons-material/ArrowBackOutlined";
import { useModel, useStatus, errorMessage } from "../api/client";
import { useSubscriptionCommands, useSubscriptionFeedback, subscriptionError } from "../api/subscriptions";
import type { Model, NodeRef, RuntimeStatus, Subscription, SubscriptionPreview } from "../bindings";
import { FixedCardGrid } from "../components/FixedCardGrid";
import { RowActions } from "../components/RowActions";
import { SearchField } from "../components/SearchField";
import { EmptyState } from "../components/EmptyState";
import { FormDialog } from "../components/FormDialog";
import { useInterface } from "../components/InterfaceProvider";
import { formatBytes } from "../lib/format";
import { nodeKey } from "../lib/refs";
import { applicationProblem, subscriptionHost, subscriptionProblem, subscriptionTime, subscriptionUsages } from "../lib/subscriptions";
import { SubscriptionDialog, newSubscription } from "./SubscriptionDialog";
import { SubscriptionReviewDialog } from "./SubscriptionReviewDialog";

function Quota({ subscription: s }: { subscription: Subscription }) {
  const usage = s.info?.usage;
  const used = usage?.upload != null && usage.download != null ? usage.upload + usage.download : null;
  const total = usage?.total;
  const days = usage?.expire ? Math.max(0, Math.ceil((usage.expire * 1000 - Date.now()) / 86400000)) : null;
  return <Box sx={{ height: 60 }}>
    {used != null && total ? <>
      <Box sx={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", gap: 1 }}>
        <Typography sx={{ fontWeight: 600, fontSize: 17 }}>{formatBytes(Math.max(0, total - used))}<Box component="span" sx={{ fontSize: 11, color: "text.secondary", ml: .5, fontWeight: 400 }}>剩余</Box></Typography>
        <Typography variant="caption" color="text.secondary" sx={{ fontSize: 11 }}>已用 {formatBytes(used)} / {formatBytes(total)}</Typography>
      </Box>
      <LinearProgress variant="determinate" value={Math.min(100, used / total * 100)} color={used >= total ? "error" : used / total >= .9 ? "warning" : "primary"} aria-label="套餐已用流量" sx={{ height: 4, borderRadius: 3, my: 1 }} />
    </> : <Box sx={{ display: "flex", alignItems: "center", justifyContent: "space-between", minHeight: 34 }}>
      <Typography variant="caption" color="text.secondary">流量信息未提供</Typography><Tooltip title="套餐信息由订阅服务提供，不使用 Routa 本地流量估算套餐余量。"><InfoOutlined sx={{ fontSize: 14, color: "text.secondary" }} /></Tooltip>
    </Box>}
    <Box sx={{ display: "flex", justifyContent: "space-between", gap: 1 }}><Typography variant="caption" color="text.secondary" sx={{ fontSize: 11 }}>{usage?.expire ? `${new Date(usage.expire * 1000).toLocaleDateString("zh-CN")} 到期` : "到期时间未提供"}</Typography>{days != null && <Typography variant="caption" color={days <= 3 ? "warning.main" : "text.secondary"} sx={{ fontSize: 11 }}>{days === 0 ? "已到期或今日到期" : `剩余 ${days} 天`}</Typography>}</Box>
  </Box>;
}

export function SubscriptionCard({ subscription: s, busy, disabled = false, runtime, onUpdate, onEdit, onDelete }: {
  subscription: Subscription; busy: boolean; disabled?: boolean; runtime?: Pick<RuntimeStatus, "core" | "last_apply">; onUpdate: () => void; onEdit: (advanced?: boolean) => void; onDelete: () => void;
}) {
  const problem = subscriptionProblem(s, runtime);
  const failedApply = !!problem && !(s.last_result && "Failed" in s.last_result);
  const lastSuccess = s.info?.last_success ?? (s.last_result && "Ok" in s.last_result && !problem ? s.last_updated : null);
  const next = s.last_updated != null && s.update_interval_mins ? s.last_updated + s.update_interval_mins * 60 : null;
  return <Paper component="article" aria-label={s.name} sx={{ height: 276, overflow: "hidden", display: "flex", flexDirection: "column" }}>
    <Box sx={{ p: 2, display: "flex", flexDirection: "column", gap: 1.5, flex: 1, minHeight: 0 }}>
      <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
        <Box sx={{ display: "grid", placeItems: "center", width: 31, height: 31, flexShrink: 0, bgcolor: "action.hover", borderRadius: 1 }}><LanguageOutlined sx={{ fontSize: 18, color: "primary.main" }} /></Box>
        <Box sx={{ minWidth: 0 }}><Typography noWrap title={s.name} sx={{ fontSize: 16, fontWeight: 600 }}>{s.name}</Typography><Typography variant="caption" color="text.secondary" noWrap sx={{ display: "block", fontSize: 11 }}>{subscriptionHost(s.url)}</Typography></Box>
      </Box>
      <Box sx={{ display: "flex", justifyContent: "space-between", alignItems: "center", minHeight: 18 }}>
        <Box sx={{ display: "flex", alignItems: "center", gap: .5, color: problem ? "error.main" : busy ? "primary.main" : s.last_result ? "success.main" : "text.secondary" }}>
          {busy ? <CircularProgress size={10} /> : <Box sx={{ width: 6, height: 6, borderRadius: "50%", bgcolor: "currentColor", flexShrink: 0 }} />}
          <Tooltip title={!busy && problem ? problem : ""}><Typography variant="caption">{busy ? "正在更新…" : problem ? failedApply ? "配置未生效" : "更新失败" : s.last_result ? "更新成功" : "尚未更新"}</Typography></Tooltip>
        </Box><Typography variant="caption" color="text.secondary" sx={{ fontSize: 11 }}>{s.nodes.length} 个节点</Typography>
      </Box>
      <Quota subscription={s} />
      <Box component="dl" sx={{ m: 0, display: "grid", gridTemplateColumns: "1fr auto", rowGap: .5, fontSize: 12, color: "text.secondary", "& dd": { m: 0, textAlign: "right" } }}>
        <Box component="dt">最近成功</Box><Box component="dd">{subscriptionTime(lastSuccess)}</Box>
        <Box component="dt">下次更新</Box><Box component="dd">{!s.update_interval_mins ? "手动更新" : next ? next * 1000 < Date.now() ? "等待更新" : subscriptionTime(next) : "等待首次更新"}</Box>
      </Box>
    </Box>
    <Box sx={{ height: 49, flexShrink: 0, borderTop: 1, borderColor: "divider", display: "flex", alignItems: "center", gap: 1.5, px: 2 }}>
      <Button size="small" variant="outlined" disabled={busy || disabled} onClick={onUpdate} sx={{ minWidth: 62 }}>{busy ? "更新中…" : problem ? "重试更新" : "更新"}</Button>
      <Button size="small" component={Link} to={`/nodes?source=${encodeURIComponent(s.id)}`}>查看节点</Button>
      <Box sx={{ ml: "auto" }}><RowActions label={s.name} items={[
        { label: "编辑订阅", onClick: () => onEdit(), disabled: busy || disabled },
        ...(problem ? [{ label: "设置备用代理", onClick: () => onEdit(true), disabled: busy || disabled }] : []),
        { label: "删除订阅", danger: true, disabled: busy || disabled, onClick: onDelete },
      ]} /></Box>
    </Box>
  </Paper>;
}

function DeleteSubscription({ sub, model, busy, error, onClose, onConfirm }: { sub: Subscription; model: Model; busy: boolean; error: string; onClose: () => void; onConfirm: (replacement: NodeRef | null) => void }) {
  const [value, setValue] = useState("");
  const usages = subscriptionUsages(model, sub.id);
  const options: { ref: NodeRef; label: string }[] = [
    ...model.subscriptions.filter(s => s.id !== sub.id).flatMap(s => s.nodes.map(n => ({ ref: { subscription_id: s.id, name: n.name }, label: `${s.name} / ${n.name}` }))),
    ...model.manual_nodes.map(n => ({ ref: { manual_id: n.id }, label: `手动 / ${n.name}` })),
  ];
  return <FormDialog onClose={onClose} busy={busy} fullWidth maxWidth="sm"><DialogTitle>删除「{sub.name}」？</DialogTitle><DialogContent><Stack spacing={2} sx={{ pt: 1 }}>
    <Typography variant="body2">将移除订阅及其 {sub.nodes.length} 个节点，不会取消服务商处的套餐。</Typography>
    {!!usages.length && <><Alert severity="warning">以下位置仍在使用此订阅：{usages.join("、")}。请指定替代节点，相关连接可能重建。</Alert><TextField select label="替代节点" value={value} onChange={e => setValue(e.target.value)} disabled={busy}>{options.map(o => <MenuItem key={nodeKey(o.ref)} value={nodeKey(o.ref)}>{o.label}</MenuItem>)}</TextField>{!options.length && <Typography variant="body2">没有其他可用节点，请先添加节点。</Typography>}</>}
    {error && <Alert severity="error">{error}</Alert>}
  </Stack></DialogContent><DialogActions><Button disabled={busy} onClick={onClose}>取消</Button><Button color="error" variant="contained" loading={busy} disabled={!!usages.length && !value} onClick={() => onConfirm(options.find(o => nodeKey(o.ref) === value)?.ref ?? null)}>{usages.length ? "替换引用并删除" : "删除订阅"}</Button></DialogActions></FormDialog>;
}

export function Subscriptions() {
  const model = useModel();
  const runtime = useStatus();
  const latestModel = useRef(model.data);
  latestModel.current = model.data;
  const actions = useSubscriptionCommands();
  const feedback = useSubscriptionFeedback();
  const simple = useInterface()?.state?.preferences.mode === "Simple";
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<{ sub: Subscription; advanced?: boolean } | null>(null);
  const [review, setReview] = useState<SubscriptionPreview | null>(null);
  const [deleting, setDeleting] = useState<Subscription | null>(null);
  const [operation, setOperation] = useState(false);
  const [error, setError] = useState("");
  const locks = useRef(new Set<string>());
  const [pending, setPending] = useState<Set<string>>(new Set());
  const [batch, setBatch] = useState(false);
  const reviewQueue = useRef<{ preview: SubscriptionPreview; done: () => void }[]>([]);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      reviewQueue.current.splice(0).forEach(item => item.done());
    };
  }, []);
  const finishReview = () => {
    const current = reviewQueue.current.shift();
    setReview(reviewQueue.current[0]?.preview ?? null);
    setError("");
    current?.done();
  };
  const resultNotice = (s: Subscription, out: Awaited<ReturnType<typeof actions.commit>>) => {
    const problem = applicationProblem(out);
    feedback.notify(problem ?? `${s.name}更新成功 · ${s.nodes.length} 个节点`, problem ? "error" : "success");
  };
  const update = async (s: Subscription) => {
    if (locks.current.has(s.id)) return;
    locks.current.add(s.id); setPending(new Set(locks.current));
    try {
      const p = await actions.preview(s);
      if (!mounted.current) return;
      if (p.affected.length) {
        await new Promise<void>(done => {
          reviewQueue.current.push({ preview: p, done });
          if (reviewQueue.current.length === 1) { setError(""); setReview(p); }
        });
        return;
      }
      resultNotice(p.subscription, await actions.commit(p, null));
    } catch (e) {
      feedback.notify(`${s.name}更新失败：${subscriptionError(e)}。原有节点未被替换。`, "error", { label: "更新设置", run: () => setEditing({ sub: s, advanced: true }) });
    } finally { locks.current.delete(s.id); setPending(new Set(locks.current)); }
  };
  const updateAll = async () => {
    setBatch(true);
    try {
      const ids = latestModel.current?.subscriptions.map(s => s.id) ?? [];
      for (const id of ids) {
        if (!mounted.current) break;
        const sub = latestModel.current?.subscriptions.find(s => s.id === id);
        if (sub) await update(sub);
      }
    } finally { if (mounted.current) setBatch(false); }
  };
  if (!model.data) return <Stack spacing={2}>{model.error ? <Alert severity="error" action={<Button onClick={() => void model.refetch()}>重试</Button>}>{errorMessage(model.error)}</Alert> : <Typography>正在加载订阅…</Typography>}</Stack>;
  const m = model.data;
  const query = search.trim().toLowerCase();
  const shown = m.subscriptions.filter(s => `${s.name} ${subscriptionHost(s.url)}`.toLowerCase().includes(query));
  return <Stack spacing={2}>
    <Box sx={{ display: "flex", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: 2 }}>
      <Box><Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>{simple && <IconButton component={Link} to="/nodes" aria-label="返回节点"><ArrowBackOutlined /></IconButton>}<Typography variant="h5">订阅</Typography></Box><Typography variant="caption" color="text.secondary" sx={{ mt: .75, display: "block" }}>{m.subscriptions.length} 个订阅 · {m.subscriptions.reduce((n, s) => n + s.nodes.length, 0)} 个节点</Typography></Box>
      <Box sx={{ display: "flex", gap: 1 }}><Button variant="outlined" startIcon={<Refresh />} disabled={batch || !!pending.size || !m.subscriptions.length} onClick={() => void updateAll()}>{batch ? "更新中…" : "全部更新"}</Button><Button variant="contained" startIcon={<Add />} onClick={() => setEditing({ sub: newSubscription() })}>添加订阅</Button></Box>
    </Box>
    <Box sx={{ display: "flex", justifyContent: "flex-end", gap: 1.5, flexWrap: "wrap" }}>
      <SearchField label="搜索名称或来源" value={search} onChange={setSearch} sx={{ width: 245 }} />
    </Box>
    {model.error && <Alert severity="warning">订阅数据可能已过期，请重新读取。<Button onClick={() => void model.refetch()}>重试</Button></Alert>}
    <Box sx={{ height: "calc(100dvh - 225px)", minHeight: 300, overflowY: "auto", scrollbarGutter: "stable" }}>
      <FixedCardGrid width={340} sx={{ gap: 1.75 }}>{shown.map(s => <SubscriptionCard key={s.id} subscription={s} busy={pending.has(s.id)} disabled={batch} runtime={runtime.data} onUpdate={() => void update(s)} onEdit={advanced => setEditing({ sub: s, advanced })} onDelete={() => { setError(""); setDeleting(s); }} />)}</FixedCardGrid>
      {!shown.length && <EmptyState icon={<LanguageOutlined />} title={m.subscriptions.length ? "没有匹配的订阅" : "从一个订阅开始"} description={!m.subscriptions.length ? "粘贴订阅地址，先预览节点，再确认添加。" : undefined} action={!m.subscriptions.length ? <Button variant="contained" onClick={() => setEditing({ sub: newSubscription() })}>添加订阅</Button> : <Button onClick={() => { setSearch(""); }}>查看全部订阅</Button>} />}
    </Box>
    {editing && <SubscriptionDialog key={editing.sub.id} subscription={editing.sub} showAdvanced={editing.advanced} notify={feedback.notify} onClose={() => setEditing(null)} />}
    {review && <SubscriptionReviewDialog key={review.subscription.id} preview={review} busy={operation} error={error} onClose={() => { if (!operation) { finishReview(); feedback.notify("已保留原配置，本次预览未应用", "info"); } }} onConfirm={async replacement => { setOperation(true); setError(""); try { resultNotice(review.subscription, await actions.commit(review, replacement)); finishReview(); } catch (e) { setError(subscriptionError(e)); } finally { setOperation(false); } }} />}
    {deleting && <DeleteSubscription sub={deleting} model={m} busy={operation} error={error} onClose={() => !operation && setDeleting(null)} onConfirm={async replacement => { setOperation(true); setError(""); try { const deleted = await actions.remove(deleting.id, replacement); const problem = applicationProblem(deleted.outcome); feedback.notify(problem ?? `已删除「${deleting.name}」`, problem ? "error" : "success", { label: "撤销", run: async () => { try { const out = await actions.restore(deleted.token); const problem = applicationProblem(out); feedback.notify(problem ?? "订阅及原有引用已恢复", problem ? "error" : "success"); } catch (e) { feedback.notify(subscriptionError(e), "error"); } } }); setDeleting(null); } catch (e) { setError(subscriptionError(e)); } finally { setOperation(false); } }} />}
    {feedback.element}
  </Stack>;
}

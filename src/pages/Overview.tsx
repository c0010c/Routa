import InputOutlined from "@mui/icons-material/InputOutlined";
import {
  Alert,
  Box,
  Button,
  Card,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  Paper,
  Select,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
  useTheme,
} from "@mui/material";
import { SparkLineChart } from "@mui/x-charts/SparkLineChart";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { Link as RouterLink, useSearchParams } from "react-router-dom";
import {
  errorMessage,
  useCoreControl,
  useDeleteListener,
  useGroupNow,
  useModel,
  useResetTraffic,
  useSaveListener,
  useSelectGroupMember,
  useStatus,
  useTraffic,
  useTrafficHistory,
} from "../api/client";
import type { CoreState, Exit, Listener, Model, RuntimeStatus } from "../bindings";
import { expandMembers, nodeKey, nodeLabel } from "../lib/refs";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { DelayText } from "../components/DelayText";
import { EmptyState } from "../components/EmptyState";
import { RowActions } from "../components/RowActions";
import { GettingStarted, needsGettingStarted } from "../components/GettingStarted";
import { ListenerStatusChip } from "../components/StatusChips";
import { ExitDialog, newExit } from "./ExitDialog";
import { ListenerDialog, newListener } from "./ListenerDialog";
import { applyOutcomeMessages, exitStatusLabel, formatBytes, formatRate, listenerNotEffective } from "../lib/format";


const TYPE_TEXT = { Mixed: "混合", Http: "HTTP", Socks: "SOCKS" } as const;

function coreText(c: CoreState): string {
  if (c === "Stopped") return "已停止";
  if (c === "Starting") return "启动中";
  if (c === "Running") return "运行中";
  return `已崩溃（退出码 ${c.Crashed.exit_code ?? "未知"}）`;
}

export function collectProblems(model: Model, status: RuntimeStatus): string[] {
  const out: string[] = [];
  if (status.core === "Stopped" || typeof status.core === "object") out.push(`mihomo ${coreText(status.core)}`);
  const a = status.last_apply;
  if (a !== null && !("Reloaded" in a)) out.push(...applyOutcomeMessages(a));
  for (const [id, h] of Object.entries(status.health.exits)) {
    if (!h || h.status === "Ok") continue;
    const name = model.exits.find((e) => e.id === id)?.name ?? id;
    out.push(`出口「${name}」${exitStatusLabel(h.status).text}`);
  }
  for (const f of status.failed_listeners) {
    out.push(`监听器「${f.name}」端口 ${f.port} 被占用`);
  }
  for (const s of model.subscriptions) {
    if (s.last_result && "Failed" in s.last_result) {
      out.push(`订阅「${s.name}」更新失败：${s.last_result.Failed.reason}`);
    }
  }
  if (status.startup_error) out.push(`启动时发现问题：${status.startup_error}`);
  if (status.save_error) out.push(`保存状态文件失败：${status.save_error}`);
  return out;
}

const CORE_DOT: Record<"Running" | "Starting" | "Stopped" | "Crashed", string> = {
  Running: "success.main",
  Starting: "warning.main",
  Stopped: "text.disabled",
  Crashed: "error.main",
};

export function CoreStatusCard({ core, controllerPort }: { core: CoreState; controllerPort: number }) {
  const c = useCoreControl();
  const kind = typeof core === "object" ? "Crashed" : core;
  return (
    <Card>
      <Stack direction="row" spacing={2} sx={{ alignItems: "center", px: 2, py: 1.5 }}>
        <Box sx={{ width: 10, height: 10, borderRadius: "50%", bgcolor: CORE_DOT[kind], flex: "none" }} />
        <Box sx={{ flex: 1 }}>
          <Typography sx={{ fontWeight: 600 }}>内核{coreText(core)}</Typography>
          <Typography variant="body2" sx={{ color: "text.secondary" }}>
            控制端口 {controllerPort}
          </Typography>
        </Box>
        {kind === "Starting" && <CircularProgress size={16} />}
        {(kind === "Stopped" || kind === "Crashed") && (
          <Button variant="contained" disabled={c.isPending} onClick={() => c.start.mutate()}>
            启动
          </Button>
        )}
        {(kind === "Running" || kind === "Crashed") && (
          <Button variant="outlined" disabled={c.isPending} onClick={() => c.restart.mutate()}>
            重启
          </Button>
        )}
        {kind === "Running" && (
          <Button variant="outlined" disabled={c.isPending} onClick={() => c.stop.mutate()}>
            停止
          </Button>
        )}
      </Stack>
    </Card>
  );
}

function StatCard({ label, value, extra, spark }: { label: string; value: string; extra?: ReactNode; spark?: ReactNode }) {
  return (
    <Card sx={{ position: "relative", minHeight: 84 }}>
      <Box sx={{ px: 2, pt: 1.5, pb: spark ? 0 : 1.5 }}>
        <Stack direction="row" sx={{ alignItems: "center", justifyContent: "space-between" }}>
          <Typography variant="body2" sx={{ color: "text.secondary" }}>
            {label}
          </Typography>
          {extra}
        </Stack>
        <Typography variant="h6" sx={{ fontVariantNumeric: "tabular-nums", fontWeight: 600 }}>
          {value}
        </Typography>
      </Box>
      {spark}
    </Card>
  );
}

function Spark({ data, color }: { data: number[]; color: string }) {
  // A chart needs two points to draw a line.
  if (data.length < 2) return null;
  return <SparkLineChart data={data} height={34} area curve="linear" color={color} margin={{ top: 2, bottom: 0, left: 0, right: 0 }} />;
}

function GroupSelect({ exit }: { exit: Exit }) {
  const select = useSelectGroupMember();
  const model = useModel().data;
  if (!model || typeof exit.kind !== "object" || !("Group" in exit.kind) || exit.kind.Group.strategy !== "Select") {
    return null;
  }
  const { selected } = exit.kind.Group;
  const members = expandMembers(model, exit.kind.Group.members);
  const value = selected && members.some((x) => nodeKey(x) === nodeKey(selected)) ? nodeKey(selected) : "";
  return (
    <Select
      size="small"
      value={value}
      displayEmpty
      onChange={(e) => {
        const m = members.find((x) => nodeKey(x) === e.target.value);
        if (m) select.mutate(exit.id, m);
      }}
      inputProps={{ "aria-label": `${exit.name} 成员` }}
      sx={{ minWidth: 140 }}
    >
      {members.map((m) => (
        <MenuItem key={nodeKey(m)} value={nodeKey(m)}>
          {nodeLabel(model, m)}
        </MenuItem>
      ))}
    </Select>
  );
}

const NEW_EXIT = "__new_exit__";

function currentNode(model: Model, exit: Exit | undefined, now: Partial<Record<string, string>> | undefined): ReactNode {
  if (!exit || exit.kind === "Direct") return "—";
  if ("Node" in exit.kind) return nodeLabel(model, exit.kind.Node.node);
  if (exit.kind.Group.strategy === "Select") return <GroupSelect exit={exit} />;
  const v = now?.[exit.name];
  if (!v) return "—";
  const prefix = `${exit.name}/`;
  return v.startsWith(prefix) ? v.slice(prefix.length) : v;
}

export function Overview({ embedded = false }: { embedded?: boolean } = {}) {
  const model = useModel();
  const status = useStatus();
  const liveTick = useTraffic();
  const history = useTrafficHistory();
  const theme = useTheme();
  const reset = useResetTraffic();
  const saveListener = useSaveListener();
  const delListener = useDeleteListener();
  const [params] = useSearchParams();
  const paramHighlight = params.get("highlight");
  const resolveExit = params.get("resolveExit");
  const [highlight, setHighlight] = useState<string | null>(null);
  const [creating, setCreating] = useState<Listener | null>(null);
  const [editing, setEditing] = useState<Listener | null>(null);
  const [deleting, setDeleting] = useState<Listener | null>(null);
  const [confirmReset, setConfirmReset] = useState(false);
  const [newExitFor, setNewExitFor] = useState<Listener | null>(null);
  const [creatingExit, setCreatingExit] = useState<Exit | null>(null);
  const highlightRef = useRef<HTMLTableRowElement | null>(null);
  const exitSelectRef = useRef<HTMLInputElement | null>(null);
  const groupNames = (model.data?.exits ?? [])
    .filter((e) => typeof e.kind === "object" && "Group" in e.kind)
    .map((e) => e.name);
  const groupNow = useGroupNow(groupNames);
  const loaded = !!model.data && !!status.data;
  const highlightExists = !!model.data?.listeners.some((l) => l.id === paramHighlight);

  useEffect(() => {
    if (!paramHighlight || !loaded || !highlightExists) return;
    setHighlight(paramHighlight);
    const t = setTimeout(() => setHighlight(null), 3000);
    return () => clearTimeout(t);
  }, [paramHighlight, loaded, highlightExists]);

  useEffect(() => {
    if (highlight && loaded) {
      highlightRef.current?.scrollIntoView?.({ block: "center" });
      if (resolveExit) exitSelectRef.current?.focus();
    }
  }, [highlight, loaded, resolveExit]);

  if (!model.data || !status.data) {
    const error = model.error || status.error;
    if (error) return (
      <Alert severity="error" action={
        <Button onClick={() => { void model.refetch(); void status.refetch(); }}>重新加载</Button>
      }>
        {errorMessage(error)}
      </Alert>
    );
    return <Typography>加载中…</Typography>;
  }
  const m = model.data;
  const s = status.data;
  const tick = liveTick ?? s.traffic;
  const problems = collectProblems(m, s);
  const failedIds = new Set(s.failed_listeners.map((f) => f.id));
  const resolvingExit = m.exits.find((e) => e.id === resolveExit);
  const working = m.listeners.filter(
    (l) => s.health.listeners[l.id] === "Active" && !failedIds.has(l.id) && !listenerNotEffective(s, l.id),
  ).length;

  return (
    <Stack spacing={2}>
      {!embedded && <Typography variant="h5">概览</Typography>}
      {resolveExit && (
        <Alert severity="info" action={
          <>
            {model.isError && <Button onClick={() => void model.refetch()}>重新加载</Button>}
            <Button component={RouterLink} to={`/exits?${new URLSearchParams({ delete: resolveExit })}`} disabled={saveListener.isPending}>
              返回删除检查
            </Button>
          </>
        }>
          {model.isError ? `未能读取最新配置：${errorMessage(model.error)}` : !highlightExists
            ? "该监听器已不存在，请返回重新检查。"
            : resolvingExit
              ? `正在处理出口「${resolvingExit.name}」的引用。修改出口会按现有规则保存并应用。`
              : "正在处理的出口已不存在，请返回重新检查。"}
        </Alert>
      )}
      {problems.length > 0 && (
        <Alert severity="error" role="alert">
          {problems.map((p) => (
            <div key={p}>{p}</div>
          ))}
        </Alert>
      )}
      <CoreStatusCard core={s.core} controllerPort={m.settings.controller_port} />
      {needsGettingStarted(m) && (
        <GettingStarted
          model={m}
          onNewExit={() => setCreatingExit(newExit())}
          onNewListener={() => setCreating(newListener(m.listeners))}
        />
      )}
      <Box sx={{ display: "grid", gridTemplateColumns: "repeat(4, minmax(0, 1fr))", gap: 1.5 }}>
        <StatCard
          label="上传速率"
          value={formatRate(tick?.total_rate.up ?? 0)}
          spark={<Spark data={history.up} color={theme.vars?.palette.primary.main ?? theme.palette.primary.main} />}
        />
        <StatCard
          label="下载速率"
          value={formatRate(tick?.total_rate.down ?? 0)}
          spark={<Spark data={history.down} color={theme.vars?.palette.success.main ?? theme.palette.success.main} />}
        />
        <StatCard
          label="累计流量"
          value={`↑ ${formatBytes(tick?.totals.total.up ?? 0)} ↓ ${formatBytes(tick?.totals.total.down ?? 0)}`}
          extra={
            <Button size="small" onClick={() => setConfirmReset(true)} sx={{ minWidth: 0, py: 0 }}>
              清零
            </Button>
          }
        />
        <StatCard label="监听器" value={`${working} / ${m.listeners.length} 在工作`} />
      </Box>
      <Stack component="section" aria-label="监听器" spacing={1}>
        <Stack direction="row" sx={{ alignItems: "center", justifyContent: "space-between" }}>
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>
            监听器
          </Typography>
          <Button variant="contained" onClick={() => setCreating(newListener(m.listeners))}>
            新建监听器
          </Button>
        </Stack>
        {m.listeners.length === 0 ? (
          <EmptyState
            icon={<InputOutlined />}
            title="还没有监听器"
            description="监听器是本地代理端口，设备连这个端口上网。"
          />
        ) : (
          <TableContainer component={Paper}>
            <Table size="small">
              <TableHead>
                <TableRow>
                  {["状态", "监听器", "出口", "当前节点", "网卡", "延迟", "实时", "累计", ""].map((h) => (
                    <TableCell key={h}>{h}</TableCell>
                  ))}
                </TableRow>
              </TableHead>
              <TableBody>
                {m.listeners.map((l) => {
                  const exit = m.exits.find((e) => e.id === l.exit);
                  const ls = s.health.listeners[l.id];
                  const iface = exit?.interface
                    ? (s.interfaces.find((i) => i.guid === exit.interface)?.friendly_name ??
                      s.health.exits[exit.id]?.interface?.friendly_name ??
                      exit.interface)
                    : "系统默认";
                  const d = exit ? s.delays[exit.name] : undefined;
                  const rate = tick?.per_listener_rate[l.id];
                  const total = tick?.totals.per_listener[l.id];
                  return (
                    <TableRow
                      key={l.id}
                      ref={l.id === highlight ? highlightRef : undefined}
                      selected={l.id === highlight}
                      sx={l.id === highlight ? { bgcolor: "action.selected" } : undefined}
                    >
                      <TableCell>
                        {ls ? <ListenerStatusChip status={ls} isPortFailed={failedIds.has(l.id)} notEffective={listenerNotEffective(s, l.id)} /> : "—"}
                      </TableCell>
                      <TableCell>
                        {l.name}
                        <Typography variant="caption" sx={{ display: "block", color: "text.secondary" }}>
                          {TYPE_TEXT[l.listener_type]} · {l.port}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Select
                          size="small"
                          displayEmpty
                          value={exit ? exit.id : ""}
                          inputRef={l.id === paramHighlight ? exitSelectRef : undefined}
                          onChange={(e) => {
                            const v = e.target.value;
                            if (v === NEW_EXIT) setNewExitFor(l);
                            else saveListener.mutate({ ...l, exit: v === "" ? null : v });
                          }}
                          inputProps={{ "aria-label": `${l.name} 出口` }}
                          sx={{
                            minWidth: 140,
                            // A just-created listener without an exit: point at where to pick one.
                            ...(l.id === paramHighlight && !exit ? { outline: "2px solid", outlineColor: "warning.main" } : {}),
                          }}
                        >
                          <MenuItem value="">不接</MenuItem>
                          {m.exits.map((x) => (
                            <MenuItem key={x.id} value={x.id}>
                              {x.name}
                            </MenuItem>
                          ))}
                          <MenuItem value={NEW_EXIT}>+ 新建出口…</MenuItem>
                        </Select>
                      </TableCell>
                      <TableCell>{currentNode(m, exit, groupNow.data)}</TableCell>
                      <TableCell>{exit ? iface : "—"}</TableCell>
                      <TableCell>
                        <DelayText result={d} />
                      </TableCell>
                      <TableCell>
                        ↑ {formatRate(rate?.up ?? 0)} ↓ {formatRate(rate?.down ?? 0)}
                      </TableCell>
                      <TableCell>
                        ↑ {formatBytes(total?.up ?? 0)} ↓ {formatBytes(total?.down ?? 0)}
                      </TableCell>
                      <TableCell>
                        <RowActions
                          label={l.name}
                          items={[
                            { label: "编辑", onClick: () => setEditing(l) },
                            { label: "删除", onClick: () => setDeleting(l), danger: true },
                          ]}
                        />
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </TableContainer>
        )}
      </Stack>
      {creating && <ListenerDialog key={creating.id} listener={creating} onClose={() => setCreating(null)} />}
      {confirmReset && (
        <Dialog open onClose={() => setConfirmReset(false)} maxWidth="xs">
          <DialogTitle>清零累计流量？</DialogTitle>
          <DialogContent>所有监听器的累计流量都会归零，无法撤销。</DialogContent>
          <DialogActions>
            <Button onClick={() => setConfirmReset(false)}>取消</Button>
            <Button
              color="error"
              variant="contained"
              onClick={() => {
                reset.mutate();
                setConfirmReset(false);
              }}
            >
              清零
            </Button>
          </DialogActions>
        </Dialog>
      )}
      {editing && <ListenerDialog key={editing.id} listener={editing} onClose={() => setEditing(null)} />}
      {deleting && (
        <ConfirmDeleteDialog
          text={`确定删除监听器「${deleting.name}」吗？`}
          onConfirm={() => delListener.mutateAsync(deleting.id)}
          onClose={() => setDeleting(null)}
        />
      )}
      {creatingExit && <ExitDialog key={creatingExit.id} exit={creatingExit} onClose={() => setCreatingExit(null)} />}
      {newExitFor && (
        <ExitDialog
          key={newExitFor.id}
          exit={newExit()}
          onClose={() => setNewExitFor(null)}
          onSaved={(saved) => saveListener.mutate({ ...newExitFor, exit: saved.id })}
        />
      )}
    </Stack>
  );
}

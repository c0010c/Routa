import {
  Alert,
  Box,
  Button,
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
} from "@mui/material";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { useSearchParams } from "react-router-dom";
import {
  useCoreControl,
  useGroupNow,
  useModel,
  useResetTraffic,
  useSaveListener,
  useSelectGroupMember,
  useStatus,
  useTraffic,
} from "../api/client";
import type { ApplyOutcome, CoreState, Exit, Listener, Model, RuntimeStatus } from "../bindings";
import { expandMembers, nodeKey, nodeLabel } from "../lib/refs";
import { ListenerStatusChip } from "../components/StatusChips";
import { ExitDialog, newExit } from "./ExitDialog";
import { ListenerDialog, newListener } from "./ListenerDialog";
import { exitStatusLabel, formatBytes, formatRate, listenerNotEffective } from "../lib/format";


const TYPE_TEXT = { Mixed: "混合", Http: "HTTP", Socks: "SOCKS" } as const;

function coreText(c: CoreState): string {
  if (c === "Stopped") return "已停止";
  if (c === "Starting") return "启动中";
  if (c === "Running") return "运行中";
  return `已崩溃（退出码 ${c.Crashed.exit_code ?? "未知"}）`;
}

function applyError(a: Exclude<ApplyOutcome, { Reloaded: unknown }>): string {
  if ("FailedKeptPrevious" in a) return a.FailedKeptPrevious.error;
  if ("FailedClosed" in a) return a.FailedClosed.error;
  if ("FailedCoreStopped" in a) return a.FailedCoreStopped.error;
  return a.NotRunning.error;
}

export function collectProblems(model: Model, status: RuntimeStatus): string[] {
  const out: string[] = [];
  if (status.core === "Stopped" || typeof status.core === "object") out.push(`mihomo ${coreText(status.core)}`);
  const a = status.last_apply;
  if (a !== null && !("Reloaded" in a)) out.push(`配置未生效：${applyError(a)}`);
  for (const [id, h] of Object.entries(status.health.exits)) {
    if (!h || h.status === "Ok") continue;
    const name = model.exits.find((e) => e.id === id)?.name ?? id;
    out.push(`出口「${name}」${exitStatusLabel(h.status).text}`);
  }
  for (const f of status.failed_listeners) {
    out.push(`Listener「${f.name}」端口 ${f.port} 被占用`);
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

function CoreControls({ core }: { core: CoreState }) {
  const c = useCoreControl();
  const crashed = typeof core === "object";
  const canStart = core === "Stopped" || crashed;
  const canStop = core === "Running";
  const canRestart = core === "Running" || crashed;
  return (
    <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
      <Typography>mihomo：{coreText(core)}</Typography>
      <Button variant="contained" size="small" disabled={!canStart || c.isPending} onClick={() => c.start.mutate()}>
        启动
      </Button>
      <Button variant="outlined" size="small" disabled={!canStop || c.isPending} onClick={() => c.stop.mutate()}>
        停止
      </Button>
      <Button variant="outlined" size="small" disabled={!canRestart || c.isPending} onClick={() => c.restart.mutate()}>
        重启
      </Button>
    </Stack>
  );
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

export function Overview() {
  const model = useModel();
  const status = useStatus();
  const liveTick = useTraffic();
  const reset = useResetTraffic();
  const saveListener = useSaveListener();
  const [params] = useSearchParams();
  const paramHighlight = params.get("highlight");
  const [highlight, setHighlight] = useState<string | null>(null);
  const [creating, setCreating] = useState<Listener | null>(null);
  const [newExitFor, setNewExitFor] = useState<Listener | null>(null);
  const highlightRef = useRef<HTMLTableRowElement | null>(null);
  const groupNames = (model.data?.exits ?? [])
    .filter((e) => typeof e.kind === "object" && "Group" in e.kind)
    .map((e) => e.name);
  const groupNow = useGroupNow(groupNames);
  const loaded = !!model.data && !!status.data;

  useEffect(() => {
    if (!paramHighlight) return;
    setHighlight(paramHighlight);
    const t = setTimeout(() => setHighlight(null), 3000);
    return () => clearTimeout(t);
  }, [paramHighlight]);

  useEffect(() => {
    if (highlight && loaded) highlightRef.current?.scrollIntoView?.({ block: "center" });
  }, [highlight, loaded]);

  if (!model.data || !status.data) return <Typography>加载中…</Typography>;
  const m = model.data;
  const s = status.data;
  const tick = liveTick ?? s.traffic;
  const problems = collectProblems(m, s);
  const failedIds = new Set(s.failed_listeners.map((f) => f.id));

  return (
    <Stack spacing={2}>
      {problems.length > 0 && (
        <Alert severity="error" role="alert">
          {problems.map((p) => (
            <div key={p}>{p}</div>
          ))}
        </Alert>
      )}
      <CoreControls core={s.core} />
      <Stack direction="row" spacing={3} sx={{ alignItems: "center" }}>
        <Typography>
          实时 ↑ {formatRate(tick?.total_rate.up ?? 0)} ↓ {formatRate(tick?.total_rate.down ?? 0)}
        </Typography>
        <Typography>
          累计 ↑ {formatBytes(tick?.totals.total.up ?? 0)} ↓ {formatBytes(tick?.totals.total.down ?? 0)}
        </Typography>
        <Button size="small" variant="outlined" onClick={() => reset.mutate()}>
          清零
        </Button>
      </Stack>
      <TableContainer component={Paper}>
        <Table size="small">
          <TableHead>
            <TableRow>
              {["状态", "监听器", "出口", "当前节点", "网卡", "延迟", "实时 ↑↓", "累计"].map((h) => (
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
                      onChange={(e) => {
                        const v = e.target.value;
                        if (v === NEW_EXIT) setNewExitFor(l);
                        else saveListener.mutate({ ...l, exit: v === "" ? null : v });
                      }}
                      inputProps={{ "aria-label": `${l.name} 出口` }}
                      sx={{ minWidth: 140 }}
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
                  <TableCell>{!d ? "—" : d.ms === null ? "超时" : `${d.ms} ms`}</TableCell>
                  <TableCell>
                    ↑ {formatRate(rate?.up ?? 0)} ↓ {formatRate(rate?.down ?? 0)}
                  </TableCell>
                  <TableCell>
                    ↑ {formatBytes(total?.up ?? 0)} ↓ {formatBytes(total?.down ?? 0)}
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </TableContainer>
      <Box>
        <Button variant="outlined" size="small" onClick={() => setCreating(newListener(m.listeners))}>
          + 新建监听器
        </Button>
      </Box>
      {creating && <ListenerDialog key={creating.id} listener={creating} onClose={() => setCreating(null)} />}
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

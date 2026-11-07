import AppsOutlined from "@mui/icons-material/AppsOutlined";
import FolderOutlined from "@mui/icons-material/FolderOutlined";
import {
  Alert,
  Box,
  Button,
  Chip,
  Drawer,
  Paper,
  Stack,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Tooltip,
  Typography,
  TextField,
} from "@mui/material";
import { useState } from "react";
import { useModel } from "../api/client";
import {
  useDeleteProcessRule,
  useFlows,
  useProcRouteInstall,
  useProcRouteStatus,
  useProcRouteRetry,
  useRemoveLearnedPath,
  useRunningProcesses,
  useSaveProcessRoutingSettings,
  useDisableRoutingAndQuit,
} from "../api/procroute";
import type {
  ComponentState,
  ProcessRule,
  ProcRouteStatus,
  RuleView,
} from "../bindings";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { EmptyState } from "../components/EmptyState";
import { LearnedLocations } from "../components/LearnedLocations";
import { RowActions } from "../components/RowActions";
import { formatBytes, type Label } from "../lib/format";
import { chipLabel, fileName, groupsOf, learningNames, ruleLabel, type ProcessGroup } from "../lib/processRule";
import { ProcessRuleDialog } from "./ProcessRuleDialog";
import { ProcessNodeSelect } from "../components/ProcessNodeSelect";
import { desktopApi, useDesktopMutation } from "../api/desktop";
import { errorMessage } from "../api/client";

export const TRUNCATED_HINT = "仅显示最近 400 条";

/** 列表里每条规则最多直接显示几个进程，其余折成「+N」。 */
const MAX_CHIPS = 4;

const DISMISSED_INTERRUPTION_KEY =
  "routa.dismissed-process-routing-interruption";

function componentLabel(c: ComponentState): Label {
  if (typeof c === "object")
    return { text: `错误：${c.Error.message}`, color: "error" };
  switch (c) {
    case "NotInstalled":
      return { text: "未安装", color: "default" };
    case "Stopped":
      return { text: "已停止", color: "warning" };
    case "Running":
      return { text: "运行中", color: "success" };
    case "NeedsUpdate":
      return { text: "需要更新", color: "warning" };
    case "OwnedByOtherUser":
      return { text: "组件被其他用户占用", color: "error" };
    default: {
      const never: never = c;
      return never;
    }
  }
}

function actionLabel(v: RuleView): Label {
  switch (v.action) {
    case "Proxy":
      return { text: "已代理", color: "success" };
    case "Direct":
      return { text: "已直连", color: "warning" };
    case "Block":
      return { text: "已阻断", color: "error" };
    case "Pending":
      return { text: "应用中", color: "default" };
    case "Failed":
      return { text: "应用失败", color: "error" };
    case "Unknown":
      return { text: "状态未知", color: "warning" };
    default:
      return { text: v.action, color: "default" };
  }
}

function formatDuration(ms: number | null): string {
  if (ms === null) return "—";
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s} 秒`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} 分 ${s % 60} 秒`;
  return `${Math.floor(m / 60)} 时 ${m % 60} 分`;
}

function formatTime(unix: number): string {
  return new Date(unix * 1000).toLocaleString();
}

/** 标题旁的组件状态和总开关：规则不生效时用户先在这一页找原因。 */
function ComponentStatus({
  status,
  enabled,
}: {
  status: ProcRouteStatus;
  enabled: boolean;
}) {
  const settings = useSaveProcessRoutingSettings();
  const quit = useDisableRoutingAndQuit();
  const c = status.component;
  const label = componentLabel(c);
  return (
    <Stack direction="row" spacing={1.5} sx={{ alignItems: "center", minWidth: 0 }}>
      <Chip
        size="small"
        label={label.text}
        color={label.color}
        variant="outlined"
        data-selectable={typeof c === "object"}
        title="分流组件状态"
      />
      {c !== "NotInstalled" && (
        <Stack direction="row" sx={{ alignItems: "center" }}>
          <Switch
            checked={enabled}
            disabled={settings.isPending}
            onChange={(e) => settings.mutate(e.target.checked)}
            slotProps={{ input: { "aria-label": "启用进程分流规则" } }}
          />
          <Typography variant="body2" color="text.secondary">
            {enabled ? "已启用" : "已暂停"}
          </Typography>
          {enabled && (
            <Button
              size="small"
              sx={{ ml: 1 }}
              disabled={settings.isPending || quit.isPending}
              onClick={() => quit.mutate(false)}
            >
              停用并退出
            </Button>
          )}
        </Stack>
      )}
    </Stack>
  );
}

/** 只在需要用户处理时出现的提示：安装/更新/修复组件、已暂停、中断、冲突、应用失败。 */
function ComponentAlerts({
  status,
  enabled,
}: {
  status: ProcRouteStatus;
  enabled: boolean;
}) {
  const install = useProcRouteInstall();
  const retry = useProcRouteRetry();
  const settings = useSaveProcessRoutingSettings();
  const [dismissedInterruption, setDismissedInterruption] = useState<
    string | null
  >(() => localStorage.getItem(DISMISSED_INTERRUPTION_KEY));
  const interruption = status.last_interruption;
  const interruptionKey = interruption
    ? JSON.stringify([interruption.at_unix, interruption.reason])
    : null;
  const c = status.component;
  const busy = install.isPending || retry.isPending;
  const installAction = (text: string) => (
    <Button color="inherit" loading={install.isPending} disabled={busy} onClick={() => install.mutate()}>
      {text}
    </Button>
  );

  return (
    <>
      {c === "NotInstalled" && (
        <Alert severity="info" action={installAction("启用进程分流")}>
          进程分流需要先安装分流组件（需要管理员授权），装好后规则才会生效。
        </Alert>
      )}
      {c === "NeedsUpdate" && (
        <Alert severity="warning" action={installAction("更新分流服务")}>
          分流服务与当前程序版本不匹配。更新需要管理员权限，期间进程分流会短暂中断。
        </Alert>
      )}
      {(c === "Stopped" || typeof c === "object") && (
        <Alert severity="warning" action={installAction("修复分流服务")}>
          分流服务没有正常运行，规则不会生效。
        </Alert>
      )}
      {c !== "NotInstalled" && !enabled && (
        <Alert
          severity="info"
          action={
            <Button color="inherit" disabled={settings.isPending} onClick={() => settings.mutate(true)}>
              恢复
            </Button>
          }
        >
          进程分流已暂停，下面的规则都不生效。
        </Alert>
      )}
      {interruption &&
        interruptionKey &&
        interruptionKey !== dismissedInterruption && (
          <Alert
            severity="warning"
            closeText="关闭中断提示"
            onClose={() => {
              setDismissedInterruption(interruptionKey);
              localStorage.setItem(DISMISSED_INTERRUPTION_KEY, interruptionKey);
            }}
          >
            {`最近一次中断：${formatTime(interruption.at_unix)}，${interruption.reason}`}
          </Alert>
        )}
      {status.conflicts.length > 0 && (
        <Alert severity="warning">
          {status.conflicts.map((m) => (
            <div key={m}>{m}</div>
          ))}
        </Alert>
      )}
      {status.apply_errors.length > 0 && (
        <Alert
          severity="warning"
          action={
            <Button
              color="inherit"
              loading={retry.isPending}
              disabled={busy || c === "NeedsUpdate" || c === "NotInstalled" || c === "OwnedByOtherUser"}
              onClick={() => retry.mutate()}
            >
              重试
            </Button>
          }
        >
          {status.apply_errors.map((m) => (
            <div key={m}>{m}</div>
          ))}
        </Alert>
      )}
    </>
  );
}

function GuardChips({
  view,
  active,
}: {
  view: RuleView | undefined;
  active: boolean;
}) {
  if (!active) return <Chip size="small" label="未启用" />;
  if (!view) return <Chip size="small" label="状态未知" color="warning" />;
  const l = actionLabel(view);
  return (
    <Stack direction="row" spacing={0.5} sx={{ flexWrap: "wrap" }}>
      <Chip size="small" label={l.text} color={l.color} />
      {view.reason && (
        <Chip size="small" variant="outlined" label={view.reason} data-selectable="true" />
      )}
    </Stack>
  );
}

/** 规则里的进程：一个程序一个标签，悬停看匹配方式和位置；多出的折成「+N」。 */
function ProcessChips({ rule }: { rule: ProcessRule }) {
  const groups = groupsOf(rule.processes);
  const extra = groups.slice(MAX_CHIPS);
  const tip = (g: ProcessGroup) => [...(g.hasName ? ["所有位置（按进程名）"] : []), ...g.paths].join("\n");
  return (
    <Stack direction="row" sx={{ flexWrap: "wrap", gap: 0.5, mt: 0.5 }}>
      {groups.slice(0, MAX_CHIPS).map((g) => (
        <Tooltip key={g.key} title={<Box sx={{ whiteSpace: "pre-line", wordBreak: "break-all" }}>{tip(g)}</Box>}>
          <Chip
            size="small"
            icon={g.paths.length ? <FolderOutlined /> : undefined}
            label={chipLabel(g)}
            sx={{ borderRadius: 1, height: 22 }}
          />
        </Tooltip>
      ))}
      {extra.length > 0 && (
        <Tooltip title={<Box sx={{ whiteSpace: "pre-line" }}>{extra.map(chipLabel).join("\n")}</Box>}>
          <Chip size="small" label={`+${extra.length}`} sx={{ borderRadius: 1, height: 22 }} />
        </Tooltip>
      )}
    </Stack>
  );
}

function FlowsDrawer({
  rule,
  truncated,
  onClose,
}: {
  rule: ProcessRule | null;
  truncated: boolean;
  onClose: () => void;
}) {
  const flows = useFlows();
  const rows = rule ? flows.filter((f) => f.rule_id === rule.id) : [];
  return (
    <Drawer anchor="right" open={rule !== null} onClose={onClose}>
      {rule && (
        <Box sx={{ width: 680, p: 2 }}>
          <Stack
            direction="row"
            spacing={1}
            sx={{ alignItems: "baseline", mb: 1 }}
          >
            <Typography variant="h6">{ruleLabel(rule)} 的连接</Typography>
            {truncated && (
              <Typography variant="caption" sx={{ color: "warning.main" }}>
                {TRUNCATED_HINT}
              </Typography>
            )}
          </Stack>
          <Typography variant="caption" sx={{ color: "text.secondary" }}>
            只列出正在被代理的连接，直连和被阻断的连接看不到。
          </Typography>
          <Table size="small">
            <TableHead>
              <TableRow>
                {[
                  "进程",
                  "PID",
                  "目标",
                  "端口",
                  "协议",
                  "持续时间",
                  "已上传",
                  "已下载",
                ].map((h) => (
                  <TableCell key={h}>{h}</TableCell>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {rows.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={8} sx={{ color: "text.secondary" }}>
                    当前没有连接
                  </TableCell>
                </TableRow>
              ) : (
                rows.map((f) => (
                  <TableRow
                    key={`${f.pid}-${f.host}-${f.port}-${f.udp ? "u" : "t"}-${f.age_ms ?? ""}`}
                  >
                    <TableCell>
                      <Tooltip title={f.exe_path ?? ""}>
                        <span>{f.exe_path ? fileName(f.exe_path) : "—"}</span>
                      </Tooltip>
                    </TableCell>
                    <TableCell data-selectable="true">{f.pid}</TableCell>
                    <TableCell data-selectable="true" sx={{ wordBreak: "break-all" }}>
                      {f.host}
                    </TableCell>
                    <TableCell data-selectable="true">{f.port}</TableCell>
                    <TableCell>{f.udp ? "UDP" : "TCP"}</TableCell>
                    <TableCell>{formatDuration(f.age_ms)}</TableCell>
                    <TableCell>{formatBytes(f.sent)}</TableCell>
                    <TableCell>{formatBytes(f.received)}</TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </Box>
      )}
    </Drawer>
  );
}

export function ProcessRoutingPage() {
  const model = useModel();
  const status = useProcRouteStatus();
  const setEnabled = useDesktopMutation(
    (a: { id: string; enabled: boolean }) =>
      desktopApi.setRuleEnabled(a.id, a.enabled),
  );
  const [togglingIds, setTogglingIds] = useState<string[]>([]);
  const [toggleErrors, setToggleErrors] = useState<Record<string, string>>({});
  const toggle = async (id: string, enabled: boolean) => {
    if (togglingIds.includes(id)) return;
    setTogglingIds((v) => [...v, id]);
    setToggleErrors(({ [id]: _, ...rest }) => rest);
    try {
      await setEnabled.mutateAsync({ id, enabled });
    } catch (e) {
      setToggleErrors((v) => ({ ...v, [id]: errorMessage(e) }));
    } finally {
      setTogglingIds((v) => v.filter((x) => x !== id));
    }
  };
  const delRule = useDeleteProcessRule();
  const removeLearned = useRemoveLearnedPath();
  const running = useRunningProcesses();
  const [editing, setEditing] = useState<{ rule?: ProcessRule } | null>(null);
  const [deleting, setDeleting] = useState<ProcessRule | null>(null);
  const [viewing, setViewing] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  if (!model.data || !status.data) return <Typography>加载中…</Typography>;
  const m = model.data;
  const s = status.data;
  const pr = m.process_routing;
  const viewingRule = pr.rules.find((r) => r.id === viewing) ?? null;
  const q = search.trim().toLowerCase();
  const shown = pr.rules.filter(
    (r) =>
      !q ||
      ruleLabel(r).toLowerCase().includes(q) ||
      r.processes.some((p) => p.value.toLowerCase().includes(q)),
  );

  return (
    <Stack spacing={2}>
      <Stack direction="row" spacing={2} sx={{ justifyContent: "space-between", alignItems: "center" }}>
        <Stack direction="row" spacing={2} sx={{ alignItems: "center", minWidth: 0 }}>
          <Typography variant="h5">进程分流</Typography>
          <ComponentStatus status={s} enabled={pr.enabled} />
        </Stack>
        <Button variant="contained" onClick={() => setEditing({})}>
          新建规则
        </Button>
      </Stack>
      <ComponentAlerts status={s} enabled={pr.enabled} />
      <TextField
        size="small"
        label="搜索规则名称或进程"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        sx={{ maxWidth: 360 }}
      />
      {pr.rules.length === 0 ? (
        <EmptyState
          icon={<AppsOutlined />}
          title="还没有分流规则"
          description="规则让指定程序的流量走某个节点，程序本身不需要设置代理。"
        />
      ) : (
        <TableContainer component={Paper}>
          <Table size="small">
            <TableHead>
              <TableRow>
                {["规则", "节点", "状态", "启用", ""].map((h) => (
                  <TableCell key={h}>{h}</TableCell>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {shown.length === 0 && (
                <TableRow>
                  <TableCell colSpan={5} sx={{ color: "text.secondary" }}>
                    没有匹配的规则
                  </TableCell>
                </TableRow>
              )}
              {shown.map((r) => {
                const view = s.actions[r.id];
                const name = ruleLabel(r);
                return (
                  <TableRow
                    key={r.id}
                    hover
                    sx={{ cursor: "pointer" }}
                    onClick={() => setViewing(r.id)}
                  >
                    <TableCell sx={{ width: "42%" }}>
                      <Typography variant="body2" sx={{ fontWeight: 600 }}>
                        {name}
                        {r.user_enabled && r.on_fault === "Block" && (
                          <Typography component="span" variant="caption" sx={{ ml: 1, fontWeight: 400, color: "text.secondary" }}>
                            不可用时阻断
                          </Typography>
                        )}
                      </Typography>
                      <ProcessChips rule={r} />
                      {learningNames(r).length > 0 && (
                        <Box sx={{ mt: 0.5 }} onClick={(e) => e.stopPropagation()}>
                          <LearnedLocations
                            rule={r}
                            others={pr.rules.filter((x) => x.id !== r.id)}
                            routingEnabled={pr.enabled}
                            running={running.data ?? []}
                            onRemove={(p) => removeLearned.mutate(r.id, p)}
                          />
                        </Box>
                      )}
                    </TableCell>
                    <TableCell>
                      <ProcessNodeSelect rule={r} model={m} reason={view?.reason} />
                    </TableCell>
                    <TableCell>
                      <GuardChips view={view} active={pr.enabled && r.user_enabled} />
                      {toggleErrors[r.id] && (
                        <Typography role="alert" variant="caption" color="error" sx={{ display: "block" }}>
                          {toggleErrors[r.id]}
                        </Typography>
                      )}
                    </TableCell>
                    <TableCell onClick={(e) => e.stopPropagation()}>
                      <Switch
                        checked={r.user_enabled}
                        disabled={togglingIds.includes(r.id)}
                        slotProps={{
                          input: { "aria-label": `启用 ${name}` },
                        }}
                        onChange={(_, checked) =>
                          void toggle(r.id, checked)
                        }
                      />
                    </TableCell>
                    <TableCell onClick={(e) => e.stopPropagation()}>
                      <RowActions
                        label={name}
                        items={[
                          {
                            label: "编辑",
                            onClick: () => setEditing({ rule: r }),
                          },
                          {
                            label: r.user_enabled ? "禁用" : "启用",
                            disabled: togglingIds.includes(r.id),
                            onClick: () => void toggle(r.id, !r.user_enabled),
                          },
                          {
                            label: "删除",
                            onClick: () => setDeleting(r),
                            danger: true,
                          },
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
      <Box component="details">
        <Box
          component="summary"
          sx={{ cursor: "pointer", fontSize: 12, color: "text.secondary" }}
        >
          分流说明与连接行为
        </Box>
        <Typography variant="body2" sx={{ color: "text.secondary", mt: 1 }}>
          DNS 查询不经过分流：程序的域名解析由系统的 DNS
          客户端服务代为发出，不属于该程序，会照常直连。
          <br />
          节点可直接在行内更改，换节点后该程序的现有连接会断开重连。
          <br />
          卸载组件和清理残留阻断规则在「设置 → 进程分流」中。
        </Typography>
      </Box>
      <ProcessRuleDialog
        key={editing?.rule?.id ?? "new"}
        open={editing !== null}
        rule={editing?.rule}
        onClose={() => setEditing(null)}
      />
      <FlowsDrawer
        rule={viewingRule}
        truncated={s.truncated_flows}
        onClose={() => setViewing(null)}
      />
      {deleting && (
        <ConfirmDeleteDialog
          text={`确定删除规则「${ruleLabel(deleting)}」吗？它包含的 ${deleting.processes.length} 个进程都将不再分流。`}
          onConfirm={() => delRule.mutateAsync(deleting.id)}
          onClose={() => setDeleting(null)}
        />
      )}
    </Stack>
  );
}

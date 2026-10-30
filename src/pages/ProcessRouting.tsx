import AppsOutlined from "@mui/icons-material/AppsOutlined";
import InsertDriveFileOutlined from "@mui/icons-material/InsertDriveFileOutlined";
import MoreVert from "@mui/icons-material/MoreVert";
import {
  Alert,
  Box,
  Button,
  Chip,
  Drawer,
  FormControlLabel,
  FormHelperText,
  FormLabel,
  IconButton,
  Menu,
  MenuItem,
  Paper,
  Radio,
  RadioGroup,
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
  useProcRouteCleanup,
  useProcRouteInstall,
  useProcRouteStatus,
  useProcRouteRetry,
  useProcRouteUninstall,
  useSaveProcessRoutingSettings,
  useSaveProcessRule,
} from "../api/procroute";
import type {
  ComponentState,
  ExitPolicy,
  ProcessRule,
  ProcRouteStatus,
  RuleView,
} from "../bindings";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { EmptyState } from "../components/EmptyState";
import { RowActions } from "../components/RowActions";
import { formatBytes, type Label } from "../lib/format";
import { ProcessRuleDialog } from "./ProcessRuleDialog";
import { ProcessListenerSelect } from "../components/ProcessListenerSelect";

export const EXIT_POLICY_LABEL = "Routa 不提供代理时";
export const EXIT_POLICY_HINT =
  "适用于退出 Routa、手动停止内核、开机后 Routa 启动前；Routa 或组件崩溃时按每条规则自己的设置";
export const TRUNCATED_HINT = "仅显示最近 400 条";

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

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
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

type Pending = "uninstall" | "cleanup" | null;

function ComponentCard({
  status,
  enabled,
  onExit,
}: {
  status: ProcRouteStatus;
  enabled: boolean;
  onExit: ExitPolicy;
}) {
  const install = useProcRouteInstall();
  const uninstall = useProcRouteUninstall();
  const cleanup = useProcRouteCleanup();
  const retry = useProcRouteRetry();
  const settings = useSaveProcessRoutingSettings();
  const [menu, setMenu] = useState<HTMLElement | null>(null);
  const [confirm, setConfirm] = useState<Pending>(null);
  const [dismissedInterruption, setDismissedInterruption] = useState<
    string | null
  >(() => localStorage.getItem(DISMISSED_INTERRUPTION_KEY));
  const interruption = status.last_interruption;
  const interruptionKey = interruption
    ? JSON.stringify([interruption.at_unix, interruption.reason])
    : null;
  const c = status.component;
  const label = componentLabel(c);
  const installText =
    c === "NotInstalled"
      ? "启用进程分流"
      : c === "NeedsUpdate"
        ? "更新分流服务"
        : c === "Stopped" || typeof c === "object"
          ? "修复分流服务"
          : null;
  const busy =
    install.isPending ||
    uninstall.isPending ||
    cleanup.isPending ||
    retry.isPending;

  return (
    <Paper sx={{ p: 2 }}>
      <Stack spacing={2}>
        <Stack direction="row" spacing={1.5} sx={{ alignItems: "center" }}>
          <Typography sx={{ fontWeight: 600 }}>分流组件</Typography>
          <Chip size="small" label={label.text} color={label.color} data-selectable={typeof c === "object"} />
          <Box sx={{ flexGrow: 1 }} />
          {installText && (
            <Button
              variant="contained"
              loading={install.isPending}
              disabled={busy}
              onClick={() => install.mutate()}
            >
              {installText}
            </Button>
          )}
          <IconButton
            aria-label="更多组件操作"
            disabled={busy}
            onClick={(e) => setMenu(e.currentTarget)}
          >
            <MoreVert />
          </IconButton>
          <Menu anchorEl={menu} open={!!menu} onClose={() => setMenu(null)}>
            <MenuItem
              disabled={c === "NotInstalled"}
              onClick={() => {
                setMenu(null);
                setConfirm("uninstall");
              }}
            >
              卸载组件
            </MenuItem>
            <MenuItem
              onClick={() => {
                setMenu(null);
                setConfirm("cleanup");
              }}
            >
              清理残留阻断规则
            </MenuItem>
          </Menu>
        </Stack>
        {c === "NeedsUpdate" && (
          <Alert severity="warning">
            分流服务与当前程序版本不匹配。更新需要管理员权限，期间进程分流会短暂中断。
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
                localStorage.setItem(
                  DISMISSED_INTERRUPTION_KEY,
                  interruptionKey,
                );
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
                disabled={
                  busy ||
                  c === "NeedsUpdate" ||
                  c === "NotInstalled" ||
                  c === "OwnedByOtherUser"
                }
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
        <Stack
          direction="row"
          sx={{ alignItems: "center", alignSelf: "flex-start" }}
        >
          <Switch
            checked={enabled}
            disabled={settings.isPending}
            onChange={(e) => settings.mutate(e.target.checked, onExit)}
            slotProps={{ input: { "aria-labelledby": "pr-enabled-label" } }}
            sx={{ ml: "-11px" }}
          />
          <Typography component="span" id="pr-enabled-label">
            启用进程分流规则
          </Typography>
        </Stack>
        <Box>
          <Box component="details">
            <Box
              component="summary"
              sx={{
                cursor: "pointer",
                color: "text.secondary",
                fontSize: 13,
                mb: 1,
              }}
            >
              退出行为与故障策略
            </Box>
            <FormLabel id="pr-exit">{EXIT_POLICY_LABEL}</FormLabel>
            <RadioGroup
              row
              aria-labelledby="pr-exit"
              value={onExit}
              onChange={(e) =>
                settings.mutate(enabled, e.target.value as ExitPolicy)
              }
            >
              <FormControlLabel
                value="Direct"
                control={<Radio size="small" />}
                label="直连"
                disabled={settings.isPending}
              />
              <FormControlLabel
                value="Block"
                control={<Radio size="small" />}
                label="阻断"
                disabled={settings.isPending}
              />
            </RadioGroup>
            <FormHelperText sx={{ mx: 0 }}>{EXIT_POLICY_HINT}</FormHelperText>
          </Box>
        </Box>
      </Stack>
      {confirm === "uninstall" && (
        <ConfirmDeleteDialog
          title="卸载组件"
          confirmLabel="卸载"
          text="将停止并删除分流服务，并清除它的所有阻断规则。需要管理员授权。"
          onConfirm={() => uninstall.mutateAsync()}
          onClose={() => setConfirm(null)}
        />
      )}
      {confirm === "cleanup" && (
        <ConfirmDeleteDialog
          title="清理残留阻断规则"
          confirmLabel="清理"
          text="将删除系统中所有 Routa 留下的阻断规则，被阻断的程序会直接联网。服务未运行时需要管理员授权。"
          onConfirm={() => cleanup.mutateAsync()}
          onClose={() => setConfirm(null)}
        />
      )}
    </Paper>
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
        <Box sx={{ width: 640, p: 2 }}>
          <Stack
            direction="row"
            spacing={1}
            sx={{ alignItems: "baseline", mb: 1 }}
          >
            <Typography variant="h6">
              {fileName(rule.exe_path)} 的连接
            </Typography>
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
                  <TableCell colSpan={7} sx={{ color: "text.secondary" }}>
                    当前没有连接
                  </TableCell>
                </TableRow>
              ) : (
                rows.map((f) => (
                  <TableRow
                    key={`${f.pid}-${f.host}-${f.port}-${f.udp ? "u" : "t"}-${f.age_ms ?? ""}`}
                  >
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
  const saveRule = useSaveProcessRule();
  const delRule = useDeleteProcessRule();
  const [editing, setEditing] = useState<{ rule?: ProcessRule } | null>(null);
  const [deleting, setDeleting] = useState<ProcessRule | null>(null);
  const [viewing, setViewing] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  if (!model.data || !status.data) return <Typography>加载中…</Typography>;
  const m = model.data;
  const s = status.data;
  const pr = m.process_routing;
  const viewingRule = pr.rules.find((r) => r.id === viewing) ?? null;

  return (
    <Stack spacing={2}>
      <Stack direction="row" sx={{ justifyContent: "space-between" }}>
        <Typography variant="h5">进程分流</Typography>
        <Button variant="contained" onClick={() => setEditing({})}>
          新建规则
        </Button>
      </Stack>
      <ComponentCard
        status={s}
        enabled={pr.enabled}
        onExit={pr.on_routa_exit}
      />
      <TextField
        size="small"
        label="搜索进程名称或路径"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        sx={{ maxWidth: 360 }}
      />
      {pr.rules.length === 0 ? (
        <EmptyState
          icon={<AppsOutlined />}
          title="还没有分流规则"
          description="规则让指定程序的流量走某个监听器，程序本身不需要设置代理。"
        />
      ) : (
        <TableContainer component={Paper}>
          <Table size="small">
            <TableHead>
              <TableRow>
                {["进程", "监听器", "状态", "启用", ""].map((h) => (
                  <TableCell key={h}>{h}</TableCell>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {pr.rules
                .filter((r) =>
                  r.exe_path.toLowerCase().includes(search.toLowerCase()),
                )
                .map((r) => {
                  const view = s.actions[r.id];
                  const name = fileName(r.exe_path);
                  return (
                    <TableRow
                      key={r.id}
                      hover
                      sx={{ cursor: "pointer" }}
                      onClick={() => setViewing(r.id)}
                    >
                      <TableCell>
                        <Stack
                          direction="row"
                          spacing={1}
                          sx={{ alignItems: "center" }}
                        >
                          <InsertDriveFileOutlined
                            fontSize="small"
                            sx={{ color: "text.secondary" }}
                          />
                          <Tooltip title={r.exe_path}>
                            <span>{name}</span>
                          </Tooltip>
                        </Stack>
                      </TableCell>
                      <TableCell>
                        <ProcessListenerSelect
                          rule={r}
                          listeners={m.listeners}
                        />
                      </TableCell>
                      <TableCell>
                        <GuardChips
                          view={view}
                          active={pr.enabled && r.user_enabled}
                        />
                      </TableCell>
                      <TableCell onClick={(e) => e.stopPropagation()}>
                        <Switch
                          checked={r.user_enabled}
                          disabled={saveRule.isPending}
                          slotProps={{
                            input: { "aria-label": `启用 ${name}` },
                          }}
                          onChange={(_, checked) =>
                            saveRule.mutate({ ...r, user_enabled: checked })
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
                              onClick: () =>
                                saveRule.mutate({
                                  ...r,
                                  user_enabled: !r.user_enabled,
                                }),
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
          HTTP 类型的监听器不能选：它转发不了 UDP，程序的 UDP
          流量会悄悄走直连。请选混合或 SOCKS 类型。
          监听器可直接在行内更改，后续新连接使用新目标，不强制断开已有连接。
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
          text={`确定删除「${fileName(deleting.exe_path)}」的分流规则吗？`}
          onConfirm={() => delRule.mutateAsync(deleting.id)}
          onClose={() => setDeleting(null)}
        />
      )}
    </Stack>
  );
}

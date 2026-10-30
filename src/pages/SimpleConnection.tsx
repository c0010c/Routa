import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  Alert,
  Box,
  Button,
  Chip,
  CircularProgress,
  Collapse,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  IconButton,
  List,
  ListItemButton,
  ListItemText,
  Paper,
  Radio,
  Stack,
  Switch,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  Typography,
} from "@mui/material";
import RouteOutlined from "@mui/icons-material/RouteOutlined";
import LanguageOutlined from "@mui/icons-material/LanguageOutlined";
import ChevronRightOutlined from "@mui/icons-material/ChevronRightOutlined";
import InfoOutlined from "@mui/icons-material/InfoOutlined";
import AppsOutlined from "@mui/icons-material/AppsOutlined";
import {
  desktopApi,
  useDesktopMutation,
  type CaptureMode,
} from "../api/desktop";
import {
  errorMessage,
  useModel,
  useStatus,
  useTraffic,
  useTestDelays,
} from "../api/client";
import {
  useProcRouteInstall,
  useProcRouteStatus,
  useRunningProcesses,
} from "../api/procroute";
import { useInterface } from "../components/InterfaceProvider";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { FormDialog } from "../components/FormDialog";
import { NodePicker } from "../components/NodePicker";
import { SubscriptionDialog, newSubscription } from "./SubscriptionDialog";
import { ManualNodeDialog, newManualNode } from "./ManualNodeDialog";
import { nodeLabel, proxyNameOf } from "../lib/refs";
import { delayText, formatRate } from "../lib/format";
import type { NodeRef } from "../bindings";

export function SimpleConnection() {
  const ui = useInterface()!;
  const desktop = ui.state!;
  const p = desktop.preferences;
  const model = useModel();
  const status = useStatus();
  const traffic = useTraffic();
  const routing = useProcRouteStatus();
  const install = useProcRouteInstall();
  const connection = useDesktopMutation(desktopApi.connect);
  const selection = useDesktopMutation(desktopApi.select);
  const preference = useDesktopMutation(desktopApi.preferences);
  const binding = useDesktopMutation(desktopApi.bind);
  const appSave = useDesktopMutation(
    ({ path, selected }: { path: string; selected: boolean }) =>
      desktopApi.app(path, selected),
  );
  const savePort = useDesktopMutation(desktopApi.port);
  const test = useTestDelays();
  const messages = useApplyResult();
  const [mode, setMode] = useState<CaptureMode>(p.capture_mode);
  const [error, setError] = useState("");
  const [dialog, setDialog] = useState<
    | "node"
    | "import"
    | "manual"
    | "apps"
    | "permission"
    | "bind"
    | "port"
    | null
  >(null);
  const [pendingNode, setPendingNode] = useState<NodeRef | null>(null);
  const [bindId, setBindId] = useState("");
  const [port, setPort] = useState("");
  const [options, setOptions] = useState(false);
  useEffect(() => {
    setMode(p.capture_mode);
  }, [p.capture_mode]);
  if (!model.data || !status.data)
    return (
      <Stack sx={{ py: 8, alignItems: "center" }}>
        <CircularProgress />
        <Typography sx={{ mt: 2 }}>正在读取代理配置…</Typography>
        {model.error && (
          <Alert severity="error">{errorMessage(model.error)}</Alert>
        )}
      </Stack>
    );
  const m = model.data;
  const st = status.data;
  const listener = m.listeners.find((l) => l.id === p.listener_id);
  const exit = m.exits.find((e) => e.id === (listener?.exit ?? p.exit_id));
  const selected =
    exit && typeof exit.kind === "object" && "Node" in exit.kind
      ? exit.kind.Node.node
      : null;
  const proxyName = selected ? proxyNameOf(m, selected) : null;
  const latency = proxyName ? st.delays[proxyName] : undefined;
  const count =
    m.manual_nodes.length +
    m.subscriptions.reduce((n, s) => n + s.nodes.length, 0);
  const otherSystemListener =
    desktop.requested === "System" &&
    desktop.system_listener_id != null &&
    desktop.system_listener_id !== p.listener_id;
  const connected = desktop.connection !== null && !otherSystemListener;
  const requested = desktop.requested !== null;
  const busy =
    connection.isPending ||
    selection.isPending ||
    preference.isPending ||
    binding.isPending;
  const rules = m.process_routing.rules.filter((r) =>
    p.managed_rules.includes(r.id),
  );
  const componentReady = routing.data?.component === "Running";
  const onError = (e: unknown) => setError(errorMessage(e));
  const selectNode = () => {
    setPendingNode(selected);
    setError("");
    setDialog(listener ? "node" : "bind");
  };
  const setScope = async (next: CaptureMode) => {
    if (next === "Apps" && !componentReady) {
      setDialog("permission");
      return;
    }
    if (next === "Apps" && requested && p.selected_apps.length === 0) {
      setError("先添加需要代理的应用，再切换到指定应用。");
      setDialog("apps");
      return;
    }
    setError("");
    try {
      if (requested) await connection.mutateAsync(next);
      else await preference.mutateAsync({ ...p, capture_mode: next });
      setMode(next);
    } catch (e) {
      onError(e);
    }
  };
  const toggleConnection = async () => {
    setError("");
    if (!requested && !listener) {
      setDialog("bind");
      return;
    }
    if (!requested && !selected) {
      setPendingNode(null);
      setDialog("node");
      return;
    }
    if (!requested && mode === "Apps" && !componentReady) {
      setDialog("permission");
      return;
    }
    try {
      await connection.mutateAsync(requested ? null : mode);
    } catch (e) {
      onError(e);
    }
  };
  return (
    <Stack
      spacing={2.5}
      sx={{ maxWidth: mode === "Apps" ? 900 : 520, mx: "auto" }}
    >
      <Stack direction="row" spacing={1.5} sx={{ alignItems: "center" }}>
        <Box
          sx={{
            bgcolor: "primary.main",
            color: "primary.contrastText",
            p: 0.75,
            display: "flex",
            borderRadius: 1,
          }}
        >
          <RouteOutlined />
        </Box>
        <Typography sx={{ flex: 1, fontWeight: 600, fontSize: 20 }}>
          Routa
        </Typography>
        <Chip label="简洁版" size="small" />
      </Stack>
      {(error || desktop.error) && (
        <Alert
          severity="error"
          action={
            <Button component={Link} to="/diagnostics" color="inherit">
              诊断
            </Button>
          }
        >
          {error || desktop.error}
        </Alert>
      )}
      {otherSystemListener ? (
        <Alert severity="info">
          完整版正在使用「
          {m.listeners.find((l) => l.id === desktop.system_listener_id)?.name ??
            "其他监听器"}
          」作为系统代理。下方节点用于简洁版；重新开启时使用该节点。
        </Alert>
      ) : (
        requested &&
        !connected &&
        !busy && (
          <Alert severity="warning">
            代理尚未确认生效，请检查内核、端口或分流组件。可以关闭接管后重试。
          </Alert>
        )
      )}
      {count === 0 ? (
        <Paper sx={{ p: 4, textAlign: "center" }}>
          <Typography variant="h5">添加节点后即可开始</Typography>
          <Typography color="text.secondary" sx={{ my: 2 }}>
            导入订阅，或手动添加一个节点。
          </Typography>
          <Button
            fullWidth
            variant="contained"
            sx={{ minHeight: 42 }}
            onClick={() => setDialog("import")}
          >
            导入订阅
          </Button>
          <Button sx={{ mt: 1 }} onClick={() => setDialog("manual")}>
            手动添加节点
          </Button>
          {m.subscriptions.map((s) =>
            s.last_result && "Failed" in s.last_result ? (
              <Alert
                key={s.id}
                severity="error"
                sx={{ mt: 2, textAlign: "left" }}
              >
                {s.name}：{s.last_result.Failed.reason}
                <Button component={Link} to="/nodes">
                  查看订阅
                </Button>
              </Alert>
            ) : null,
          )}
        </Paper>
      ) : (
        <>
          <Box>
            <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
              <Box
                sx={{
                  width: 8,
                  height: 8,
                  borderRadius: "50%",
                  bgcolor: connected ? "success.main" : "text.disabled",
                }}
              />
              <Typography sx={{ fontWeight: 600, fontSize: 18 }}>
                {busy
                  ? "正在应用…"
                  : connected
                    ? mode === "Apps"
                      ? "应用代理已开启"
                      : "代理已开启"
                    : "代理未开启"}
              </Typography>
            </Stack>
            <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
              {mode === "Tun"
                ? "TUN · 虚拟网卡"
                : mode === "Apps"
                  ? "指定应用"
                  : "系统代理"}
            </Typography>
          </Box>
          <Box>
            <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
              当前节点
            </Typography>
            <Button
              fullWidth
              variant="outlined"
              onClick={selectNode}
              disabled={busy}
              sx={{
                textAlign: "left",
                justifyContent: "flex-start",
                p: 2,
                bgcolor: "background.paper",
                gap: 1.5,
              }}
            >
              <LanguageOutlined color="action" />
              <Box sx={{ flex: 1, minWidth: 0 }}>
                <Typography noWrap sx={{ fontSize: 20, fontWeight: 600 }}>
                  {selected ? nodeLabel(m, selected) : "请选择节点"}
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  {selected && "subscription_id" in selected
                    ? m.subscriptions.find(
                        (s) => s.id === selected.subscription_id,
                      )?.name
                    : "手动节点"}
                </Typography>
              </Box>
              <Typography variant="caption" color="text.secondary">
                {latency ? delayText(latency) : "未测速"}
              </Typography>
              <ChevronRightOutlined />
            </Button>
          </Box>
          <Button
            fullWidth
            variant={requested ? "outlined" : "contained"}
            sx={{ minHeight: 44 }}
            onClick={() => void toggleConnection()}
            disabled={busy}
            startIcon={busy ? <CircularProgress size={17} /> : undefined}
          >
            {busy ? "正在应用…" : requested ? "关闭代理" : "开启代理"}
          </Button>
          <Stack
            direction="row"
            sx={{ justifyContent: "space-between", alignItems: "center" }}
          >
            <Stack direction="row" sx={{ alignItems: "center" }}>
              <Typography variant="body2" color="text.secondary">
                代理范围
              </Typography>
              <Tooltip title="系统代理只影响遵循系统代理设置的应用；指定应用通过分流组件接管。">
                <IconButton size="small" aria-label="代理范围说明">
                  <InfoOutlined sx={{ fontSize: 16 }} />
                </IconButton>
              </Tooltip>
            </Stack>
            <ToggleButtonGroup
              size="small"
              exclusive
              value={mode === "Apps" ? "Apps" : "System"}
              onChange={(_, v) => v && void setScope(v)}
              disabled={busy}
            >
              <ToggleButton value="System">
                {mode === "Tun" ? "TUN 代理" : "系统代理"}
              </ToggleButton>
              <ToggleButton value="Apps">指定应用</ToggleButton>
            </ToggleButtonGroup>
          </Stack>
          {mode === "Apps" ? (
            <>
              <Stack
                direction="row"
                sx={{ justifyContent: "space-between", alignItems: "center" }}
              >
                <Typography sx={{ fontWeight: 600 }}>指定应用</Typography>
                <Button onClick={() => setDialog("apps")}>＋ 添加应用</Button>
              </Stack>
              <Box
                sx={{
                  display: "grid",
                  gridTemplateColumns: { xs: "1fr", md: "1fr 1fr" },
                  gap: 1.5,
                }}
              >
                {rules.map((r) => (
                  <Paper
                    key={r.id}
                    sx={{
                      p: 2,
                      display: "flex",
                      alignItems: "center",
                      gap: 1.5,
                      bgcolor: p.selected_apps.includes(r.id)
                        ? "action.selected"
                        : "background.paper",
                    }}
                  >
                    <AppsOutlined color="action" />
                    <Box sx={{ flex: 1, minWidth: 0 }}>
                      <Typography noWrap title={r.exe_path}>
                        {r.exe_path.split(/[\\/]/).pop()}
                      </Typography>
                      <Typography variant="caption" color="text.secondary">
                        {r.listener !== p.listener_id
                          ? `使用其他监听器 · 关闭并重新勾选后使用当前节点`
                          : requested ||
                              (m.process_routing.enabled && r.user_enabled)
                            ? (routing.data?.actions[r.id]?.reason ??
                              routing.data?.actions[r.id]?.action ??
                              "等待确认")
                            : p.selected_apps.includes(r.id)
                              ? "已选择"
                              : "直连"}
                      </Typography>
                    </Box>
                    <Switch
                      checked={p.selected_apps.includes(r.id)}
                      disabled={appSave.isPending}
                      slotProps={{
                        input: { "aria-label": `${r.exe_path}走代理` },
                      }}
                      onChange={async (_, selected) => {
                        try {
                          await appSave.mutateAsync({
                            path: r.exe_path,
                            selected,
                          });
                        } catch (e) {
                          onError(e);
                        }
                      }}
                    />
                  </Paper>
                ))}
              </Box>
              {rules.length === 0 && (
                <Typography variant="body2" color="text.secondary">
                  添加需要代理的应用，其余应用不由此规则接管。
                </Typography>
              )}
            </>
          ) : (
            <Stack
              direction="row"
              divider={<Divider orientation="vertical" flexItem />}
              spacing={3}
            >
              <Box sx={{ flex: 1 }}>
                <Typography variant="caption" color="text.secondary">
                  ↓ 下载
                </Typography>
                <Typography sx={{ fontSize: 22 }}>
                  {connected && traffic
                    ? formatRate(traffic.total_rate.down)
                    : "—"}
                </Typography>
              </Box>
              <Box sx={{ flex: 1 }}>
                <Typography variant="caption" color="text.secondary">
                  ↑ 上传
                </Typography>
                <Typography sx={{ fontSize: 22 }}>
                  {connected && traffic
                    ? formatRate(traffic.total_rate.up)
                    : "—"}
                </Typography>
              </Box>
            </Stack>
          )}
          <Button
            color="inherit"
            size="small"
            sx={{ alignSelf: "flex-start", color: "text.secondary" }}
            onClick={() => setOptions(!options)}
          >
            连接选项 {options ? "⌃" : "⌄"}
          </Button>
          <Collapse in={options}>
            <Stack spacing={1.5}>
              <ToggleButtonGroup
                size="small"
                exclusive
                value={mode}
                disabled={busy}
                onChange={(_, v) => v && void setScope(v)}
              >
                <ToggleButton value="System">系统代理</ToggleButton>
                <ToggleButton value="Tun">TUN（需要管理员权限）</ToggleButton>
              </ToggleButtonGroup>
              <Stack
                direction="row"
                sx={{ alignItems: "center", justifyContent: "space-between" }}
              >
                <Typography variant="caption" color="text.secondary" data-selectable={!!listener}>
                  {listener
                    ? `${listener.listen}:${listener.port} · ${listener.listener_type === "Mixed" ? "混合" : "SOCKS5"}`
                    : "尚未绑定默认监听器"}
                </Typography>
                <Button
                  size="small"
                  disabled={requested}
                  onClick={() => {
                    setPort(String(listener?.port ?? 7881));
                    setDialog(listener ? "port" : "bind");
                  }}
                >
                  修改端口
                </Button>
              </Stack>
              {mode === "Apps" && (
                <Typography variant="caption" color="text.secondary">
                  应用代理意外故障时默认阻断；可在完整版修改故障策略。
                </Typography>
              )}
            </Stack>
          </Collapse>
        </>
      )}
      {(m.listeners.some((l) => l.id !== p.listener_id && l.user_enabled) ||
        m.process_routing.rules.some(
          (r) => !p.managed_rules.includes(r.id) && r.user_enabled,
        ) ||
        (!requested &&
          m.process_routing.enabled &&
          rules.some((r) => r.user_enabled))) && (
        <Alert
          severity="info"
          action={
            <Button
              onClick={async () => {
                try {
                  await ui.chooseMode("Full");
                } catch (e) {
                  onError(e);
                }
              }}
            >
              查看
            </Button>
          }
        >
          完整版还有其他配置，切换界面不会停止它们。
        </Alert>
      )}
      {dialog === "import" && (
        <ImportSubscription onClose={() => setDialog(null)} />
      )}
      {dialog === "manual" && (
        <ImportManualNode onClose={() => setDialog(null)} />
      )}
      {dialog === "node" && (
        <Dialog
          open
          onClose={() => !selection.isPending && setDialog(null)}
          fullWidth
          maxWidth="sm"
        >
          <DialogTitle>选择节点</DialogTitle>
          <DialogContent>
            <Stack spacing={2} sx={{ pt: 1 }}>
              <NodePicker value={pendingNode} onChange={setPendingNode} />
              {error && <Alert severity="error">{error}</Alert>}
              <Button
                disabled={test.isPending}
                onClick={() =>
                  test.mutate(
                    m.subscriptions.flatMap((s) =>
                      s.nodes.map((n) => `${s.name}/${n.name}`),
                    ),
                  )
                }
              >
                测速
              </Button>
              <Typography variant="caption" color="text.secondary">
                选择不会自动开启代理；在线切换时，内核可能重新建立受影响的连接。
              </Typography>
            </Stack>
          </DialogContent>
          <DialogActions>
            <Button
              disabled={selection.isPending}
              onClick={() => setDialog(null)}
            >
              取消
            </Button>
            <Button
              variant="contained"
              disabled={!pendingNode || selection.isPending}
              onClick={async () => {
                try {
                  await selection.mutateAsync(pendingNode!);
                  setDialog(null);
                  setError("");
                } catch (e) {
                  onError(e);
                }
              }}
            >
              {selection.isPending ? "正在应用…" : "使用此节点"}
            </Button>
          </DialogActions>
        </Dialog>
      )}
      {dialog === "apps" && (
        <SimpleAppPicker
          onClose={() => setDialog(null)}
          onSave={async (path) => {
            await appSave.mutateAsync({ path, selected: true });
          }}
        />
      )}
      {dialog === "permission" && (
        <Dialog open onClose={() => !install.isPending && setDialog(null)}>
          <DialogTitle>启用应用代理</DialogTitle>
          <DialogContent>
            <Typography variant="body2">
              首次使用需要安装分流组件。Windows
              会请求管理员权限，未授权时保留原连接方式。
            </Typography>
            {error && (
              <Alert severity="error" sx={{ mt: 2 }}>
                {error}
              </Alert>
            )}
          </DialogContent>
          <DialogActions>
            <Button
              disabled={install.isPending}
              onClick={() => setDialog(null)}
            >
              暂不启用
            </Button>
            <Button
              variant="contained"
              disabled={install.isPending}
              onClick={async () => {
                try {
                  await install.mutateAsync();
                  messages.showMessage(
                    "组件已安装，请待组件就绪后选择指定应用",
                    "success",
                  );
                  setDialog(null);
                } catch (e) {
                  onError(e);
                }
              }}
            >
              {install.isPending ? "正在安装…" : "安装组件"}
            </Button>
          </DialogActions>
        </Dialog>
      )}
      {dialog === "bind" && (
        <Dialog
          open
          onClose={() => !binding.isPending && setDialog(null)}
          fullWidth
          maxWidth="sm"
        >
          <DialogTitle>选择默认监听器</DialogTitle>
          <DialogContent>
            <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
              仅绑定你选中的监听器，其他配置保持原样。
            </Typography>
            <List>
              {m.listeners
                .filter((l) => l.listener_type !== "Http" && !l.auth)
                .map((l) => (
                  <ListItemButton
                    key={l.id}
                    selected={bindId === l.id}
                    onClick={() => setBindId(l.id)}
                  >
                    <Radio checked={bindId === l.id} />
                    <ListItemText
                      primary={`${l.port} · ${l.name}`}
                      secondary={l.listen}
                    />
                  </ListItemButton>
                ))}
            </List>
            {error && <Alert severity="error">{error}</Alert>}
          </DialogContent>
          <DialogActions>
            <Button
              disabled={binding.isPending}
              onClick={() => setDialog(null)}
            >
              取消
            </Button>
            <Button
              disabled={
                binding.isPending || m.listeners.some((l) => l.port === 7881)
              }
              onClick={async () => {
                try {
                  await binding.mutateAsync(null);
                  setDialog(null);
                } catch (e) {
                  onError(e);
                }
              }}
            >
              新建 7881 混合监听器
            </Button>
            <Button
              variant="contained"
              disabled={!bindId || binding.isPending}
              onClick={async () => {
                try {
                  await binding.mutateAsync(bindId);
                  setDialog(null);
                } catch (e) {
                  onError(e);
                }
              }}
            >
              绑定所选监听器
            </Button>
          </DialogActions>
        </Dialog>
      )}
      {dialog === "port" && (
        <FormDialog onClose={() => setDialog(null)}>
          <DialogTitle>修改本地端口</DialogTitle>
          <DialogContent>
            <TextField
              autoFocus
              label="端口"
              type="number"
              value={port}
              onChange={(e) => setPort(e.target.value)}
              sx={{ mt: 1 }}
            />
            {error && (
              <Alert severity="error" sx={{ mt: 2 }}>
                {error}
              </Alert>
            )}
          </DialogContent>
          <DialogActions>
            <Button
              disabled={savePort.isPending}
              onClick={() => setDialog(null)}
            >
              取消
            </Button>
            <Button
              variant="contained"
              disabled={
                savePort.isPending ||
                !/^\d+$/.test(port) ||
                Number(port) < 1 ||
                Number(port) > 65535
              }
              onClick={async () => {
                try {
                  await savePort.mutateAsync(Number(port));
                  setDialog(null);
                  setError("");
                } catch (e) {
                  onError(e);
                }
              }}
            >
              保存端口
            </Button>
          </DialogActions>
        </FormDialog>
      )}
    </Stack>
  );
}

function SimpleAppPicker({
  onClose,
  onSave,
}: {
  onClose: () => void;
  onSave: (path: string) => Promise<void>;
}) {
  const running = useRunningProcesses();
  const [search, setSearch] = useState("");
  const [path, setPath] = useState("");
  const [chosen, setChosen] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  return (
    <Dialog open fullWidth maxWidth="sm" onClose={() => !busy && onClose()}>
      <DialogTitle>添加应用</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 1 }}>
          <TextField
            label="搜索运行中的应用"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          {running.isLoading ? (
            <CircularProgress size={22} />
          ) : (
            <List sx={{ maxHeight: 270, overflow: "auto" }}>
              {(running.data ?? [])
                .filter((a) =>
                  `${a.name} ${a.exe_path}`
                    .toLowerCase()
                    .includes(search.toLowerCase()),
                )
                .map((a) => (
                  <ListItemButton
                    key={a.exe_path}
                    selected={chosen.includes(a.exe_path)}
                    onClick={() =>
                      setChosen((c) =>
                        c.includes(a.exe_path)
                          ? c.filter((x) => x !== a.exe_path)
                          : [...c, a.exe_path],
                      )
                    }
                  >
                    <ListItemText
                      primary={a.name}
                      secondary={a.exe_path}
                      slotProps={{ secondary: { noWrap: true } }}
                    />
                    <Radio checked={chosen.includes(a.exe_path)} />
                  </ListItemButton>
                ))}
            </List>
          )}
          {running.error && (
            <Alert severity="error">{errorMessage(running.error)}</Alert>
          )}
          <TextField
            label="或输入程序完整路径"
            placeholder="C:\Apps\Example\example.exe"
            value={path}
            onChange={(e) => setPath(e.target.value)}
          />
          {error && <Alert severity="error">{error}</Alert>}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button disabled={busy} onClick={onClose}>
          取消
        </Button>
        <Button
          variant="contained"
          disabled={busy || (!path.trim() && !chosen.length)}
          onClick={async () => {
            setBusy(true);
            setError("");
            try {
              for (const p of new Set([
                ...chosen,
                ...(path.trim() ? [path.trim()] : []),
              ]))
                await onSave(p);
              onClose();
            } catch (e) {
              setError(errorMessage(e));
            } finally {
              setBusy(false);
            }
          }}
        >
          {busy ? "正在添加…" : "添加所选应用"}
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function ImportSubscription({ onClose }: { onClose: () => void }) {
  const [subscription] = useState(newSubscription);
  return <SubscriptionDialog subscription={subscription} onClose={onClose} />;
}
function ImportManualNode({ onClose }: { onClose: () => void }) {
  const [node] = useState(() => newManualNode());
  return <ManualNodeDialog node={node} onClose={onClose} />;
}

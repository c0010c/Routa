import { useRef, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import {
  Alert,
  Box,
  Button,
  IconButton,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import InfoOutlined from "@mui/icons-material/InfoOutlined";
import MemoryOutlined from "@mui/icons-material/MemoryOutlined";
import LanOutlined from "@mui/icons-material/LanOutlined";
import {
  useCoreControl,
  useModel,
  useStatus,
  useTraffic,
  useResetTraffic,
  errorMessage,
  unwrap,
} from "../api/client";
import {
  desktopApi,
  useDesktopMutation,
  type CaptureMode,
} from "../api/desktop";
import { commands, type Exit, type Listener, type Model } from "../bindings";
import { useInterface } from "../components/InterfaceProvider";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { collectProblems } from "./Overview";
import { formatBytes, formatRate } from "../lib/format";
import { expandMembers, nodeLabel } from "../lib/refs";

const TYPE_TEXT = { Mixed: "混合", Http: "HTTP", Socks: "SOCKS5" } as const;
const STRATEGY_TEXT = {
  Select: "手动选择",
  UrlTest: "自动测速",
  Fallback: "故障转移",
  LoadBalance: "负载均衡",
} as const;

function proxyLabel(listener: Listener) {
  return `${listener.name} · ${TYPE_TEXT[listener.listener_type]} :${listener.port}`;
}

function nodeInfo(model: Model | undefined, listener: Listener | undefined) {
  if (!model) return { label: "正在读取节点…", strategy: "", valid: false };
  if (!listener)
    return { label: "尚未选择默认代理", strategy: "", valid: false };
  const exit = model.exits.find((e) => e.id === listener.exit);
  if (!exit)
    return {
      label: listener.exit ? "出口已不存在" : "未配置出口",
      strategy: "",
      valid: false,
    };
  if (exit.kind === "Direct")
    return { label: "直连", strategy: "直连出口", valid: true };
  if ("Node" in exit.kind) {
    const ref = exit.kind.Node.node;
    return {
      label: nodeLabel(model, ref),
      strategy: "固定节点",
      valid: expandMembers(model, [ref]).length > 0,
    };
  }
  const group = exit.kind.Group;
  const members = expandMembers(model, group.members);
  const selected = group.strategy === "Select" ? group.selected : null;
  return {
    label: selected
      ? nodeLabel(model, selected)
      : members.length
        ? "由内核选择"
        : "没有可用节点",
    strategy: STRATEGY_TEXT[group.strategy],
    valid:
      members.length > 0 &&
      (!selected || expandMembers(model, [selected]).length > 0),
  };
}

function StatusPanel({
  label,
  icon,
  value,
  color,
  children,
  hint,
}: {
  label: string;
  icon: ReactNode;
  value: string;
  color: string;
  children: ReactNode;
  hint?: string;
}) {
  return (
    <Paper
      component="section"
      aria-label={label}
      sx={{ p: 2.5, minWidth: 0, minHeight: 166 }}
    >
      <Stack
        direction="row"
        spacing={0.75}
        sx={{ alignItems: "center", color: "text.secondary", height: 24 }}
      >
        {icon}
        <Typography variant="body2">{label}</Typography>
        {hint && (
          <Tooltip title={hint}>
            <IconButton
              size="small"
              aria-label={`${label}说明`}
              sx={{ p: 0.25 }}
            >
              <InfoOutlined sx={{ fontSize: 16 }} />
            </IconButton>
          </Tooltip>
        )}
      </Stack>
      <Stack
        direction="row"
        spacing={1.25}
        sx={{ alignItems: "center", my: 1.25, minHeight: 32 }}
        aria-live="polite"
      >
        <Box
          aria-hidden
          sx={{
            width: 9,
            height: 9,
            borderRadius: "50%",
            bgcolor: color,
            flex: "none",
            transition: "background-color 140ms ease",
            "@media (prefers-reduced-motion: reduce)": { transition: "none" },
          }}
        />
        <Typography sx={{ fontSize: 21, fontWeight: 600 }}>{value}</Typography>
      </Stack>
      <Box
        sx={{
          height: 42,
          overflow: "auto",
          overflowWrap: "anywhere",
          color: "text.secondary",
          typography: "caption",
        }}
      >
        {children}
      </Box>
    </Paper>
  );
}

export function RuntimeSummary() {
  const model = useModel(),
    status = useStatus(),
    tick = useTraffic(),
    reset = useResetTraffic();
  const ui = useInterface();
  const desktop = ui?.state;
  const bind = useDesktopMutation(desktopApi.bind),
    connect = useDesktopMutation(desktopApi.connect);
  const core = useCoreControl();
  const [error, setError] = useState("");
  const [confirm, setConfirm] = useState(false);
  // Guard the short interval before React paints a mutation's pending state.
  const operation = useRef(false);
  const m = model.data,
    s = status.data;
  const desktopReady = !!desktop && !ui?.error;
  const dataReady = desktopReady && !!m && !!s && !model.error && !status.error;
  const busy = bind.isPending || connect.isPending || core.isPending;
  const requested = desktop?.requested;
  const defaultId = desktop?.preferences.listener_id;
  const listener = m?.listeners.find((l) => l.id === defaultId);
  const node = nodeInfo(m, listener);
  const connection =
    desktopReady && !status.error && s?.core === "Running"
      ? desktop.connection
      : null;
  const active = connection === "System" || connection === "Tun";
  // The proxy page can enable a different system listener without rebinding the default.
  const captureId =
    requested === "System"
      ? (desktop?.system_listener_id ?? defaultId)
      : defaultId;
  const captureListener = m?.listeners.find((l) => l.id === captureId);
  const captureNode = nodeInfo(m, captureListener);
  const groupExits = [listener, captureListener]
    .map((l) => m?.exits.find((e) => e.id === l?.exit))
    .filter(
      (e): e is Exit => !!e && typeof e.kind === "object" && "Group" in e.kind,
    );
  const names = [...new Set(groupExits.map((e) => e.name))];
  const groupNow = useQuery({
    queryKey: ["groupNow", ...names],
    queryFn: async () => unwrap(await commands.groupNow(names)),
    enabled:
      names.length > 0 &&
      s?.core === "Running" &&
      !status.error &&
      !model.error,
    refetchInterval: 5000,
  });
  const currentNode = (
    l: Listener | undefined,
    fallback: string,
    live = false,
  ) => {
    const exit = m?.exits.find((e) => e.id === l?.exit);
    if (!exit || typeof exit.kind !== "object" || !("Group" in exit.kind)) {
      // A saved node can differ from the configuration the core kept running.
      if (live && s?.last_apply && "FailedKeptPrevious" in s.last_apply)
        return "当前节点未确认";
      return fallback;
    }
    if (exit.kind.Group.strategy === "LoadBalance") return "按连接分配节点";
    if (s?.core !== "Running") return fallback;
    if (groupNow.error) return "当前节点读取失败";
    const now = groupNow.data?.[exit.name];
    if (!now)
      return groupNow.isPending ? "正在读取当前节点…" : "当前节点未确认";
    const prefix = `${exit.name}/`;
    return now.startsWith(prefix) ? now.slice(prefix.length) : now;
  };
  const coreKind = s
    ? typeof s.core === "object"
      ? "Crashed"
      : s.core
    : undefined;
  const coreValue = status.error
    ? "状态未知"
    : !s
      ? "读取中…"
      : coreKind === "Running"
        ? "运行中"
        : coreKind === "Starting"
          ? "启动中"
          : coreKind === "Stopped"
            ? "已停止"
            : "已崩溃";
  const captureUnknown = !!ui?.error || !!status.error;
  const captureUnconfirmed = requested === "System" || requested === "Tun";
  const captureValue = captureUnknown
    ? "状态未知"
    : !desktop || !s
      ? "读取中…"
      : active
        ? connection === "System"
          ? "系统代理已开启"
          : "TUN 已开启"
        : connect.isPending
          ? "切换中…"
          : captureUnconfirmed
            ? desktop.error || s.core !== "Running"
              ? "接管异常"
              : "尚未生效"
            : "未开启";
  const captureColor = captureUnknown
    ? "warning.main"
    : active
      ? "primary.main"
      : captureUnconfirmed
        ? "warning.main"
        : "text.disabled";
  const available =
    m?.listeners.filter((l) => l.listener_type !== "Http" && !l.auth) ?? [];
  const unavailableSelection =
    !!defaultId && !available.some((l) => l.id === defaultId);
  const selectedMissing = !!m && !model.error && unavailableSelection;
  const validListener =
    !!listener?.user_enabled && node.valid && !unavailableSelection;
  const systemReason = !dataReady
    ? "请等待状态读取成功"
    : !validListener
      ? "请先选择可用的默认代理和节点"
      : listener?.listener_type !== "Mixed"
        ? "系统代理需要混合类型的代理"
        : !["127.0.0.1", "0.0.0.0", "localhost"].includes(listener.listen)
          ? "系统代理需要监听本机 IPv4 地址"
          : "";
  const tunReason = !dataReady
    ? "请等待状态读取成功"
    : !validListener
      ? "请先选择可用的默认代理和节点"
      : !desktop?.is_admin
        ? "TUN 需要管理员权限，请以管理员身份运行 Routa"
        : "";
  const changeCapture = async (mode: CaptureMode) => {
    if (operation.current) return;
    operation.current = true;
    setError("");
    try {
      await connect.mutateAsync(requested === mode ? null : mode);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      operation.current = false;
    }
  };
  const traffic = tick ?? s?.traffic;
  const problems = m && s ? collectProblems(m, s) : [];
  const feedback =
    error ||
    (ui?.error ? `接管状态读取失败：${errorMessage(ui.error)}` : "") ||
    desktop?.error ||
    (model.error ? `代理配置读取失败：${errorMessage(model.error)}` : "") ||
    (status.error ? `内核状态读取失败：${errorMessage(status.error)}` : "");
  return (
    <Stack spacing={2}>
      <Box
        sx={{
          display: "grid",
          gridTemplateColumns: {
            xs: "1fr",
            sm: "minmax(0,.85fr) minmax(0,1.3fr)",
          },
          gap: 1.75,
        }}
      >
        <StatusPanel
          label="内核"
          icon={<MemoryOutlined sx={{ fontSize: 17 }} />}
          value={coreValue}
          color={
            status.error
              ? "warning.main"
              : coreKind === "Running"
                ? "success.main"
                : coreKind === "Crashed"
                  ? "error.main"
                  : coreKind === "Starting"
                    ? "warning.main"
                    : "text.disabled"
          }
        >
          <div>
            {m
              ? `mihomo · 控制端口 ${m.settings.controller_port}`
              : model.error
                ? "配置读取失败"
                : "正在读取配置…"}
          </div>
          {s && typeof s.core === "object" && (
            <div>退出码 {s.core.Crashed.exit_code ?? "未知"}</div>
          )}
          {status.error && <div>内核状态读取失败，请重试</div>}
        </StatusPanel>
        <StatusPanel
          label="系统接管"
          icon={<LanOutlined sx={{ fontSize: 17 }} />}
          value={captureValue}
          color={captureColor}
          hint="内核运行与系统接管分别显示。关闭系统代理和 TUN 后，手动配置代理或进程分流的应用仍可能使用代理。"
        >
          {captureUnknown ? (
            <div>状态读取失败，请重试</div>
          ) : !desktop || !s ? (
            <div>正在确认系统代理与 TUN 状态…</div>
          ) : active ? (
            <>
              <div>
                {model.error
                  ? "接管代理配置读取失败"
                  : captureListener
                    ? proxyLabel(captureListener)
                    : m
                      ? "接管代理已不存在"
                      : "正在读取接管代理…"}
              </div>
              <div>
                已选节点：
                {model.error
                  ? "无法读取"
                  : currentNode(captureListener, captureNode.label, true)}
              </div>
            </>
          ) : (
            <>
              <div>
                系统代理 {requested === "System" ? "未确认" : "关闭"} · TUN{" "}
                {requested === "Tun" ? "未确认" : "关闭"}
              </div>
              {connection === "Apps" && <div>指定应用代理已开启</div>}
              {captureUnconfirmed && <div>请检查内核与接管设置</div>}
            </>
          )}
        </StatusPanel>
      </Box>
      <Box
        sx={{
          display: "grid",
          gridTemplateColumns: {
            xs: "repeat(2,minmax(0,1fr))",
            md: "repeat(4,minmax(0,1fr))",
          },
          gap: 1.5,
        }}
      >
        {[
          [
            "上传速率",
            traffic && !status.error && s?.core === "Running"
              ? formatRate(traffic.total_rate.up)
              : "—",
          ],
          [
            "下载速率",
            traffic && !status.error && s?.core === "Running"
              ? formatRate(traffic.total_rate.down)
              : "—",
          ],
          [
            "累计流量",
            traffic
              ? formatBytes(traffic.totals.total.up + traffic.totals.total.down)
              : "—",
          ],
          [
            "生效代理",
            s && m && !status.error && !model.error
              ? `${s.effective_listeners.length} / ${m.listeners.length}`
              : "—",
          ],
        ].map(([label, value]) => (
          <Paper key={label} sx={{ p: 1.75, minWidth: 0 }}>
            <Stack
              direction="row"
              sx={{
                alignItems: "center",
                justifyContent: "space-between",
                minHeight: 24,
              }}
            >
              <Typography variant="caption" color="text.secondary">
                {label}
              </Typography>
              {label === "累计流量" && (
                <Button
                  size="small"
                  sx={{ minWidth: 0, p: 0 }}
                  disabled={!traffic || reset.isPending}
                  onClick={() => setConfirm(true)}
                >
                  清零
                </Button>
              )}
            </Stack>
            <Typography
              variant="h6"
              sx={{
                fontVariantNumeric: "tabular-nums",
                overflowWrap: "anywhere",
              }}
            >
              {value}
            </Typography>
          </Paper>
        ))}
      </Box>
      <Paper component="section" aria-label="接管设置" sx={{ p: 2.5 }}>
        <Stack spacing={1.5}>
          <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>
            接管设置
          </Typography>
          <TextField
            select
            label="默认代理"
            value={defaultId ?? ""}
            disabled={!dataReady || !!requested || busy}
            slotProps={{
              inputLabel: { shrink: true },
              select: { displayEmpty: true },
            }}
            onChange={async (e) => {
              if (operation.current) return;
              operation.current = true;
              setError("");
              try {
                await bind.mutateAsync(e.target.value);
              } catch (e) {
                setError(errorMessage(e));
              } finally {
                operation.current = false;
              }
            }}
          >
            <MenuItem value="" disabled>
              {m
                ? available.length
                  ? "选择代理"
                  : "没有可用代理"
                : "正在读取代理…"}
            </MenuItem>
            {unavailableSelection && (
              <MenuItem value={defaultId!} disabled>
                {model.error
                  ? "代理配置读取失败"
                  : !m
                    ? "正在读取代理…"
                    : listener
                      ? `${proxyLabel(listener)}（不支持接管）`
                      : "默认代理已不存在"}
              </MenuItem>
            )}
            {available.map((l) => (
              <MenuItem key={l.id} value={l.id}>
                {proxyLabel(l)}
              </MenuItem>
            ))}
          </TextField>
          <Box
            sx={{
              height: 42,
              overflow: "auto",
              overflowWrap: "anywhere",
              typography: "body2",
            }}
          >
            <Box component="span" sx={{ color: "text.secondary", mr: 1 }}>
              已选节点
            </Box>
            {model.error
              ? "节点配置读取失败"
              : node.valid
                ? currentNode(listener, node.label)
                : node.label}
            {node.strategy && (
              <Box component="span" sx={{ color: "text.secondary", ml: 1 }}>
                · {node.strategy}
              </Box>
            )}
            {selectedMissing && (
              <Typography
                variant="caption"
                component="div"
                sx={{ color: "warning.main" }}
              >
                请重新选择默认代理
              </Typography>
            )}
          </Box>
          <Box sx={{ display: "flex", gap: 1, flexWrap: "wrap" }}>
            {(["System", "Tun"] as const).map((mode) => {
              const closing = requested === mode;
              const reason = closing
                ? ""
                : mode === "System"
                  ? systemReason
                  : tunReason;
              const name = mode === "System" ? "系统代理" : "TUN";
              return (
                <Tooltip key={mode} title={reason}>
                  <span>
                    <Button
                      variant={
                        closing || (!requested && mode === "System")
                          ? "contained"
                          : "outlined"
                      }
                      aria-pressed={connection === mode}
                      disabled={
                        !desktopReady ||
                        busy ||
                        !!reason ||
                        (!closing && coreKind === "Starting")
                      }
                      onClick={() => void changeCapture(mode)}
                    >
                      {connect.isPending &&
                      connect.variables === (closing ? null : mode)
                        ? `正在${closing ? "关闭" : "开启"}${name}…`
                        : `${closing ? "关闭" : "开启"}${mode === "Tun" ? " TUN" : name}`}
                    </Button>
                  </span>
                </Tooltip>
              );
            })}
            <Button component={Link} to="/process-routing">
              指定应用
            </Button>
          </Box>
          <Box sx={{ height: 48, overflow: "auto" }}>
            {busy && (
              <Typography variant="body2" color="text.secondary" role="status">
                {bind.isPending
                  ? "正在保存默认代理…"
                  : connect.isPending
                    ? "正在切换接管方式…"
                    : "正在操作内核…"}
              </Typography>
            )}
            {feedback && (
              <Typography
                component="div"
                variant="body2"
                sx={{ color: "error.main" }}
                role="alert"
              >
                {feedback}
              </Typography>
            )}
            {(ui?.error || model.error || status.error) && (
              <Button
                size="small"
                onClick={() => {
                  ui?.retry();
                  void model.refetch();
                  void status.refetch();
                }}
              >
                重新读取
              </Button>
            )}
          </Box>
        </Stack>
      </Paper>
      {problems.length > 0 && (
        <Stack spacing={1} sx={{ maxHeight: 152, overflow: "auto" }}>
          {problems.map((p) => (
            <Alert key={p} severity="warning">
              {p}
            </Alert>
          ))}
        </Stack>
      )}
      <Box
        sx={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          flexWrap: "wrap",
          gap: 1,
        }}
      >
        <Box sx={{ display: "flex", gap: 1 }}>
          <Button component={Link} to="/listeners">
            管理代理
          </Button>
          <Button component={Link} to="/diagnostics">
            故障诊断
          </Button>
        </Box>
        <Box
          sx={{ display: "flex", gap: 1, alignItems: "center", minHeight: 32 }}
        >
          <Typography variant="caption" color="text.secondary">
            内核操作
          </Typography>
          {(coreKind === "Stopped" || coreKind === "Crashed" || !s) && (
            <Button
              size="small"
              disabled={busy || !s || !!status.error}
              onClick={() => core.start.mutate()}
            >
              启动
            </Button>
          )}
          {(coreKind === "Running" || coreKind === "Crashed") && (
            <Button
              size="small"
              disabled={busy || !!status.error}
              onClick={() => core.restart.mutate()}
            >
              重启
            </Button>
          )}
          {coreKind === "Running" && (
            <Button
              size="small"
              disabled={busy || !!status.error}
              onClick={() => core.stop.mutate()}
            >
              停止
            </Button>
          )}
          {coreKind === "Starting" && (
            <Typography variant="caption" color="text.secondary">
              启动中…
            </Typography>
          )}
        </Box>
      </Box>
      {confirm && (
        <ConfirmDeleteDialog
          title="清零流量"
          confirmLabel="清零"
          text="确定清零累计流量吗？"
          onConfirm={() => reset.mutateAsync()}
          onClose={() => setConfirm(false)}
        />
      )}
    </Stack>
  );
}

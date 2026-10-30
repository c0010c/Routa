import { ListenerNodeDialog } from "./ListenerNodeDialog";
import { fieldErrors, type FieldErrors } from "../lib/validation";
import {
  Alert,
  Box,
  Button,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  MenuItem,
  Stack,
  Switch,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from "@mui/material";
import { useState } from "react";
import {
  errorMessage,
  isValidationError,
  useModel,
  useSaveProxy,
} from "../api/client";
import type {
  Exit,
  ExitKind,
  GroupStrategy,
  Listener,
  ListenerType,
  NodeRef,
  PortRule,
  RuleAction,
} from "../bindings";
import { expandMembers, nodeKey, nodeLabel } from "../lib/refs";
import { FormDialog } from "../components/FormDialog";
import { InterfacePicker } from "../components/InterfacePicker";
import { MemberPicker } from "../components/MemberPicker";
import { NodePicker } from "../components/NodePicker";

function proposePort(existing: Listener[]) {
  if (!existing.length) return 7881;
  const max = Math.max(...existing.map((l) => l.port));
  if (max < 65535) return max + 1;
  const used = new Set(existing.map((l) => l.port));
  for (let p = 1024; p <= 65535; p++) if (!used.has(p)) return p;
  return 7881;
}
export function newListener(existing: Listener[]): Listener {
  return {
    id: crypto.randomUUID(),
    name: "",
    listener_type: "Mixed",
    listen: "127.0.0.1",
    port: proposePort(existing),
    auth: null,
    exit: null,
    user_enabled: true,
  };
}
const actions: [RuleAction, string][] = [
  ["Proxy", "代理"],
  ["Direct", "直连"],
  ["Reject", "阻断"],
];
export function ListenerDialog({
  listener,
  onClose,
  onSaved,
  initialNode,
}: {
  listener: Listener;
  onClose: () => void;
  onSaved?: (l: Listener) => void;
  initialNode?: NodeRef;
}) {
  const model = useModel();
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });
  const [showPw, setShowPw] = useState(false);
  const save = useSaveProxy();
  const m = model.data;
  const [l, setL] = useState(listener);
  const [port, setPort] = useState(String(l.port));
  const ipv6Listen = l.listen.includes(":");
  const allowLan = !["127.0.0.1", "::1", "[::1]", "localhost"].includes(
    l.listen,
  );
  const [error, setError] = useState("");
  const [target, setTarget] = useState<Exit>(
    () =>
      m?.exits.find((e) => e.id === l.exit) ?? {
        id: `proxy-${l.id}`,
        name: `代理-${l.id}`,
        interface: null,
        kind: initialNode ? { Node: { node: initialNode } } : "Direct",
      },
  );
  const [mode, setMode] = useState(
    l.routing ? "rules" : target.kind === "Direct" ? "direct" : "forward",
  );
  const [kind, setKind] = useState<"Node" | "Group">(
    typeof target.kind === "object" && "Group" in target.kind
      ? "Group"
      : "Node",
  );
  const [node, setNode] = useState<NodeRef | null>(
    typeof target.kind === "object" && "Node" in target.kind
      ? target.kind.Node.node
      : (initialNode ?? null),
  );
  const [group, setGroup] = useState(
    typeof target.kind === "object" && "Group" in target.kind
      ? target.kind.Group
      : { strategy: "UrlTest" as GroupStrategy, members: [], selected: null },
  );
  const [rules, setRules] = useState<PortRule[]>(l.routing?.rules ?? []);
  const [unmatched, setUnmatched] = useState<RuleAction>(
    l.routing?.unmatched ?? "Proxy",
  );
  const [choosingNode, setChoosingNode] = useState(false);
  const [probe, setProbe] = useState("");
  const [probeResult, setProbeResult] = useState("");
  const submit = async () => {
    if (!l.name.trim()) {
      setErrs({ form: [], name: "名称不能为空" });
      return;
    }
    const p = Number(port);
    if (!Number.isInteger(p) || p < 1 || p > 65535) {
      setError("端口需在 1–65535 之间");
      return;
    }
    let destination: ExitKind = "Direct";
    if (mode !== "direct") {
      if (kind === "Node") {
        if (!node) {
          setError("请选择节点");
          return;
        }
        destination = { Node: { node } };
      } else {
        if (!group.members.length) {
          setError("请选择策略成员");
          return;
        }
        if (
          group.strategy === "Select" &&
          group.selected &&
          m &&
          !expandMembers(m, group.members).some(
            (r) => nodeKey(r) === nodeKey(group.selected!),
          )
        ) {
          setError("当前节点需属于策略组成员");
          return;
        }
        destination = { Group: group };
      }
    }
    const next = {
      ...l,
      name: l.name.trim(),
      listen: allowLan
        ? ipv6Listen
          ? "::"
          : "0.0.0.0"
        : ipv6Listen
          ? "::1"
          : "127.0.0.1",
      port: p,
      routing: mode === "rules" ? { rules, unmatched } : null,
    };
    try {
      await save.mutateAsync(next, { ...target, kind: destination });
      onSaved?.({ ...next, exit: `proxy-${l.id}` });
      onClose();
    } catch (e) {
      if (isValidationError(e)) setErrs(fieldErrors(e.errors));
      else setError(errorMessage(e));
    }
  };
  const verify = () => {
    const input = probe.trim().toLowerCase();
    const match = rules.find((r) => {
      const v = r.value.toLowerCase();
      if (r.matcher === "Domain") return input === v;
      if (r.matcher === "DomainSuffix")
        return input === v || input.endsWith(`.${v}`);
      const [ip, prefix] = v.split("/");
      const num = (s: string) =>
        s.split(".").reduce((a, b) => (a * 256 + Number(b)) >>> 0, 0);
      const bits = Number(prefix);
      return (
        /^\d+\.\d+\.\d+\.\d+$/.test(input) &&
        (bits === 0 || num(input) >>> (32 - bits) === num(ip) >>> (32 - bits))
      );
    });
    setProbeResult(
      `${match ? "匹配 " + match.value : "未匹配规则"} → ${actions.find(([a]) => a === (match?.action ?? unmatched))?.[1]}`,
    );
  };
  return (
    <>
      <FormDialog onClose={onClose} fullWidth maxWidth="sm">
        <DialogTitle>
          {m?.listeners.some((x) => x.id === l.id) ? "编辑代理" : "新增代理"}
        </DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 1 }}>
            {error && <Alert severity="error">{error}</Alert>}
            {errs.form.map((msg) => (
              <Alert severity="error" key={msg}>
                {msg}
              </Alert>
            ))}
            <TextField
              label="名称"
              value={l.name}
              error={!!errs.name}
              helperText={errs.name}
              onChange={(e) => {
                setL({ ...l, name: e.target.value });
                setErrs({ ...errs, name: undefined });
              }}
            />
            <TextField
              label="端口"
              type="number"
              value={port}
              error={!!errs.port}
              helperText={errs.port}
              onChange={(e) => {
                setPort(e.target.value);
                setErrs({ ...errs, port: undefined });
                setError("");
              }}
            />
            <TextField
              select
              label="协议"
              value={l.listener_type}
              onChange={(e) =>
                setL({ ...l, listener_type: e.target.value as ListenerType })
              }
            >
              {[
                ["Mixed", "Mixed（HTTP + SOCKS5）"],
                ["Http", "HTTP"],
                ["Socks", "SOCKS5"],
              ].map(([v, t]) => (
                <MenuItem key={v} value={v}>
                  {t}
                </MenuItem>
              ))}
            </TextField>
            <Box>
              <Typography variant="body2" sx={{ mb: 1 }}>
                流量处理
              </Typography>
              <ToggleButtonGroup
                exclusive
                fullWidth
                value={mode}
                onChange={(_, v) => v && setMode(v)}
              >
                {[
                  ["rules", "规则分流"],
                  ["forward", "统一转发"],
                  ["direct", "直连"],
                ].map(([v, t]) => (
                  <ToggleButton key={v} value={v}>
                    {t}
                  </ToggleButton>
                ))}
              </ToggleButtonGroup>
            </Box>
            {mode !== "direct" && (
              <>
                <TextField
                  select
                  label="去向"
                  value={kind}
                  onChange={(e) => setKind(e.target.value as "Node" | "Group")}
                >
                  <MenuItem value="Node">单个节点</MenuItem>
                  <MenuItem value="Group">策略组</MenuItem>
                </TextField>
                {kind === "Node" ? (
                  <NodePicker value={node} onChange={setNode} />
                ) : (
                  <>
                    <TextField
                      select
                      label="策略"
                      value={group.strategy}
                      onChange={(e) =>
                        setGroup({
                          ...group,
                          strategy: e.target.value as GroupStrategy,
                        })
                      }
                    >
                      {[
                        ["UrlTest", "自动测速"],
                        ["Fallback", "故障转移"],
                        ["LoadBalance", "负载均衡"],
                        ["Select", "手动选择"],
                      ].map(([v, t]) => (
                        <MenuItem key={v} value={v}>
                          {t}
                        </MenuItem>
                      ))}
                    </TextField>
                    <MemberPicker
                      value={group.members}
                      onChange={(members) => setGroup({ ...group, members })}
                    />
                    {group.strategy === "Select" && (
                      <Box>
                        <Typography variant="body2" sx={{ mb: 1 }}>
                          已选节点
                        </Typography>
                        <Button
                          variant="outlined"
                          fullWidth
                          onClick={() => setChoosingNode(true)}
                        >
                          {group.selected && m
                            ? nodeLabel(m, group.selected)
                            : "选择节点"}
                        </Button>
                      </Box>
                    )}
                  </>
                )}
              </>
            )}
            {mode === "rules" && (
              <>
                <Typography variant="subtitle2">按顺序匹配</Typography>
                {rules.map((r, i) => (
                  <Box
                    key={i}
                    sx={{
                      p: 1.5,
                      border: 1,
                      borderColor: "divider",
                      borderRadius: 1,
                    }}
                  >
                    <Stack spacing={1}>
                      <TextField
                        select
                        label="匹配类型"
                        value={r.matcher}
                        onChange={(e) =>
                          setRules(
                            rules.map((x, j) =>
                              j === i
                                ? {
                                    ...x,
                                    matcher: e.target
                                      .value as PortRule["matcher"],
                                  }
                                : x,
                            ),
                          )
                        }
                      >
                        {[
                          ["Domain", "完整域名"],
                          ["DomainSuffix", "域名后缀"],
                          ["Ipv4Cidr", "IPv4 CIDR"],
                        ].map(([v, t]) => (
                          <MenuItem key={v} value={v}>
                            {t}
                          </MenuItem>
                        ))}
                      </TextField>
                      <TextField
                        label="匹配内容"
                        value={r.value}
                        onChange={(e) =>
                          setRules(
                            rules.map((x, j) =>
                              j === i
                                ? { ...x, value: e.target.value.trim() }
                                : x,
                            ),
                          )
                        }
                      />
                      <TextField
                        select
                        label="处理方式"
                        value={r.action}
                        onChange={(e) =>
                          setRules(
                            rules.map((x, j) =>
                              j === i
                                ? { ...x, action: e.target.value as RuleAction }
                                : x,
                            ),
                          )
                        }
                      >
                        {actions.map(([v, t]) => (
                          <MenuItem key={v} value={v}>
                            {t}
                          </MenuItem>
                        ))}
                      </TextField>
                      <Button
                        color="error"
                        onClick={() =>
                          setRules(rules.filter((_, j) => j !== i))
                        }
                      >
                        移除规则
                      </Button>
                    </Stack>
                  </Box>
                ))}
                <Button
                  onClick={() =>
                    setRules([
                      ...rules,
                      { matcher: "DomainSuffix", value: "", action: "Proxy" },
                    ])
                  }
                >
                  添加规则
                </Button>
                <TextField
                  select
                  label="未匹配去向"
                  value={unmatched}
                  onChange={(e) => setUnmatched(e.target.value as RuleAction)}
                >
                  {actions.map(([v, t]) => (
                    <MenuItem key={v} value={v}>
                      {t}
                    </MenuItem>
                  ))}
                </TextField>
                <Box component="details">
                  <summary>验证规则</summary>
                  <Stack spacing={1} sx={{ mt: 1 }}>
                    <TextField
                      label="域名或 IPv4 地址"
                      value={probe}
                      onChange={(e) => setProbe(e.target.value)}
                    />
                    <Button onClick={verify}>验证</Button>
                    {rules.some((r) => r.matcher === "Ipv4Cidr") && (
                      <Typography variant="caption" color="text.secondary">
                        域名解析后的 IP 规则请使用解析地址验证。
                      </Typography>
                    )}
                    <Typography>{probeResult}</Typography>
                  </Stack>
                </Box>
              </>
            )}
            <Box component="details">
              <summary>高级选项</summary>
              <Stack spacing={2} sx={{ mt: 2 }}>
                <FormControlLabel
                  label="启用代理"
                  control={
                    <Switch
                      checked={l.user_enabled}
                      onChange={(_, v) => setL({ ...l, user_enabled: v })}
                    />
                  }
                />
                <FormControlLabel
                  label="允许局域网"
                  control={
                    <Switch
                      checked={allowLan}
                      onChange={(_, v) =>
                        setL({
                          ...l,
                          listen: v
                            ? ipv6Listen
                              ? "::"
                              : "0.0.0.0"
                            : ipv6Listen
                              ? "::1"
                              : "127.0.0.1",
                        })
                      }
                    />
                  }
                />
                <InterfacePicker
                  label="出站网卡"
                  value={target.interface}
                  onChange={(value) =>
                    setTarget({ ...target, interface: value })
                  }
                />
                <FormControlLabel
                  label="启用认证"
                  control={
                    <Switch
                      checked={!!l.auth}
                      onChange={(_, v) =>
                        setL({
                          ...l,
                          auth: v ? { username: "", password: "" } : null,
                        })
                      }
                    />
                  }
                />
                {l.auth && (
                  <>
                    <TextField
                      label="用户名"
                      value={l.auth.username}
                      onChange={(e) =>
                        setL({
                          ...l,
                          auth: { ...l.auth!, username: e.target.value },
                        })
                      }
                    />
                    <TextField
                      label="密码"
                      type={showPw ? "text" : "password"}
                      slotProps={{
                        input: {
                          endAdornment: (
                            <Button
                              size="small"
                              onClick={() => setShowPw(!showPw)}
                            >
                              {showPw ? "隐藏" : "显示"}
                            </Button>
                          ),
                        },
                      }}
                      value={l.auth.password}
                      onChange={(e) =>
                        setL({
                          ...l,
                          auth: { ...l.auth!, password: e.target.value },
                        })
                      }
                    />
                  </>
                )}
              </Stack>
            </Box>
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={onClose}>取消</Button>
          <Button
            variant="contained"
            disabled={save.isPending}
            onClick={() => void submit()}
          >
            保存
          </Button>
        </DialogActions>
      </FormDialog>
      {choosingNode && (
        <ListenerNodeDialog
          exitId={target.id}
          listenerName={l.name || "新代理"}
          draft={{ members: group.members, selected: group.selected }}
          onSelect={(selected, members) => setGroup({ ...group, members, selected })}
          onClose={() => setChoosingNode(false)}
        />
      )}
    </>
  );
}

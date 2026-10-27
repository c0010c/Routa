import {
  Alert,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  FormControlLabel,
  FormGroup,
  FormLabel,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  Switch,
  TextField,
} from "@mui/material";
import { useEffect, useState } from "react";
import { isValidationError, useModel, useSaveManualNode } from "../api/client";
import { commands, type JsonValue, type ManualNode, type ManualNodeKind, type TailscaleOpts } from "../bindings";
import { fieldErrors, type FieldErrors } from "../lib/validation";

type Raw = Partial<{ [key in string]: JsonValue }>;
type FormType = "socks5" | "http";
export type ManualNodeMode = "form" | "yaml";

export const TAILSCALE_NOTICE = "已转为 Tailscale 实例，需要在 Tailscale 页重新登录";

export function newManualNode(groups: string[] = []): ManualNode {
  return { id: crypto.randomUUID(), name: "", groups, kind: { Proxy: { raw: { type: "socks5" } } } };
}

const isObj = (v: JsonValue | undefined): v is Raw => typeof v === "object" && v !== null && !Array.isArray(v);
const str = (v: JsonValue | undefined): string => (typeof v === "string" ? v : typeof v === "number" ? String(v) : "");

function rawOf(n: ManualNode): Raw {
  return "Proxy" in n.kind && isObj(n.kind.Proxy.raw) ? n.kind.Proxy.raw : {};
}

function isFormType(t: JsonValue | undefined): t is FormType {
  return t === "socks5" || t === "http";
}

/** Converts a pasted `type: tailscale` proxy; `state-dir` is managed by the app and dropped. */
export function tailscaleFromRaw(raw: Raw, name: string): TailscaleOpts {
  const s = (k: string) => (typeof raw[k] === "string" && (raw[k] as string).trim() !== "" ? (raw[k] as string) : null);
  return {
    hostname: s("hostname") ?? `mihomo-${name}`,
    exit_node: s("exit-node") ?? "auto:any",
    ephemeral: typeof raw.ephemeral === "boolean" ? raw.ephemeral : false,
    udp: typeof raw.udp === "boolean" ? raw.udp : true,
    auth_key: s("auth-key"),
    interface: null,
  };
}

interface Preview {
  text: string;
  error?: string;
  stripped: string[];
  raw?: Raw;
}

async function parse(text: string): Promise<Preview> {
  const r = await commands.parseProxyYaml(text);
  if (r.status === "error") return { text, error: r.error, stripped: [] };
  return { text, stripped: r.data.stripped, raw: isObj(r.data.raw) ? r.data.raw : {} };
}

const isTailscale = (raw: Raw | undefined) => str(raw?.type).trim().toLowerCase() === "tailscale";

export function ManualNodeDialog({
  node,
  mode: forcedMode,
  onClose,
}: {
  node: ManualNode;
  /** Defaults to the form for socks5/http nodes and YAML for anything else. */
  mode?: ManualNodeMode;
  onClose: () => void;
}) {
  const model = useModel();
  const save = useSaveManualNode();
  const base = rawOf(node);
  const mode: ManualNodeMode = forcedMode ?? (isFormType(base.type) ? "form" : "yaml");

  const [name, setName] = useState(node.name);
  const [groups, setGroups] = useState<string[]>(node.groups);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });
  const [busy, setBusy] = useState(false);

  // Form mode.
  const [type, setType] = useState<FormType>(isFormType(base.type) ? base.type : "socks5");
  const [server, setServer] = useState(str(base.server));
  const [port, setPort] = useState(str(base.port));
  const [username, setUsername] = useState(str(base.username));
  const [password, setPassword] = useState(str(base.password));
  const [udp, setUdp] = useState(base.udp === true);
  const [tls, setTls] = useState(base.tls === true);

  // YAML mode.
  const [text, setText] = useState(() =>
    mode === "yaml" && "Proxy" in node.kind && Object.keys(base).length > 1 ? JSON.stringify(base, null, 2) : "",
  );
  const [preview, setPreview] = useState<Preview | null>(null);

  useEffect(() => {
    if (mode !== "yaml" || text.trim() === "") {
      setPreview(null);
      return;
    }
    let live = true;
    const t = setTimeout(() => {
      void parse(text).then((p) => {
        if (live) setPreview(p);
      });
    }, 300);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [mode, text]);

  const portNum = Number(port);
  const portValid = port.trim() !== "" && Number.isInteger(portNum) && portNum >= 1 && portNum <= 65535;

  const formRaw = (finalName: string): Raw => {
    const raw: Raw = { ...base, name: finalName, type, server: server.trim(), port: portNum };
    if (username !== "") raw.username = username;
    else delete raw.username;
    if (password !== "") raw.password = password;
    else delete raw.password;
    if (type === "socks5") raw.udp = udp;
    else raw.tls = tls;
    return raw;
  };

  const submit = async () => {
    setErrs({ form: [] });
    let kind: ManualNodeKind;
    let finalName = name.trim();
    if (mode === "form") {
      kind = { Proxy: { raw: formRaw(finalName) } };
    } else {
      setBusy(true);
      const p = await parse(text);
      setBusy(false);
      setPreview(p);
      if (p.error || !p.raw) return;
      if (finalName === "") finalName = str(p.raw.name).trim();
      if (isTailscale(p.raw)) {
        // Make sure the user saw the conversion notice before it is saved.
        if (!(preview?.text === text && isTailscale(preview.raw))) return;
        kind = { Tailscale: tailscaleFromRaw(p.raw, finalName) };
      } else {
        kind = { Proxy: { raw: { ...p.raw, name: finalName } } };
      }
    }
    try {
      await save.mutateAsync({ ...node, name: finalName, groups, kind });
      onClose();
    } catch (e) {
      if (isValidationError(e)) setErrs(fieldErrors(e.errors));
    }
  };

  const toggleGroup = (id: string) => setGroups((gs) => (gs.includes(id) ? gs.filter((g) => g !== id) : [...gs, id]));

  const nodeGroups = model.data?.node_groups ?? [];
  const isEdit = model.data?.manual_nodes.some((n) => n.id === node.id);
  const canSave = mode === "form" ? name.trim() !== "" && server.trim() !== "" && portValid : text.trim() !== "";
  return (
    <Dialog open onClose={onClose} fullWidth maxWidth="sm">
      <DialogTitle>{isEdit ? "编辑节点" : "新建节点"}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 1 }}>
          {errs.form.length > 0 && (
            <Alert severity="error" role="alert">
              {errs.form.map((m) => (
                <div key={m}>{m}</div>
              ))}
            </Alert>
          )}
          <TextField
            size="small"
            label="名称"
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              setErrs((x) => ({ ...x, name: undefined }));
            }}
            error={!!errs.name}
            helperText={errs.name ?? (mode === "yaml" ? "留空则使用 YAML 中的 name" : undefined)}
          />
          {mode === "form" ? (
            <>
              <FormControl size="small">
                <InputLabel id="mn-type">类型</InputLabel>
                <Select labelId="mn-type" label="类型" value={type} onChange={(e) => setType(e.target.value as FormType)}>
                  <MenuItem value="socks5">socks5</MenuItem>
                  <MenuItem value="http">http</MenuItem>
                </Select>
              </FormControl>
              <TextField size="small" label="服务器" value={server} onChange={(e) => setServer(e.target.value)} />
              <TextField
                size="small"
                label="端口"
                value={port}
                onChange={(e) => setPort(e.target.value)}
                error={port !== "" && !portValid}
                helperText={port !== "" && !portValid ? "端口须为 1–65535" : undefined}
              />
              <TextField size="small" label="用户名" value={username} onChange={(e) => setUsername(e.target.value)} />
              <TextField
                size="small"
                label="密码"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
              />
              {type === "socks5" ? (
                <FormControlLabel control={<Switch checked={udp} onChange={(e) => setUdp(e.target.checked)} />} label="UDP" />
              ) : (
                <FormControlLabel control={<Switch checked={tls} onChange={(e) => setTls(e.target.checked)} />} label="TLS" />
              )}
            </>
          ) : (
            <>
              <TextField
                label="YAML"
                multiline
                minRows={6}
                value={text}
                onChange={(e) => setText(e.target.value)}
                placeholder={"- name: 节点\n  type: vmess\n  server: example.com\n  port: 443"}
                error={!!preview?.error}
                helperText={preview?.error ?? "一次粘贴一个节点，可以带开头的「- 」"}
                slotProps={{ htmlInput: { style: { fontFamily: "monospace" } } }}
              />
              {preview && preview.stripped.length > 0 && (
                <Alert severity="warning">以下字段会被忽略：{preview.stripped.join("、")}</Alert>
              )}
              {preview && isTailscale(preview.raw) && <Alert severity="info">{TAILSCALE_NOTICE}</Alert>}
            </>
          )}
          {nodeGroups.length > 0 && (
            <FormControl component="fieldset">
              <FormLabel component="legend">所属组</FormLabel>
              <FormGroup row>
                {nodeGroups.map((g) => (
                  <FormControlLabel
                    key={g.id}
                    control={<Checkbox checked={groups.includes(g.id)} onChange={() => toggleGroup(g.id)} />}
                    label={g.name}
                  />
                ))}
              </FormGroup>
            </FormControl>
          )}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>取消</Button>
        <Button variant="contained" disabled={busy || save.isPending || !canSave} onClick={() => void submit()}>
          保存
        </Button>
      </DialogActions>
    </Dialog>
  );
}

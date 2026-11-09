import {
  Alert,
  Button,
  Checkbox,
  Collapse,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  FormControlLabel,
  FormGroup,
  FormHelperText,
  FormLabel,
  IconButton,
  InputAdornment,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  Switch,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import ExpandLess from "@mui/icons-material/ExpandLess";
import ExpandMore from "@mui/icons-material/ExpandMore";
import InfoOutlined from "@mui/icons-material/InfoOutlined";
import LinkIcon from "@mui/icons-material/Link";
import { useEffect, useRef, useState } from "react";
import { isValidationError, useModel, useSaveManualNode } from "../api/client";
import { commands, type JsonValue, type ManualNode, type ManualNodeKind, type TailscaleOpts } from "../bindings";
import { fieldErrors, type FieldErrors } from "../lib/validation";
import {
  findDuplicateNode,
  isRealityVision,
  isSupportedFingerprint,
  MIHOMO_FINGERPRINTS,
  publicKeyError, shortIdError, splitHostPort } from "../lib/vlessReality";
import { FormDialog } from "../components/FormDialog";

type Raw = Partial<{ [key in string]: JsonValue }>;
type FormType = "socks5" | "http" | "vless";
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

/** The type the form shows for `raw`, or null when only YAML can show it. */
function formTypeOf(raw: Raw): FormType | null {
  if (raw.type === "socks5" || raw.type === "http") return raw.type;
  return isRealityVision(raw) ? "vless" : null;
}

const realityOf = (raw: Raw): Raw => (isObj(raw["reality-opts"]) ? raw["reality-opts"] : {});

/** Shown in the picker; the rest of MIHOMO_FINGERPRINTS only appear when a node already uses one. */
const FINGERPRINTS = MIHOMO_FINGERPRINTS.slice(0, 9);
const VISION = "xtls-rprx-vision";
const MLKEM = "support-x25519mlkem768";

/**
 * Xray share links never carry the ML-KEM flag, and Xray clients offer X25519MLKEM768 by
 * default, so servers built for them may refuse REALITY without it. Off only when stated.
 */
const mlkemOf = (raw: Raw): boolean => {
  const v = realityOf(raw)[MLKEM];
  return typeof v === "boolean" ? v : true;
};

type LinkState =
  | { kind: "idle" }
  | { kind: "error"; message: string }
  | { kind: "filled"; count: number; fpReplaced?: string }
  | { kind: "other"; raw: Raw; label: string };

const countLinks = (text: string) => text.split(/\r?\n/).filter((l) => l.trim() !== "").length;

function linkLabel(raw: Raw): string {
  const security = isObj(raw["reality-opts"]) ? " + reality" : raw.tls === true ? " + tls" : "";
  return `${str(raw.type)}${raw.network ? ` + ${str(raw.network)}` : ""}${security}`;
}

/**
 * Converts a pasted `type: tailscale` proxy; `state-dir` is managed by the app and dropped.
 * `ephemeral` always starts off: the app recreates the instance on every reload, and an
 * ephemeral machine removed from the tailnet meanwhile has to log in again.
 */
export function tailscaleFromRaw(raw: Raw, name: string): TailscaleOpts {
  const s = (k: string) => (typeof raw[k] === "string" && (raw[k] as string).trim() !== "" ? (raw[k] as string) : null);
  return {
    hostname: s("hostname") ?? `mihomo-${name}`,
    exit_node: s("exit-node") ?? "",
    ephemeral: false,
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

function Info({ title }: { title: string }) {
  return (
    <InputAdornment position="end">
      <Tooltip title={title}>
        <IconButton size="small" edge="end" aria-label={title} tabIndex={-1}>
          <InfoOutlined fontSize="small" />
        </IconButton>
      </Tooltip>
    </InputAdornment>
  );
}

export function ManualNodeDialog({
  node,
  mode: forcedMode,
  onClose,
}: {
  node: ManualNode;
  /** Defaults to the form for socks5/http/VLESS REALITY nodes and YAML for anything else. */
  mode?: ManualNodeMode;
  onClose: () => void;
}) {
  const model = useModel();
  const save = useSaveManualNode();
  const [base, setBase] = useState<Raw>(() => rawOf(node));
  const initialType = formTypeOf(base);
  const [mode, setMode] = useState<ManualNodeMode>(forcedMode ?? (initialType ? "form" : "yaml"));

  const [name, setName] = useState(node.name);
  const [groups, setGroups] = useState<string[]>(node.groups);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });
  const [busy, setBusy] = useState(false);

  // Form mode.
  const [type, setType] = useState<FormType>(initialType ?? "socks5");
  const [server, setServer] = useState(str(base.server));
  const [port, setPort] = useState(str(base.port));
  const [username, setUsername] = useState(str(base.username));
  const [password, setPassword] = useState(str(base.password));
  const [udp, setUdp] = useState(base.udp === true);
  const [tls, setTls] = useState(base.tls === true);

  // VLESS + REALITY.
  const [uuid, setUuid] = useState(str(base.uuid));
  const [sni, setSni] = useState(str(base.servername));
  const [pbk, setPbk] = useState(str(realityOf(base)["public-key"]));
  const [sid, setSid] = useState(str(realityOf(base)["short-id"]));
  const [fp, setFp] = useState(str(base["client-fingerprint"]) || "chrome");
  const [vision, setVision] = useState(initialType === "vless" ? base.flow === VISION : true);
  const [vUdp, setVUdp] = useState(base.udp !== false);
  // An existing node keeps mihomo's default (off) unless it says otherwise.
  const [mlkem, setMlkem] = useState(initialType === "vless" ? realityOf(base)[MLKEM] === true : true);
  const [advanced, setAdvanced] = useState(false);

  // Share link. `autoName` is the name the last link filled in, so a newer link may replace it.
  const [link, setLink] = useState("");
  const [linkState, setLinkState] = useState<LinkState>({ kind: "idle" });
  const nameRef = useRef({ name, autoName: null as string | null });
  nameRef.current.name = name;

  const fillFromLink = (raw: Raw, count: number) => {
    setMode("form");
    setType("vless");
    setBase(raw);
    setServer(str(raw.server));
    setPort(str(raw.port));
    setUuid(str(raw.uuid));
    setSni(str(raw.servername));
    setPbk(str(realityOf(raw)["public-key"]));
    setSid(str(realityOf(raw)["short-id"]));
    const linkFp = str(raw["client-fingerprint"]);
    const fpOk = linkFp === "" || isSupportedFingerprint(linkFp);
    setFp(fpOk ? linkFp || "chrome" : "chrome");
    setVision(raw.flow === VISION);
    setVUdp(raw.udp !== false);
    setMlkem(mlkemOf(raw));
    const linkName = str(raw.name).trim();
    const cur = nameRef.current;
    if (linkName !== "" && (cur.name.trim() === "" || cur.name === cur.autoName)) {
      setName(linkName);
      cur.autoName = linkName;
    }
    setLinkState({ kind: "filled", count, fpReplaced: fpOk ? undefined : linkFp });
  };

  useEffect(() => {
    if (link.trim() === "") {
      setLinkState({ kind: "idle" });
      return;
    }
    let live = true;
    const t = setTimeout(() => {
      void commands.parseShareLink(link).then((r) => {
        if (!live) return;
        if (r.status === "error") {
          setLinkState({ kind: "error", message: r.error });
          return;
        }
        const raw = isObj(r.data.raw) ? r.data.raw : {};
        if (isRealityVision(raw)) fillFromLink(raw, countLinks(link));
        else setLinkState({ kind: "other", raw, label: linkLabel(raw) });
      });
    }, 300);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [link]);

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
  const pbkError = publicKeyError(pbk);
  const sidError = shortIdError(sid);

  const vlessRaw = (finalName: string): Raw => {
    const raw: Raw = {
      ...(base.type === "vless" ? base : {}),
      name: finalName,
      type: "vless",
      server: server.trim(),
      port: portNum,
      uuid: uuid.trim(),
      network: "tcp",
      tls: true,
      udp: vUdp,
      servername: sni.trim(),
      "client-fingerprint": fp,
      "reality-opts": { ...realityOf(base), "public-key": pbk.trim(), "short-id": sid.trim(), [MLKEM]: mlkem },
    };
    if (vision) raw.flow = VISION;
    else delete raw.flow;
    return raw;
  };

  const formRaw = (finalName: string): Raw => {
    if (type === "vless") return vlessRaw(finalName);
    // Fields of another type (say, from a pasted vless link) do not carry over.
    const raw: Raw = { ...(base.type === type ? base : {}), name: finalName, type, server: server.trim(), port: portNum };
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

  const onServer = (v: string) => {
    const hp = splitHostPort(v);
    if (hp) {
      setServer(hp.host);
      setPort(hp.port);
    } else setServer(v);
  };

  const openAsYaml = (raw: Raw) => {
    setMode("yaml");
    setText(JSON.stringify(raw, null, 2));
    setLink("");
  };

  const toggleGroup = (id: string) => setGroups((gs) => (gs.includes(id) ? gs.filter((g) => g !== id) : [...gs, id]));

  const nodeGroups = model.data?.node_groups ?? [];
  const isEdit = model.data?.manual_nodes.some((n) => n.id === node.id);
  const formOk =
    name.trim() !== "" &&
    server.trim() !== "" &&
    portValid &&
    (type !== "vless" || (uuid.trim() !== "" && sni.trim() !== "" && pbk.trim() !== "" && !pbkError && !sidError));
  const canSave = mode === "form" ? formOk : text.trim() !== "";
  const duplicate =
    mode === "form" && type === "vless" && portValid && uuid.trim() !== ""
      ? findDuplicateNode(model.data?.manual_nodes ?? [], node.id, server.trim(), portNum, uuid.trim())
      : undefined;

  const linkHelp =
    linkState.kind === "error"
      ? linkState.message
      : linkState.kind === "filled"
        ? "已填写，确认后保存"
        : linkState.kind === "idle"
          ? "支持 vless、vmess、ss、trojan 等，链接本身不会保存"
          : undefined;

  return (
    <FormDialog onClose={onClose} fullWidth maxWidth="sm">
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
          <Stack spacing={0.5}>
            <TextField
              size="small"
              label="分享链接"
              // Multiline so a pasted multi-line block keeps its line breaks.
              multiline
              maxRows={3}
              placeholder="有分享链接？粘贴到这里自动填写"
              value={link}
              onChange={(e) => setLink(e.target.value)}
              error={linkState.kind === "error"}
              helperText={linkHelp}
              slotProps={{
                input: {
                  startAdornment: (
                    <InputAdornment position="start">
                      <LinkIcon fontSize="small" />
                    </InputAdornment>
                  ),
                },
                htmlInput: { style: { fontFamily: "monospace" } },
              }}
            />
            {linkState.kind === "filled" && linkState.count > 1 && (
              <Typography variant="caption" color="warning.main">
                共 {linkState.count} 条，只用了第一条。多条请用「导入 YAML」
              </Typography>
            )}
            {linkState.kind === "filled" && linkState.fpReplaced && (
              <Typography variant="caption" color="warning.main">
                链接里的指纹 {linkState.fpReplaced} mihomo 不支持，已改为 chrome
              </Typography>
            )}
            {linkState.kind === "other" && (
              <Alert
                severity="info"
                action={
                  <Button size="small" onClick={() => openAsYaml(linkState.raw)}>
                    用 YAML 打开
                  </Button>
                }
              >
                这是 {linkState.label} 节点，表单只支持 VLESS + REALITY
              </Alert>
            )}
          </Stack>
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
                  <MenuItem value="vless">VLESS + REALITY</MenuItem>
                </Select>
              </FormControl>
              <TextField size="small" label="服务器" value={server} onChange={(e) => onServer(e.target.value)} />
              <TextField
                size="small"
                label="端口"
                value={port}
                onChange={(e) => setPort(e.target.value)}
                error={port !== "" && !portValid}
                helperText={port !== "" && !portValid ? "端口须为 1–65535" : undefined}
              />
              {type === "vless" ? (
                <>
                  <TextField
                    size="small"
                    label="UUID"
                    value={uuid}
                    onChange={(e) => setUuid(e.target.value)}
                    slotProps={{ htmlInput: { style: { fontFamily: "monospace" } } }}
                  />
                  <TextField
                    size="small"
                    label="SNI"
                    value={sni}
                    onChange={(e) => setSni(e.target.value)}
                    slotProps={{ input: { endAdornment: <Info title="服务端 REALITY 配置 serverNames 中的一个（链接中的 sni）" /> } }}
                  />
                  <TextField
                    size="small"
                    label="Public Key"
                    value={pbk}
                    onChange={(e) => setPbk(e.target.value)}
                    error={!!pbkError}
                    helperText={pbkError}
                    slotProps={{
                      input: {
                        endAdornment: <Info title="xray x25519 输出的 Public key（链接中的 pbk），不是服务端配置里的 privateKey" />,
                      },
                      htmlInput: { style: { fontFamily: "monospace" } },
                    }}
                  />
                  <TextField
                    size="small"
                    label="Short ID"
                    placeholder="可留空"
                    value={sid}
                    onChange={(e) => setSid(e.target.value)}
                    error={!!sidError}
                    helperText={sidError}
                    slotProps={{ htmlInput: { style: { fontFamily: "monospace" } } }}
                  />
                  <Button
                    size="small"
                    color="inherit"
                    sx={{ alignSelf: "flex-start" }}
                    endIcon={advanced ? <ExpandLess /> : <ExpandMore />}
                    onClick={() => setAdvanced((a) => !a)}
                  >
                    {`高级 · ${fp}${vision ? " · Vision" : ""}${vUdp ? " · UDP" : ""}${mlkem ? " · ML-KEM" : ""}`}
                  </Button>
                  <Collapse in={advanced}>
                    <Stack spacing={1}>
                      <FormControl size="small">
                        <InputLabel id="mn-fp">客户端指纹</InputLabel>
                        <Select labelId="mn-fp" label="客户端指纹" value={fp} onChange={(e) => setFp(e.target.value)}>
                          {(FINGERPRINTS.includes(fp) ? FINGERPRINTS : [fp, ...FINGERPRINTS]).map((f) => (
                            <MenuItem key={f} value={f}>
                              {f}
                            </MenuItem>
                          ))}
                        </Select>
                        {!isSupportedFingerprint(fp) && (
                          <FormHelperText error>mihomo 不支持 {fp}，指纹不会生效，请换一个</FormHelperText>
                        )}
                      </FormControl>
                      <FormGroup row>
                        <FormControlLabel
                          control={<Switch checked={vision} onChange={(e) => setVision(e.target.checked)} />}
                          label="XTLS Vision 流控"
                        />
                        <FormControlLabel
                          control={<Switch checked={vUdp} onChange={(e) => setVUdp(e.target.checked)} />}
                          label="UDP"
                        />
                        <FormControlLabel
                          control={<Switch checked={mlkem} onChange={(e) => setMlkem(e.target.checked)} />}
                          label="后量子密钥交换（X25519MLKEM768）"
                        />
                      </FormGroup>
                    </Stack>
                  </Collapse>
                  {duplicate && <Alert severity="warning">已有相同节点「{duplicate.name}」，仍可保存</Alert>}
                </>
              ) : (
                <>
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
    </FormDialog>
  );
}

import ExpandLessIcon from "@mui/icons-material/ExpandLess";
import ExpandMoreIcon from "@mui/icons-material/ExpandMore";
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
  Radio,
  RadioGroup,
  Stack,
  Step,
  StepLabel,
  Stepper,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import { useEffect, useState } from "react";
import { isValidationError, useModel, useSaveManualNode } from "../api/client";
import type { ManualNode, TailscaleOpts } from "../bindings";
import { ExitNodeField } from "../components/ExitNodeField";
import { FormDialog } from "../components/FormDialog";
import { InterfacePicker } from "../components/InterfacePicker";
import {
  CHECK_TIMEOUT_MS,
  EXIT_NODE_MAX_RETRIES,
  ExitNodeIssueAlert,
  TailscaleLoginLink,
  TailscaleStateChip,
  type CheckState,
  useTailscaleCheck,
  useTailscaleState,
} from "../components/TailscaleState";
import { delayFailure } from "../lib/format";
import { isAutoExitNode } from "../lib/tailnet";
import { fieldErrors, type FieldErrors } from "../lib/validation";
import { ListenerDialog, newListener } from "./ListenerDialog";

export const EPHEMERAL_HELP = "建议关闭：临时节点离线一段时间后会被 tailnet 自动删除，修改配置或重启内核后可能需要重新登录";
export const HOSTNAME_CHANGE_HELP = "修改后会在 tailnet 中注册为一台新机器，需要重新登录";
export const NO_EXIT_NODE_HELP = "不使用出口节点时只能访问 tailnet 内的设备，访问公网的测速会失败，所以不做测速。";

const defaultHostname = (name: string) => `mihomo-${name}`;

export function newTailscaleNode(groups: string[] = []): ManualNode {
  const opts: TailscaleOpts = {
    hostname: defaultHostname(""),
    exit_node: "",
    ephemeral: false,
    udp: true,
    auth_key: null,
    interface: null,
  };
  return { id: crypto.randomUUID(), name: "", groups, kind: { Tailscale: opts } };
}

const optsOf = (n: ManualNode): TailscaleOpts =>
  "Tailscale" in n.kind ? n.kind.Tailscale : (newTailscaleNode().kind as { Tailscale: TailscaleOpts }).Tailscale;

const STEPS = ["命名", "登录", "出口节点与验证"];

/**
 * New instance: a wizard (name → login → exit node and check). Existing
 * instance: one form. "用于新代理" swaps the dialog for the exit dialog.
 */
export function TailscaleDialog({ node, onClose }: { node: ManualNode; onClose: () => void }) {
  const model = useModel();
  const save = useSaveManualNode();
  // Fixed at open: the wizard's first step saves the node, which makes it "existing".
  const [isNew] = useState(() => !model.data?.manual_nodes.some((n) => n.id === node.id));
  const initial = optsOf(node);

  const [step, setStep] = useState(0);
  const [name, setName] = useState(node.name);
  const [hostname, setHostname] = useState(initial.hostname);
  // The hostname tracks the name until the user edits it by hand.
  // Existing instances never change hostname implicitly: a new hostname registers as a new tailnet machine.
  const [hostTouched, setHostTouched] = useState(initial.hostname !== defaultHostname(node.name));
  const [exitNode, setExitNode] = useState(initial.exit_node);
  const [ephemeral, setEphemeral] = useState(initial.ephemeral);
  const [udp, setUdp] = useState(initial.udp);
  const [useAuthKey, setUseAuthKey] = useState(initial.auth_key !== null);
  const [authKey, setAuthKey] = useState(initial.auth_key ?? "");
  const [iface, setIface] = useState(initial.interface);
  const [groups, setGroups] = useState<string[]>(node.groups);
  const [advanced, setAdvanced] = useState(false);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });
  const [creatingExit, setCreatingExit] = useState(false);
  // The exit node last saved by the wizard (its first step saves none).
  const [applied, setApplied] = useState("");

  const build = (exit: string): ManualNode => ({
    ...node,
    name: name.trim(),
    groups,
    kind: {
      Tailscale: {
        hostname: hostname.trim(),
        exit_node: exit.trim(),
        ephemeral,
        udp,
        auth_key: useAuthKey && authKey.trim() !== "" ? authKey.trim() : null,
        interface: iface,
      },
    },
  });
  const current = build(exitNode);
  const check = useTailscaleCheck(current);
  const loginState = useTailscaleState(current);
  const loggedIn = loginState !== "Unknown" && "LoggedIn" in loginState;

  // The login step moves on by itself once the instance reports it is logged in.
  useEffect(() => {
    if (isNew && step === 1 && loggedIn) setStep(2);
  }, [isNew, step, loggedIn]);

  const persist = async (n: ManualNode): Promise<boolean> => {
    setErrs({ form: [] });
    try {
      await save.mutateAsync(n);
      return true;
    } catch (e) {
      if (isValidationError(e)) setErrs(fieldErrors(e.errors));
      return false;
    }
  };

  const changeName = (v: string) => {
    setName(v);
    if (!hostTouched && isNew) setHostname(defaultHostname(v));
    setErrs((x) => ({ ...x, name: undefined }));
  };
  const toggleGroup = (id: string) => setGroups((gs) => (gs.includes(id) ? gs.filter((g) => g !== id) : [...gs, id]));

  if (creatingExit) {
    return <ListenerDialog listener={{...newListener(model.data?.listeners ?? []), name: current.name}} initialNode={{manual_id: node.id}} onClose={()=>setCreatingExit(false)} />;
  }

  const nodeGroups = model.data?.node_groups ?? [];
  const fieldsOk = name.trim() !== "" && hostname.trim() !== "" && (!useAuthKey || authKey.trim() !== "");
  const errorAlert = errs.form.length > 0 && (
    <Alert severity="error" role="alert">
      {errs.form.map((m) => (
        <div key={m}>{m}</div>
      ))}
    </Alert>
  );

  const instanceFields = (
    <>
      <TextField
        size="small"
        label="名称"
        value={name}
        onChange={(e) => changeName(e.target.value)}
        error={!!errs.name}
        helperText={errs.name ?? (name.trim() && hostname.trim() ? `在 tailnet 中显示为 ${hostname.trim()}` : "例如：家里、公司")}
      />
      <FormControl>
        <FormLabel id="ts-login-method">登录方式</FormLabel>
        <RadioGroup row aria-labelledby="ts-login-method" value={useAuthKey ? "key" : "browser"} onChange={(e) => setUseAuthKey(e.target.value === "key")}>
          <FormControlLabel value="browser" control={<Radio size="small" />} label="浏览器授权（推荐）" />
          <FormControlLabel value="key" control={<Radio size="small" />} label="使用 auth key" />
        </RadioGroup>
      </FormControl>
      {useAuthKey && (
        <TextField
          size="small"
          label="auth key"
          type="password"
          value={authKey}
          onChange={(e) => setAuthKey(e.target.value)}
          helperText="在 Tailscale 管理后台的 Settings → Keys 中生成"
          autoComplete="off"
        />
      )}
      <Button
        size="small"
        onClick={() => setAdvanced((a) => !a)}
        endIcon={advanced ? <ExpandLessIcon /> : <ExpandMoreIcon />}
        sx={{ alignSelf: "flex-start" }}
        aria-expanded={advanced}
      >
        高级选项
      </Button>
      <Collapse in={advanced} unmountOnExit>
        <Stack spacing={2}>
          <TextField
            size="small"
            label="tailnet 中的机器名"
            value={hostname}
            onChange={(e) => {
              setHostname(e.target.value);
              setHostTouched(true);
            }}
            helperText={isNew ? "默认跟随名称" : HOSTNAME_CHANGE_HELP}
          />
          <InterfacePicker value={iface} onChange={setIface} />
          <FormControlLabel control={<Switch checked={udp} onChange={(e) => setUdp(e.target.checked)} />} label="转发 UDP" />
          <FormControl>
            <FormControlLabel control={<Switch checked={ephemeral} onChange={(e) => setEphemeral(e.target.checked)} />} label="临时节点（ephemeral）" />
            <FormHelperText sx={{ mt: 0 }}>{EPHEMERAL_HELP}</FormHelperText>
          </FormControl>
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
      </Collapse>
    </>
  );

  if (!isNew) {
    const canSave = fieldsOk && !isAutoExitNode(exitNode);
    return (
      <FormDialog onClose={onClose} fullWidth maxWidth="sm">
        <DialogTitle>编辑 Tailscale 实例</DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ mt: 1 }}>
            {errorAlert}
            {instanceFields}
            <ExitNodeField value={exitNode} onChange={setExitNode} />
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={onClose}>取消</Button>
          <Button
            variant="contained"
            disabled={save.isPending || !canSave}
            onClick={async () => {
              if (await persist(current)) onClose();
            }}
          >
            保存
          </Button>
        </DialogActions>
      </FormDialog>
    );
  }

  return (
    <FormDialog onClose={onClose} fullWidth maxWidth="sm">
      <DialogTitle>新建 Tailscale 实例</DialogTitle>
      <DialogContent>
        <Stepper activeStep={step} sx={{ mb: 3, mt: 1 }}>
          {STEPS.map((s) => (
            <Step key={s}>
              <StepLabel>{s}</StepLabel>
            </Step>
          ))}
        </Stepper>
        <Stack spacing={2}>
          {errorAlert}
          {step === 0 && instanceFields}
          {step === 1 && (
            <>
              <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
                <TailscaleStateChip node={current} />
                <Typography variant="body2" color="text.secondary">
                  {useAuthKey ? "正在使用 auth key 登录…" : "实例启动后会给出授权链接，请在浏览器中完成授权"}
                </Typography>
              </Stack>
              <TailscaleLoginLink node={current} hideChip />
              <Typography variant="body2" color="text.secondary">
                授权完成后自动进入下一步。也可以先关闭窗口，稍后在 Tailscale 页继续登录。
              </Typography>
            </>
          )}
          {step === 2 && (
            <>
              <TailscaleStateChip node={current} />
              <ExitNodeField value={exitNode} onChange={setExitNode} />
              {exitNode.trim() === "" ? (
                <Alert severity="info">{NO_EXIT_NODE_HELP}</Alert>
              ) : (
                applied === exitNode.trim() && <CheckResult state={check.state} />
              )}
            </>
          )}
        </Stack>
      </DialogContent>
      <DialogActions>
        {step === 0 && (
          <>
            <Button onClick={onClose}>取消</Button>
            <Button
              variant="contained"
              disabled={save.isPending || !fieldsOk}
              onClick={async () => {
                if (await persist(build(""))) setStep(1);
              }}
            >
              保存并登录
            </Button>
          </>
        )}
        {step === 1 && <Button onClick={onClose}>稍后登录</Button>}
        {step === 2 && (
          <WizardFinish
            exitNode={exitNode}
            check={applied === exitNode.trim() ? check.state : { phase: "idle" }}
            busy={save.isPending || check.state.phase === "running"}
            onApply={async () => {
              if (!(await persist(current))) return;
              setApplied(exitNode.trim());
              await check.run();
            }}
            onFinish={async (then) => {
              // Switching back to "none" after applying an exit node still has to be saved.
              if (exitNode.trim() !== applied && !(await persist(current))) return;
              if (then === "exit") setCreatingExit(true);
              else onClose();
            }}
          />
        )}
      </DialogActions>
    </FormDialog>
  );
}

function WizardFinish(props: {
  exitNode: string;
  check: CheckState;
  busy: boolean;
  onApply: () => Promise<void>;
  onFinish: (then: "close" | "exit") => Promise<void>;
}) {
  const none = props.exitNode.trim() === "";
  const passed = props.check.phase === "done" && props.check.result.ms !== null;
  const tested = props.check.phase === "done";
  if (none || passed) {
    return (
      <>
        <Button disabled={props.busy} onClick={() => void props.onFinish("close")}>
          完成
        </Button>
        <Button variant="contained" disabled={props.busy} onClick={() => void props.onFinish("exit")}>
          用于新代理
        </Button>
      </>
    );
  }
  return (
    <>
      <Button onClick={() => void props.onFinish("close")}>{tested ? "先关闭" : "稍后设置"}</Button>
      <Button variant="contained" disabled={props.busy || isAutoExitNode(props.exitNode)} onClick={() => void props.onApply()}>
        {tested ? "重新应用并测速" : "应用并测速"}
      </Button>
    </>
  );
}

function CheckResult({ state }: { state: CheckState }) {
  if (state.phase === "idle") return null;
  if (state.phase === "running") {
    return (
      <Alert severity="info">
        正在验证（{state.elapsedSecs} 秒）…实例刚重新启动，通常要半分钟到一分钟才能经出口节点上网，最多等 {CHECK_TIMEOUT_MS / 1000} 秒
        {state.retrying !== null && `。出口节点这次没设置上，正在自动重启实例重试（第 ${state.retrying}/${EXIT_NODE_MAX_RETRIES} 次）`}
      </Alert>
    );
  }
  if (state.issue) return <ExitNodeIssueAlert issue={state.issue} withRetry={false} />;
  if (state.result.ms !== null) return <Alert severity="success">出口节点已生效，测速 {state.result.ms} ms</Alert>;
  const f = delayFailure(state.result.error);
  return (
    <Alert severity="error">
      测速{f.text === "超时" ? "超时" : `失败（${f.text}）`}：{f.detail}。请确认出口节点在线，稍后可在 Tailscale 页重新测速。
    </Alert>
  );
}

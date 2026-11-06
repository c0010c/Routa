import InsertDriveFileOutlined from "@mui/icons-material/InsertDriveFileOutlined";
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Chip,
  createFilterOptions,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  FormControlLabel,
  FormHelperText,
  FormLabel,
  Radio,
  RadioGroup,
  Stack,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import { useState } from "react";
import { errorMessage, isValidationError, useModel } from "../api/client";
import { useRunningProcesses, useSaveProcessRuleConfirmed } from "../api/procroute";
import type { FaultPolicy, ProcessRule, RunningProcess, ValidationError } from "../bindings";
import { FormDialog } from "../components/FormDialog";
import { InterfacePicker } from "../components/InterfacePicker";
import { NodePickerDialog } from "../components/NodePickerDialog";
import { fileName, ruleLabel } from "../lib/processRule";
import { isTailscaleRef, nodeLabel } from "../lib/refs";
import { fieldErrors } from "../lib/validation";

export const FAULT_POLICY_HINT = "只在 mihomo 或分流组件意外故障时生效；节点失效、网卡断开时，始终阻断";
export const RESET_HINT = "仅对 IPv4 TCP 连接有效；更换代理目标不触发断开";
export const NAME_MATCH_HINT = "按进程名匹配：同名程序装在哪里都会被分流；组件故障时只能直连。需要更严格时，在高级选项里打开「按完整路径匹配」。";
export const FULL_PATH_HINT = "关闭时只看进程名，同名程序装在哪里都会命中；打开后只命中指定路径下的那个程序，组件故障时也能阻断。";

interface RuleErrors {
  processes?: string;
  node?: string;
  form: string[];
}

/** Backend validation errors placed on the dialog's fields; the wording comes from `fieldErrors`. */
function ruleErrors(errors: ValidationError[]): RuleErrors {
  const out: RuleErrors = { form: [] };
  for (const e of errors) {
    const text = fieldErrors([e]);
    const msg = [...(text.name ? [text.name] : []), ...(text.port ? [text.port] : []), ...text.form].join("\n");
    if ("DuplicateProcessRule" in e || "InvalidExePath" in e || "EmptyProcessRule" in e) out.processes ??= msg;
    else out.form.push(msg);
  }
  return out;
}

const filterProcesses = createFilterOptions<RunningProcess>({ stringify: (p) => `${p.name} ${p.exe_path}` });
const isPath = (p: string) => /[\\/:]/.test(p);
const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

export function ProcessRuleDialog({ open, rule, onClose }: { open: boolean; rule?: ProcessRule; onClose: () => void }) {
  return open ? <RuleForm rule={rule} onClose={onClose} /> : null;
}

function RuleForm({ rule, onClose }: { rule?: ProcessRule; onClose: () => void }) {
  const model = useModel();
  const running = useRunningProcesses();
  // The dialog stays open on failure and shows the reason itself.
  const save = useSaveProcessRuleConfirmed({ errorFeedback: "local" });
  const others = (model.data?.process_routing.rules ?? []).filter((x) => x.id !== rule?.id);
  // Mirrors `ProcessRule::default`.
  const [r, setR] = useState<ProcessRule>(
    () =>
      rule ?? {
        id: crypto.randomUUID(),
        name: "",
        processes: [],
        match_full_path: false,
        node: null,
        interface: null,
        on_fault: "Direct",
        reset_on_change: false,
        user_enabled: true,
      },
  );
  const [input, setInput] = useState("");
  const [advanced, setAdvanced] = useState(() => !!rule?.match_full_path);
  const [errs, setErrs] = useState<RuleErrors>({ form: [] });
  const [picking, setPicking] = useState(false);
  const m = model.data;
  const node = r.node ?? null;
  const tailscale = !!m && !!node && isTailscaleRef(m, node);
  /** A Tailscale instance has its own NIC (set in its node settings); the rule's value doesn't apply. */
  const tailscaleNic = (() => {
    if (!m || !node || !tailscale || !("manual_id" in node)) return null;
    const kind = m.manual_nodes.find((x) => x.id === node.manual_id)?.kind;
    return kind && typeof kind === "object" && "Tailscale" in kind ? kind.Tailscale.interface : null;
  })();
  const runningPath = (name: string) => running.data?.find((p) => same(p.name, name))?.exe_path;
  /** The rule that already has a program with this file name (one program belongs to one rule). */
  const ownerOf = (p: string) => others.find((x) => x.processes.some((q) => same(fileName(q), fileName(p))));
  const missingPaths = r.match_full_path ? r.processes.filter((p) => !isPath(p)) : [];

  /** Turns one typed or picked entry into what the rule stores, or says why it can't be added. */
  const prepare = (raw: string): { process?: string; error?: string } => {
    let p = raw.trim().replace(/^"|"$/g, "").replace(/\//g, "\\");
    if (!p) return {};
    if (r.match_full_path && !isPath(p)) {
      const path = runningPath(p);
      if (!path) return { error: `已开启按完整路径匹配，请填写 ${p} 的完整路径，或从正在运行的程序中选择` };
      p = path;
    }
    if (!r.match_full_path) p = fileName(p);
    if (!/\.exe$/i.test(p)) return { error: `${p} 不是 .exe 程序` };
    if (r.processes.some((q) => same(fileName(q), fileName(p)))) return { error: `${fileName(p)} 已经在这条规则里` };
    const owner = ownerOf(p);
    if (owner) return { error: `${fileName(p)} 已在规则「${ruleLabel(owner)}」中，一个程序只能属于一条规则` };
    return { process: p };
  };
  /** Adds one entry; returns an error message instead when it can't be added. */
  const add = (raw: string): string | undefined => {
    const { process, error } = prepare(raw);
    if (process) setR((x) => ({ ...x, processes: [...x.processes, process] }));
    return error;
  };

  const setFullPath = (on: boolean) =>
    setR((x) => ({
      ...x,
      match_full_path: on,
      // Name matching can't block on a component fault (the firewall only knows full paths).
      on_fault: on ? x.on_fault : "Direct",
      processes: x.processes.map((p) => (on ? (isPath(p) ? p : (runningPath(p) ?? p)) : fileName(p))),
    }));

  const submit = async () => {
    // Text still in the box counts as one more process.
    const pending = prepare(input);
    if (pending.error) {
      setErrs({ form: [], processes: pending.error });
      return;
    }
    const processes = pending.process ? [...r.processes, pending.process] : r.processes;
    if (processes.length === 0) {
      setErrs({ form: [], processes: "至少添加一个进程" });
      return;
    }
    if (!node) {
      setErrs({ form: [], node: "请选择一个节点" });
      return;
    }
    setErrs({ form: [] });
    try {
      await save.mutateAsync({ ...r, name: r.name.trim(), processes, interface: tailscale ? null : (r.interface ?? null) });
      onClose();
    } catch (e) {
      setErrs(isValidationError(e) ? ruleErrors(e.errors) : { form: [errorMessage(e)] });
    }
  };

  const isEdit = rule !== undefined;
  const processHelp =
    errs.processes ??
    (missingPaths.length > 0
      ? `${missingPaths.join("、")} 没在运行，找不到完整路径：请删掉后粘贴完整路径重新添加`
      : r.match_full_path
        ? "按完整路径匹配。可添加多个，回车添加，也可以从正在运行的程序中选择。"
        : "可添加多个，输入进程名后回车，也可以从正在运行的程序中选择。");
  return (
    <FormDialog onClose={onClose} fullWidth maxWidth="sm">
      <DialogTitle>{isEdit ? "编辑分流规则" : "新建分流规则"}</DialogTitle>
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
            label="规则名称（可选）"
            size="small"
            placeholder="例如：AI 工具"
            value={r.name}
            onChange={(e) => setR({ ...r, name: e.target.value })}
            helperText="不填时用第一个进程名代替"
          />
          <Autocomplete<RunningProcess | string, true, false, true>
            multiple
            freeSolo
            filterSelectedOptions
            options={running.data ?? []}
            filterOptions={(opts, state) => filterProcesses(opts as RunningProcess[], state)}
            value={r.processes}
            inputValue={input}
            onInputChange={(_, v, reason) => {
              if (reason === "reset") return;
              setInput(v);
              setErrs((x) => ({ ...x, processes: undefined }));
            }}
            getOptionLabel={(o) => (typeof o === "string" ? o : o.name)}
            getOptionKey={(o) => (typeof o === "string" ? o : o.exe_path)}
            getOptionDisabled={(o) =>
              typeof o !== "string" &&
              (!!ownerOf(o.name) || r.processes.some((q) => same(fileName(q), o.name)))
            }
            isOptionEqualToValue={(o, v) => typeof o !== "string" && typeof v === "string" && same(fileName(v), o.name)}
            onChange={(_, value, reason) => {
              if (reason === "removeOption" || reason === "clear") {
                setR((x) => ({ ...x, processes: value.filter((v): v is string => typeof v === "string") }));
                return;
              }
              const last = value[value.length - 1];
              if (last === undefined) return;
              const e = add(typeof last === "string" ? last : r.match_full_path ? last.exe_path : last.name);
              setErrs((x) => ({ ...x, processes: e }));
              if (!e) setInput("");
            }}
            renderValue={(value, getItemProps) =>
              value.map((v, index) => {
                const p = typeof v === "string" ? v : v.name;
                const missing = r.match_full_path && !isPath(p);
                const { key, ...itemProps } = getItemProps({ index });
                return (
                  <Chip
                    key={key}
                    {...itemProps}
                    size="small"
                    label={p}
                    title={p}
                    color={missing ? "warning" : "default"}
                    variant={missing ? "outlined" : "filled"}
                  />
                );
              })
            }
            renderOption={({ key, ...props }, o) => {
              const p = o as RunningProcess;
              const owner = ownerOf(p.name);
              const added = r.processes.some((q) => same(fileName(q), p.name));
              return (
                <Box component="li" key={key} {...props}>
                  <InsertDriveFileOutlined fontSize="small" sx={{ mr: 1, color: "text.secondary" }} />
                  <Box sx={{ minWidth: 0 }}>
                    <Typography variant="body2">{p.name}</Typography>
                    <Typography variant="caption" sx={{ color: "text.secondary", wordBreak: "break-all" }}>
                      {added ? "已添加" : owner ? `已在「${ruleLabel(owner)}」中` : p.exe_path}
                    </Typography>
                  </Box>
                </Box>
              );
            }}
            renderInput={(params) => (
              <TextField
                {...params}
                label="进程"
                size="small"
                placeholder={r.processes.length ? "" : r.match_full_path ? "选择正在运行的程序，或粘贴 exe 完整路径" : "如 ChatGPT.exe，回车添加"}
                error={!!errs.processes || missingPaths.length > 0}
                helperText={processHelp}
              />
            )}
          />
          {!r.match_full_path && (
            <Alert severity="info" variant="outlined" sx={{ py: 0 }}>
              {NAME_MATCH_HINT}
            </Alert>
          )}
          <FormControl size="small" error={!!errs.node}>
            <Button
              variant="outlined"
              color={errs.node ? "error" : "inherit"}
              onClick={() => setPicking(true)}
              sx={{ justifyContent: "flex-start", textTransform: "none" }}
            >
              {`节点：${node && m ? nodeLabel(m, node) : "未选择"}`}
            </Button>
            {errs.node && <FormHelperText>{errs.node}</FormHelperText>}
          </FormControl>
          {picking && (
            <NodePickerDialog
              title="选择节点"
              value={node}
              onClose={() => setPicking(false)}
              onPick={(n) => {
                setR((x) => ({ ...x, node: n, interface: m && isTailscaleRef(m, n) ? null : x.interface }));
                setErrs((x) => ({ ...x, node: undefined }));
                setPicking(false);
              }}
            />
          )}
          <FormControlLabel
            control={<Switch checked={r.user_enabled} onChange={(e) => setR({ ...r, user_enabled: e.target.checked })} />}
            label="启用"
          />
          <Box component="details" open={advanced} onToggle={(e) => setAdvanced((e.currentTarget as HTMLDetailsElement).open)}>
            <Box component="summary" sx={{ cursor: "pointer", color: "text.secondary", fontSize: 14, mb: 1.5 }}>
              高级选项
            </Box>
            <Stack spacing={2}>
              <Box>
                <FormControlLabel
                  control={<Switch checked={r.match_full_path} onChange={(e) => setFullPath(e.target.checked)} />}
                  label="按完整路径匹配"
                />
                <FormHelperText sx={{ mx: 0, mt: 0 }}>{FULL_PATH_HINT}</FormHelperText>
              </Box>
              <Box>
                <InterfacePicker
                  label="出口网卡"
                  value={tailscale ? tailscaleNic : (r.interface ?? null)}
                  onChange={(v) => setR({ ...r, interface: v })}
                  disabled={tailscale}
                />
                {tailscale && <FormHelperText sx={{ mx: 0 }}>在 Tailscale 节点设置里修改</FormHelperText>}
              </Box>
              <FormControl>
                <FormLabel id="pr-fault">组件故障时</FormLabel>
                <RadioGroup
                  row
                  aria-labelledby="pr-fault"
                  value={r.on_fault}
                  onChange={(e) => setR({ ...r, on_fault: e.target.value as FaultPolicy })}
                >
                  <FormControlLabel value="Direct" control={<Radio size="small" />} label="直连" />
                  <FormControlLabel
                    value="Block"
                    control={<Radio size="small" />}
                    label="阻断"
                    disabled={!r.match_full_path}
                  />
                </RadioGroup>
                <FormHelperText sx={{ mx: 0 }}>
                  {r.match_full_path ? FAULT_POLICY_HINT : "按进程名匹配时只能直连；要阻断请先打开「按完整路径匹配」"}
                </FormHelperText>
              </FormControl>
              <Box>
                <FormControlLabel
                  control={
                    <Switch checked={r.reset_on_change} onChange={(e) => setR({ ...r, reset_on_change: e.target.checked })} />
                  }
                  label="规则变化时断开旧连接"
                />
                {r.reset_on_change && <FormHelperText sx={{ mx: 0, mt: 0 }}>{RESET_HINT}</FormHelperText>}
              </Box>
            </Stack>
          </Box>
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>取消</Button>
        <Button
          variant="contained"
          disabled={save.isPending || missingPaths.length > 0}
          onClick={() => void submit()}
        >
          保存
        </Button>
      </DialogActions>
    </FormDialog>
  );
}

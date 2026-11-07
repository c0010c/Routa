import HelpOutline from "@mui/icons-material/HelpOutlineOutlined";
import {
  Alert,
  Box,
  Button,
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
  Tooltip,
} from "@mui/material";
import { useState, type ReactNode } from "react";
import { errorMessage, isValidationError, useModel } from "../api/client";
import { useRemoveLearnedPath, useRunningProcesses, useSaveProcessRuleConfirmed } from "../api/procroute";
import type { FaultPolicy, ProcessRule, ValidationError } from "../bindings";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { FormDialog } from "../components/FormDialog";
import { InterfacePicker } from "../components/InterfacePicker";
import { LearnedLocations } from "../components/LearnedLocations";
import { NodePickerDialog } from "../components/NodePickerDialog";
import { ProcessEntriesField } from "../components/ProcessEntriesField";
import { fileName, newlyLearning, ruleLabel, skippedOf } from "../lib/processRule";
import { isTailscaleRef, nodeLabel } from "../lib/refs";
import { fieldErrors } from "../lib/validation";

export const FAULT_LABEL = "Routa 或组件不可用时";
export const FAULT_HELP =
  "Routa 退出或尚未运行、mihomo 内核启动或被停止、内核或分流组件出故障时生效；节点失效、网卡断开时，始终阻断。\n\n" +
  "选阻断后，按进程名匹配的程序也会被阻断。程序联网时会自动记录它的位置，分流服务停止后，系统防火墙按这些位置继续阻断；还没联网过的位置除外。\n\n" +
  "其他规则用路径指定的位置不会被记录，也不会被这条规则阻断。";
export const RESET_HINT = "仅对 IPv4 TCP 连接有效；更换代理目标不触发断开。";

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

/** ⓘ next to a label; clicking it doesn't toggle the control the label belongs to. */
function Help({ children }: { children: ReactNode }) {
  return (
    <Tooltip title={<Box sx={{ whiteSpace: "pre-line" }}>{children}</Box>}>
      <HelpOutline
        onClick={(e) => e.preventDefault()}
        sx={{ fontSize: 16, color: "text.secondary", verticalAlign: "-3px", ml: 0.5, cursor: "help" }}
      />
    </Tooltip>
  );
}

/** Info toast after saving a rule that starts recording locations; `null` when nothing new is recorded. */
function learningNotice(before: ProcessRule | undefined, saved: ProcessRule, others: ProcessRule[], routingEnabled: boolean) {
  const newly = newlyLearning(before, saved);
  if (newly.length === 0) return null;
  const skipped = skippedOf(saved, others, routingEnabled);
  return [
    "已开启按进程名阻断",
    `${newly.join("、")} 联网时会自动记录位置，分流服务停止后也能拦住；还没联网过的位置除外。`,
    ...(skipped.length
      ? [`${skipped.map((s) => `「${ruleLabel(s.rule)}」指定的 ${fileName(s.path)}`).join("、")} 不受影响。`]
      : []),
  ].join("\n");
}

export function ProcessRuleDialog({ open, rule, onClose }: { open: boolean; rule?: ProcessRule; onClose: () => void }) {
  return open ? <RuleForm rule={rule} onClose={onClose} /> : null;
}

function RuleForm({ rule, onClose }: { rule?: ProcessRule; onClose: () => void }) {
  const model = useModel();
  const running = useRunningProcesses();
  // The dialog stays open on failure and shows the reason itself.
  const save = useSaveProcessRuleConfirmed({ errorFeedback: "local" });
  const removeLearned = useRemoveLearnedPath();
  const notify = useApplyResult();
  const m = model.data;
  const rules = m?.process_routing.rules ?? [];
  const routingEnabled = m?.process_routing.enabled ?? false;
  const others = rules.filter((x) => x.id !== rule?.id);
  // Mirrors `ProcessRule::default`.
  const [r, setR] = useState<ProcessRule>(
    () =>
      rule ?? {
        id: crypto.randomUUID(),
        name: "",
        processes: [],
        learned: [],
        node: null,
        interface: null,
        on_fault: "Direct",
        reset_on_change: false,
        user_enabled: true,
      },
  );
  const [errs, setErrs] = useState<RuleErrors>({ form: [] });
  const [picking, setPicking] = useState(false);
  const node = r.node ?? null;
  const tailscale = !!m && !!node && isTailscaleRef(m, node);
  /** A Tailscale instance has its own NIC (set in its node settings); the rule's value doesn't apply. */
  const tailscaleNic = (() => {
    if (!m || !node || !tailscale || !("manual_id" in node)) return null;
    const kind = m.manual_nodes.find((x) => x.id === node.manual_id)?.kind;
    return kind && typeof kind === "object" && "Tailscale" in kind ? kind.Tailscale.interface : null;
  })();
  const block = r.on_fault === "Block";
  const skipped = block ? skippedOf(r, others, routingEnabled) : [];
  // Learned locations are the service's: show the stored list, removals apply at once.
  const stored = rule && rules.find((x) => x.id === rule.id);
  const autoName = r.processes.length ? ruleLabel({ name: "", processes: r.processes }) : "";

  const submit = async () => {
    if (r.processes.length === 0) {
      setErrs({ form: [], processes: "至少添加一个进程" });
      return;
    }
    if (!node) {
      setErrs({ form: [], node: "请选择一个节点" });
      return;
    }
    setErrs({ form: [] });
    const next = { ...r, name: r.name.trim(), interface: tailscale ? null : (r.interface ?? null) };
    try {
      await save.mutateAsync(next);
      onClose();
      const notice = learningNotice(rule, next, others, routingEnabled);
      if (notice) notify.showMessage(notice, "info");
    } catch (e) {
      setErrs(isValidationError(e) ? ruleErrors(e.errors) : { form: [errorMessage(e)] });
    }
  };

  return (
    <FormDialog onClose={onClose} fullWidth maxWidth="sm">
      <DialogTitle>{rule ? "编辑分流规则" : "新建分流规则"}</DialogTitle>
      <DialogContent>
        <Stack spacing={2.5} sx={{ mt: 1 }}>
          {errs.form.length > 0 && (
            <Alert severity="error" role="alert">
              {errs.form.map((x) => (
                <div key={x}>{x}</div>
              ))}
            </Alert>
          )}
          <TextField
            label="规则名称（可选）"
            size="small"
            placeholder={autoName || "例如：AI 工具"}
            value={r.name}
            onChange={(e) => setR({ ...r, name: e.target.value })}
            helperText={autoName && !r.name ? `不填时显示为「${autoName}」` : "不填时用第一个进程名代替"}
          />
          <ProcessEntriesField
            entries={r.processes}
            others={others}
            routingEnabled={routingEnabled}
            running={running.data ?? []}
            error={errs.processes}
            onChange={(processes) => {
              setR((x) => ({ ...x, processes }));
              setErrs((x) => ({ ...x, processes: undefined }));
            }}
          />
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
          <Box>
            <InterfacePicker
              value={tailscale ? tailscaleNic : (r.interface ?? null)}
              onChange={(v) => setR({ ...r, interface: v })}
              disabled={tailscale}
            />
            {tailscale && <FormHelperText sx={{ mx: 0 }}>在 Tailscale 节点设置里修改</FormHelperText>}
          </Box>
          <FormControlLabel
            control={<Switch checked={r.user_enabled} onChange={(e) => setR({ ...r, user_enabled: e.target.checked })} />}
            label="启用"
          />
          <FormControl>
            <FormLabel id="pr-fault">
              {FAULT_LABEL}
              <Help>{FAULT_HELP}</Help>
            </FormLabel>
            <RadioGroup
              row
              aria-labelledby="pr-fault"
              value={r.on_fault}
              onChange={(e) => setR({ ...r, on_fault: e.target.value as FaultPolicy })}
            >
              <FormControlLabel value="Direct" control={<Radio size="small" />} label="直连" />
              <FormControlLabel value="Block" control={<Radio size="small" />} label="阻断" />
            </RadioGroup>
            {skipped.length > 0 && (
              <FormHelperText sx={{ mx: 0, mt: 0 }}>
                {skipped.map((s) => `${fileName(s.path)} 在「${ruleLabel(s.rule)}」指定的位置`).join("、")}
                不受这条规则阻断。
              </FormHelperText>
            )}
            {block && stored && (
              <LearnedLocations
                rule={{ ...r, learned: stored.learned }}
                others={others}
                routingEnabled={routingEnabled}
                running={running.data ?? []}
                onRemove={(p) => removeLearned.mutate(r.id, p)}
              />
            )}
          </FormControl>
          <FormControlLabel
            control={<Switch checked={r.reset_on_change} onChange={(e) => setR({ ...r, reset_on_change: e.target.checked })} />}
            label={
              <>
                规则变化时断开旧连接
                <Help>{RESET_HINT}</Help>
              </>
            }
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>取消</Button>
        <Button variant="contained" disabled={save.isPending} onClick={() => void submit()}>
          保存
        </Button>
      </DialogActions>
    </FormDialog>
  );
}

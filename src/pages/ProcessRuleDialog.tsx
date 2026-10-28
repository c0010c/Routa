import InsertDriveFileOutlined from "@mui/icons-material/InsertDriveFileOutlined";
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  createFilterOptions,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  FormControlLabel,
  FormHelperText,
  FormLabel,
  InputLabel,
  Link,
  MenuItem,
  Radio,
  RadioGroup,
  Select,
  Stack,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import { useState } from "react";
import { Link as RouterLink } from "react-router-dom";
import { isValidationError, useModel } from "../api/client";
import { useRunningProcesses, useSaveProcessRule } from "../api/procroute";
import type { FaultPolicy, ProcessRule, RunningProcess, ValidationError } from "../bindings";
import { FormDialog } from "../components/FormDialog";
import { fieldErrors } from "../lib/validation";

export const FAULT_POLICY_HINT = "只在 mihomo 或分流组件意外故障时生效；Listener 因节点失效、网卡断开被停用时，始终阻断";
export const RESET_HINT = "仅对 IPv4 TCP 连接有效";

const TYPE_TEXT = { Mixed: "混合", Http: "HTTP", Socks: "SOCKS" } as const;

interface RuleErrors {
  path?: string;
  listener?: string;
  form: string[];
}

/** Backend validation errors placed on the dialog's fields; the wording comes from `fieldErrors`. */
function ruleErrors(errors: ValidationError[]): RuleErrors {
  const out: RuleErrors = { form: [] };
  for (const e of errors) {
    const text = fieldErrors([e]);
    const msg = [...(text.name ? [text.name] : []), ...(text.port ? [text.port] : []), ...text.form].join("\n");
    if ("DuplicateProcessRule" in e || "InvalidExePath" in e) out.path ??= msg;
    else if ("ListenerTypeNotAllowed" in e) out.listener ??= msg;
    else out.form.push(msg);
  }
  return out;
}

const filterProcesses = createFilterOptions<RunningProcess>({ stringify: (p) => `${p.name} ${p.exe_path}` });

export function ProcessRuleDialog({ open, rule, onClose }: { open: boolean; rule?: ProcessRule; onClose: () => void }) {
  return open ? <RuleForm rule={rule} onClose={onClose} /> : null;
}

function RuleForm({ rule, onClose }: { rule?: ProcessRule; onClose: () => void }) {
  const model = useModel();
  const running = useRunningProcesses();
  const save = useSaveProcessRule();
  const listeners = (model.data?.listeners ?? []).filter((l) => l.listener_type !== "Http");
  // Mirrors `ProcessRule::new_default`; the listener defaults to the first usable one once the model is in.
  const [r, setR] = useState<ProcessRule>(
    () =>
      rule ?? {
        id: crypto.randomUUID(),
        exe_path: "",
        listener: "",
        on_fault: "Block",
        reset_on_change: false,
        user_enabled: true,
      },
  );
  const [errs, setErrs] = useState<RuleErrors>({ form: [] });
  const listenerId = r.listener !== "" ? r.listener : (listeners[0]?.id ?? "");
  // An HTTP or deleted listener is not an option: show the select empty instead of an out-of-range value.
  const selectValue = listeners.some((l) => l.id === listenerId) ? listenerId : "";

  const submit = async () => {
    if (selectValue === "") {
      setErrs({ form: [], listener: "请选择一个监听器" });
      return;
    }
    setErrs({ form: [] });
    try {
      await save.mutateAsync({ ...r, exe_path: r.exe_path.trim(), listener: selectValue });
      onClose();
    } catch (e) {
      if (isValidationError(e)) setErrs(ruleErrors(e.errors));
    }
  };

  const isEdit = rule !== undefined;
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
          <Autocomplete<RunningProcess, false, false, true>
            freeSolo
            options={running.data ?? []}
            filterOptions={filterProcesses}
            getOptionLabel={(o) => (typeof o === "string" ? o : o.exe_path)}
            getOptionKey={(o) => (typeof o === "string" ? o : o.exe_path)}
            inputValue={r.exe_path}
            onInputChange={(_, v) => {
              setR((x) => ({ ...x, exe_path: v }));
              setErrs((x) => ({ ...x, path: undefined }));
            }}
            renderOption={({ key, ...props }, o) => (
              <Box component="li" key={key} {...props}>
                <InsertDriveFileOutlined fontSize="small" sx={{ mr: 1, color: "text.secondary" }} />
                <Box sx={{ minWidth: 0 }}>
                  <Typography variant="body2">{o.name}</Typography>
                  <Typography variant="caption" sx={{ color: "text.secondary", wordBreak: "break-all" }}>
                    {o.exe_path}
                  </Typography>
                </Box>
              </Box>
            )}
            renderInput={(params) => (
              <TextField
                {...params}
                label="程序路径"
                size="small"
                error={!!errs.path}
                helperText={errs.path ?? "从正在运行的程序中选择，或填写 exe 的完整路径"}
              />
            )}
          />
          <FormControl size="small" error={!!errs.listener}>
            <InputLabel id="pr-listener">监听器</InputLabel>
            <Select
              labelId="pr-listener"
              label="监听器"
              value={selectValue}
              onChange={(e) => {
                setR({ ...r, listener: e.target.value });
                setErrs((x) => ({ ...x, listener: undefined }));
              }}
            >
              {listeners.map((l) => (
                <MenuItem key={l.id} value={l.id}>
                  {`${l.name}（${TYPE_TEXT[l.listener_type]} :${l.port}）`}
                </MenuItem>
              ))}
            </Select>
            {errs.listener ? (
              <FormHelperText>{errs.listener}</FormHelperText>
            ) : (
              model.data &&
              listeners.length === 0 && (
                <FormHelperText>
                  没有可用的混合或 SOCKS 监听器，请先到
                  <Link component={RouterLink} to="/listeners">
                    监听器
                  </Link>
                  页新建
                </FormHelperText>
              )
            )}
          </FormControl>
          <FormControl>
            <FormLabel id="pr-fault">组件故障时</FormLabel>
            <RadioGroup
              row
              aria-labelledby="pr-fault"
              value={r.on_fault}
              onChange={(e) => setR({ ...r, on_fault: e.target.value as FaultPolicy })}
            >
              <FormControlLabel value="Direct" control={<Radio size="small" />} label="直连" />
              <FormControlLabel value="Block" control={<Radio size="small" />} label="阻断" />
            </RadioGroup>
            <FormHelperText sx={{ mx: 0 }}>{FAULT_POLICY_HINT}</FormHelperText>
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
          <FormControlLabel
            control={<Switch checked={r.user_enabled} onChange={(e) => setR({ ...r, user_enabled: e.target.checked })} />}
            label="启用"
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

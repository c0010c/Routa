import { Alert, Button, FormControlLabel, Paper, Stack, Switch, TextField, Typography } from "@mui/material";
import { useState } from "react";
import { isValidationError, useModel, useOpenLogDir, useSaveSettings } from "../api/client";
import type { Settings as SettingsModel } from "../bindings";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { IntervalInput } from "../components/IntervalInput";
import { fieldErrors, type FieldErrors } from "../lib/validation";

function SettingsForm({ initial }: { initial: SettingsModel }) {
  const save = useSaveSettings();
  const openLogDir = useOpenLogDir();
  const apply = useApplyResult();
  const [saved, setSaved] = useState(initial);
  const [autostart, setAutostart] = useState(initial.autostart);
  const [corePath, setCorePath] = useState(initial.core_path ?? "");
  const [port, setPort] = useState(String(initial.controller_port));
  const [interval, setIntervalMins] = useState(initial.delay_test_interval_mins);
  const [url, setUrl] = useState(initial.delay_test_url);
  const [intervalValid, setIntervalValid] = useState(true);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });
  const [portError, setPortError] = useState<string>();

  const dirty =
    autostart !== saved.autostart ||
    corePath.trim() !== (saved.core_path ?? "") ||
    port !== String(saved.controller_port) ||
    interval !== saved.delay_test_interval_mins ||
    url !== saved.delay_test_url;

  const submit = async () => {
    const p = Number(port);
    if (!Number.isInteger(p) || p < 1 || p > 65535) {
      setPortError("端口必须在 1–65535 之间");
      return;
    }
    setPortError(undefined);
    setErrs({ form: [] });
    const next: SettingsModel = {
      autostart,
      core_path: corePath.trim() === "" ? null : corePath.trim(),
      controller_port: p,
      delay_test_interval_mins: interval,
      delay_test_url: url,
    };
    try {
      await save.mutateAsync(next);
    } catch (e) {
      if (isValidationError(e)) setErrs(fieldErrors(e.errors));
      return;
    }
    // The backend restarts mihomo itself when these change.
    const restarted = next.controller_port !== saved.controller_port || next.core_path !== saved.core_path;
    setSaved(next);
    apply.showMessage(restarted ? "设置已保存，mihomo 正在重启" : "设置已保存", "success");
  };

  return (
    <Stack spacing={2}>
      <Typography variant="h5">设置</Typography>
      <Paper sx={{ p: 2 }}>
        <Stack spacing={2} sx={{ maxWidth: 560 }}>
          {errs.form.length > 0 && (
            <Alert severity="error" role="alert">
              {errs.form.map((m) => (
                <div key={m}>{m}</div>
              ))}
            </Alert>
          )}
          <FormControlLabel
            control={<Switch checked={autostart} onChange={(e) => setAutostart(e.target.checked)} />}
            label="开机自启"
          />
          <TextField
            size="small"
            label="mihomo 内核路径"
            value={corePath}
            onChange={(e) => setCorePath(e.target.value)}
            helperText="留空使用内置内核"
          />
          <TextField
            size="small"
            label="控制端口"
            type="number"
            value={port}
            onChange={(e) => setPort(e.target.value)}
            error={!!portError}
            helperText={portError}
          />
          <IntervalInput label="自动测速" value={interval} onChange={setIntervalMins} onValidityChange={setIntervalValid} defaultMins={10} />
          <TextField size="small" label="测速 URL" value={url} onChange={(e) => setUrl(e.target.value)} />
          <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
            <Button variant="contained" disabled={save.isPending || !intervalValid || !dirty} onClick={() => void submit()}>
              保存
            </Button>
            <Typography variant="body2" sx={{ color: dirty ? "warning.main" : "text.secondary" }}>
              {dirty ? "有未保存的修改" : "已保存"}
            </Typography>
            <Button onClick={() => openLogDir.mutate()}>打开日志目录</Button>
          </Stack>
        </Stack>
      </Paper>
    </Stack>
  );
}

export function Settings() {
  const model = useModel();
  if (!model.data) return <Typography>加载中…</Typography>;
  return <SettingsForm initial={model.data.settings} />;
}

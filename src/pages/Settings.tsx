import { Alert, Box, Button, Card, Divider, Stack, Switch, TextField, Typography } from "@mui/material";
import { Fragment, useState, type ReactNode } from "react";
import { isValidationError, useModel, useOpenLogDir, useSaveSettings } from "../api/client";
import type { ApplyOutcome, Listener, Settings as SettingsModel } from "../bindings";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { IntervalInput } from "../components/IntervalInput";
import { fieldErrors, type FieldErrors } from "../lib/validation";
import { applyOutcomeMessages } from "../lib/format";

/** One Win11-style settings row: label and explanation on the left, control on the right. */
function Row({ label, description, children }: { label: string; description?: string; children: ReactNode }) {
  return (
    <Stack direction="row" spacing={2} sx={{ alignItems: "center", px: 2, py: 1.5 }}>
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography>{label}</Typography>
        {description && (
          <Typography variant="body2" sx={{ color: "text.secondary" }}>
            {description}
          </Typography>
        )}
      </Box>
      {children}
    </Stack>
  );
}

function Section({ title, children }: { title: string; children: ReactNode[] }) {
  return (
    <Stack component="section" aria-label={title} spacing={1}>
      <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>
        {title}
      </Typography>
      <Card>
        {children.map((c, i) => (
          <Fragment key={i}>
            {i > 0 && <Divider />}
            {c}
          </Fragment>
        ))}
      </Card>
    </Stack>
  );
}

function SettingsForm({ initial, listeners }: { initial: SettingsModel; listeners: Listener[] }) {
  const save = useSaveSettings({ errorFeedback: "local" });
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
  const [portLocal, setPortLocal] = useState<string>();
  const portError = portLocal ?? errs.port;

  const dirty =
    autostart !== saved.autostart ||
    corePath.trim() !== (saved.core_path ?? "") ||
    port !== String(saved.controller_port) ||
    interval !== saved.delay_test_interval_mins ||
    url !== saved.delay_test_url;

  const submit = async () => {
    const p = Number(port);
    if (!Number.isInteger(p) || p < 1 || p > 65535) {
      setPortLocal("端口必须在 1–65535 之间");
      return;
    }
    setPortLocal(undefined);
    setErrs({ form: [] });
    const next: SettingsModel = {
      autostart,
      core_path: corePath.trim() === "" ? null : corePath.trim(),
      controller_port: p,
      delay_test_interval_mins: interval,
      delay_test_url: url,
    };
    let outcome: ApplyOutcome;
    try {
      outcome = await save.mutateAsync(next);
    } catch (e) {
      if (isValidationError(e)) {
        const fields = fieldErrors(e.errors.filter((error) => !("CoreRejected" in error)));
        if (e.errors.some((error) => "CoreRejected" in error)) {
          fields.form.push("设置无法使用，请检查后重新保存");
        }
        const conflict = e.errors.find((error) => "PortConflict" in error);
        if (conflict && "PortConflict" in conflict) {
          const { port: conflictPort } = conflict.PortConflict;
          const listener = listeners.find((l) => l.port === conflictPort);
          fields.port = listener
            ? `控制端口 ${conflictPort} 与 Listener「${listener.name}」冲突，请使用其他端口`
            : `控制端口 ${conflictPort} 与已有 Listener 冲突，请使用其他端口`;
        }
        setErrs(fields);
      } else {
        apply.showError("设置未能保存，请稍后手动重试");
      }
      console.error("Saving settings failed", e);
      return;
    }
    setSaved(next);
    // The shared mutation hook already reports apply failures and warnings.
    if ("Reloaded" in outcome && applyOutcomeMessages(outcome).length === 0) {
      apply.showMessage("设置已保存并生效", "success");
    }
  };

  return (
    <Stack spacing={2}>
      <Typography variant="h5">设置</Typography>
      {errs.form.length > 0 && (
        <Alert severity="error" role="alert">
          {errs.form.map((m) => (
            <div key={m}>{m}</div>
          ))}
        </Alert>
      )}
      <Stack spacing={2} sx={{ maxWidth: 760 }}>
        <Section title="常规">
          {[
            <Row key="autostart" label="开机自启" description="登录 Windows 后自动启动 Routa">
              <Switch
                checked={autostart}
                onChange={(e) => setAutostart(e.target.checked)}
                slotProps={{ input: { "aria-label": "开机自启" } }}
              />
            </Row>,
          ]}
        </Section>
        <Section title="内核">
          {[
            <Row key="path" label="mihomo 内核路径" description="留空使用内置内核">
              <TextField
                placeholder="内置内核"
                value={corePath}
                onChange={(e) => setCorePath(e.target.value)}
                slotProps={{ htmlInput: { "aria-label": "mihomo 内核路径" } }}
                sx={{ width: 260 }}
              />
            </Row>,
            <Row key="port" label="控制端口" description="修改后 mihomo 会重启">
              <TextField
                type="number"
                value={port}
                onChange={(e) => {
                  setPort(e.target.value);
                  setPortLocal(undefined);
                  setErrs((x) => ({ ...x, port: undefined }));
                }}
                error={!!portError}
                helperText={portError}
                slotProps={{ htmlInput: { "aria-label": "控制端口" } }}
                sx={{ width: 140 }}
              />
            </Row>,
            <Row key="log" label="日志">
              <Button variant="outlined" onClick={() => openLogDir.mutate()}>
                打开日志目录
              </Button>
            </Row>,
          ]}
        </Section>
        <Section title="测速">
          {[
            <Row key="auto" label="自动测速" description="按间隔对所有出口测延迟">
              <IntervalInput value={interval} onChange={setIntervalMins} onValidityChange={setIntervalValid} defaultMins={10} />
            </Row>,
            <Row key="url" label="测速 URL">
              <TextField
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                slotProps={{ htmlInput: { "aria-label": "测速 URL" } }}
                sx={{ width: 320 }}
              />
            </Row>,
          ]}
        </Section>
        <Stack direction="row" spacing={1.5} sx={{ alignItems: "center" }}>
          <Button variant="contained" disabled={save.isPending || !intervalValid || !dirty} onClick={() => void submit()}>
            保存
          </Button>
          <Typography variant="body2" sx={{ color: dirty ? "warning.main" : "text.secondary" }}>
            {dirty ? "有未保存的修改" : "已保存"}
          </Typography>
        </Stack>
      </Stack>
    </Stack>
  );
}

export function Settings() {
  const model = useModel();
  if (!model.data) return <Typography>加载中…</Typography>;
  return <SettingsForm initial={model.data.settings} listeners={model.data.listeners} />;
}

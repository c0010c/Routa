import mihomoVersion from "../../src-tauri/binaries/MIHOMO_VERSION?raw";
import {
  Alert,
  Box,
  Button,
  Card,
  Divider,
  Stack,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import { Fragment, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import {
  isValidationError,
  useModel,
  useOpenLogDir,
} from "../api/client";
import { settingsSaveMessage, useSettingsSave } from "../api/settings";
import type {
  Listener,
  Settings as SettingsModel,
} from "../bindings";
import { SaveFeedback, useSaveFeedback } from "../components/SaveFeedback";
import { IntervalInput } from "../components/IntervalInput";
import { fieldErrors, type FieldErrors } from "../lib/validation";
import { applyOutcomeMessages } from "../lib/format";
import { UiPreferencesPanel } from "../components/UiPreferencesPanel";

/** One Win11-style settings row: label and explanation on the left, control on the right. */
function Row({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <Stack
      spacing={1}
      sx={{ alignItems: "flex-start", px: 2, py: 1.5 }}
    >
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

function Section({
  title,
  children,
  footer,
}: {
  title: string;
  children: ReactNode[];
  footer?: ReactNode;
}) {
  return (
    <Stack component="section" aria-label={title} spacing={1}>
      <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>
        {title}
      </Typography>
      <Card sx={footer ? { display: "flex", flexDirection: "column", height: "min(420px, calc(100dvh - 96px))", minHeight: 220 } : undefined}>
        <Box sx={footer ? { flex: 1, minHeight: 0, overflowY: "auto" } : undefined}>
          {children.map((c, i) => (
            <Fragment key={i}>
              {i > 0 && <Divider />}
              {c}
            </Fragment>
          ))}
        </Box>
        {footer}
      </Card>
    </Stack>
  );
}

function SettingsForm({
  initial,
  listeners,
}: {
  initial: SettingsModel;
  listeners: Listener[];
}) {
  const save = useSettingsSave();
  const openLogDir = useOpenLogDir();
  const feedback = useSaveFeedback<"autostart" | "interval" | "url" | "core">();
  const [port, setPort] = useState(String(initial.controller_port));
  const [url, setUrl] = useState(initial.delay_test_url);
  const [intervalRevision, setIntervalRevision] = useState(0);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });
  const [portLocal, setPortLocal] = useState<string>();
  const portError = portLocal ?? errs.port;

  const dirty = port !== String(initial.controller_port);
  const pending = save.isPending || feedback.pending;

  const saveImmediate = (key: "autostart" | "interval" | "url", patch: Partial<SettingsModel>) =>
    feedback.run(key, async () => {
      try {
        await save.mutateAsync(patch);
      } catch (error) {
        if (key === "interval") setIntervalRevision((value) => value + 1);
        throw new Error(settingsSaveMessage(error));
      }
    });

  const saveUrl = () => {
    if (url === initial.delay_test_url || pending) return;
    void saveImmediate("url", { delay_test_url: url });
  };

  const submit = async () => {
    const p = Number(port);
    if (!Number.isInteger(p) || p < 1 || p > 65535) {
      setPortLocal("端口必须在 1–65535 之间");
      return;
    }
    setPortLocal(undefined);
    setErrs({ form: [] });
    await feedback.run("core", async () => {
      try {
        const outcome = await save.mutateAsync({ controller_port: p });
        const warnings = applyOutcomeMessages(outcome);
        if (warnings.length) return { kind: "warning", text: warnings.join("；") };
        return { kind: "success", text: "已保存并生效" };
      } catch (e) {
        if (isValidationError(e)) {
          const fields = fieldErrors(
            e.errors.filter((error) => !("CoreRejected" in error)),
          );
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
        }
        throw new Error(isValidationError(e) ? "设置未能保存，请检查后重试" : settingsSaveMessage(e));
      }
    });
  };

  return (
    <Stack spacing={2} sx={{ maxWidth: 760 }}>
      <Typography variant="h5">设置</Typography>
      <UiPreferencesPanel />
      <Button
        component={Link}
        to="/diagnostics"
        sx={{ alignSelf: "flex-start" }}
      >
        故障诊断
      </Button>
      <Stack spacing={2} sx={{ maxWidth: 760 }}>
        <Section title="常规">
          {[
            <Row
              key="autostart"
              label="开机自启"
              description="登录 Windows 后自动启动 Routa"
            >
              <Switch
                checked={initial.autostart}
                disabled={pending}
                onChange={(_, value) => void saveImmediate("autostart", { autostart: value })}
                slotProps={{ input: { "aria-label": "开机自启" } }}
              />
              <SaveFeedback notice={feedback.notices.autostart} />
            </Row>,
          ]}
        </Section>
        <Section title="测速">
          {[
            <Row key="auto" label="自动测速" description="按间隔对所有出口测延迟">
              <IntervalInput
                key={`${initial.delay_test_interval_mins}:${intervalRevision}`}
                value={initial.delay_test_interval_mins}
                onChange={(value) => {
                  if (value !== initial.delay_test_interval_mins) void saveImmediate("interval", { delay_test_interval_mins: value });
                }}
                defaultMins={10}
                onValidityChange={() => feedback.clear("interval")}
                disabled={pending}
                commitOnBlur
              />
              <SaveFeedback notice={feedback.notices.interval} />
            </Row>,
            <Row key="url" label="测速 URL">
              <TextField
                value={url}
                disabled={pending}
                onChange={(e) => { setUrl(e.target.value); feedback.clear("url"); }}
                onBlur={saveUrl}
                onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); saveUrl(); } }}
                slotProps={{ htmlInput: { "aria-label": "测速 URL" } }}
                sx={{ width: 320, maxWidth: "100%" }}
              />
              <SaveFeedback notice={feedback.notices.url} onRetry={saveUrl} />
            </Row>,
          ]}
        </Section>
        <Section title="内核" footer={
          <Stack
            role="group"
            aria-label="内核保存操作"
            spacing={0.5}
            sx={{ flexShrink: 0, p: 2, borderTop: 1, borderColor: "divider", bgcolor: "background.paper" }}
          >
            <Stack direction="row" spacing={1.5} sx={{ alignItems: "center", flexWrap: "wrap", rowGap: 1 }}>
              <Button variant="contained" disabled={pending || !dirty} onClick={() => void submit()}>保存并重启</Button>
              <Typography variant="body2" color="text.secondary">{dirty ? "有未保存的修改" : ""}</Typography>
            </Stack>
            <SaveFeedback notice={feedback.notices.core} />
          </Stack>
        }>
          {[
            <Row key="path" label="内置 mihomo"><Typography color="text.secondary">{mihomoVersion.trim()} · 随 Routa 提供</Typography></Row>,
            <Row key="port" label="控制端口" description="修改后 mihomo 会重启">
              <TextField
                type="number"
                disabled={pending}
                value={port}
                onChange={(e) => {
                  setPort(e.target.value);
                  setPortLocal(undefined);
                  setErrs((x) => ({ ...x, port: undefined }));
                  feedback.clear("core");
                }}
                error={!!portError}
                helperText={portError ?? " "}
                slotProps={{ htmlInput: { "aria-label": "控制端口" } }}
                sx={{ width: 140 }}
              />
              {errs.form.length > 0 && <Alert severity="error">{errs.form.join("；")}</Alert>}
            </Row>,
            <Row key="log" label="日志">
              <Button variant="outlined" onClick={() => openLogDir.mutate()}>
                打开日志目录
              </Button>
            </Row>,
          ]}
        </Section>
      </Stack>
    </Stack>
  );
}

export function Settings() {
  const model = useModel();
  if (!model.data) return <Typography>加载中…</Typography>;
  return (
    <SettingsForm
      initial={model.data.settings}
      listeners={model.data.listeners}
    />
  );
}

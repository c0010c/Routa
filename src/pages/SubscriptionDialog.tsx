import {
  Alert,
  Button,
  Box,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  FormHelperText,
  InputLabel,
  InputAdornment,
  MenuItem,
  Select,
  Stack,
  TextField,
} from "@mui/material";
import { visibleListeners } from "../lib/hidden";
import { useState } from "react";
import {
  errorMessage,
  isValidationError,
  useModel,
  useSaveSubscription,
} from "../api/client";
import type { NodeRef, Subscription, SubscriptionPreview } from "../bindings";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { useSubscriptionCommands, subscriptionError, type SubscriptionNotice } from "../api/subscriptions";
import { applicationProblem } from "../lib/subscriptions";
import { SubscriptionReviewDialog } from "./SubscriptionReviewDialog";
import { IntervalInput } from "../components/IntervalInput";
import { fieldErrors, type FieldErrors } from "../lib/validation";
import { FormDialog } from "../components/FormDialog";

export function newSubscription(): Subscription {
  return {
    id: crypto.randomUUID(),
    name: "",
    url: "",
    update_interval_mins: 1440,
    fallback_exit: null,
    last_updated: null,
    last_result: null,
    nodes: [],
  };
}

export function SubscriptionDialog({
  subscription,
  onClose,
  notify: onNotify,
  showAdvanced = false,
}: {
  subscription: Subscription;
  onClose: () => void;
  notify?: SubscriptionNotice;
  showAdvanced?: boolean;
}) {
  const model = useModel();
  const save = useSaveSubscription({ errorFeedback: "local", applyFeedback: false });
  const actions = useSubscriptionCommands();
  const feedback = useApplyResult();
  const notify: SubscriptionNotice = onNotify ?? ((text, severity = "success") => feedback.showMessage(text, severity));
  const [preview, setPreview] = useState<SubscriptionPreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [previewError, setPreviewError] = useState("");
  const [discard, setDiscard] = useState(false);
  const [revealUrl, setRevealUrl] = useState(false);
  const [name, setName] = useState(subscription.name);
  const [url, setUrl] = useState(subscription.url);
  const [interval, setIntervalMins] = useState(
    subscription.update_interval_mins,
  );
  const [fallback, setFallback] = useState(
    subscription.fallback_listener ??
      (model.data ? visibleListeners(model.data) : []).find((l) => l.exit === subscription.fallback_exit)
        ?.id ??
      null,
  );
  const [intervalValid, setIntervalValid] = useState(true);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });
  const dirty = name !== subscription.name || url !== subscription.url || interval !== subscription.update_interval_mins || fallback !== (subscription.fallback_listener ?? (model.data ? visibleListeners(model.data) : []).find(l => l.exit === subscription.fallback_exit)?.id ?? null);
  const dismiss = () => { if (busy || save.isPending) return; if (dirty) setDiscard(true); else onClose(); };

  const submit = async () => {
    setErrs({ form: [] });
    setPreviewError("");
    try {
      const next = {
        ...subscription,
        name: name.trim() || suggestedName(),
        url: url.trim(),
        update_interval_mins: interval,
        fallback_listener: fallback,
        fallback_exit:
          model.data?.listeners.find((l) => l.id === fallback)?.exit ?? null,
      };
      if (!isEdit || next.url !== subscription.url) {
        setBusy(true);
        setPreview(await actions.preview(next));
      } else {
        const outcome = await save.mutateAsync(next);
        const problem = applicationProblem(outcome);
        notify(problem ?? "订阅设置已保存", problem ? "error" : "success");
        onClose();
      }
    } catch (e) {
      if (isValidationError(e)) setErrs(fieldErrors(e.errors));
      else { setErrs({ form: [errorMessage(e)] }); notify(subscriptionError(e), "error"); }
    } finally {
      setBusy(false);
    }
  };

  const exits = (model.data ? visibleListeners(model.data) : [])
    .filter((l) => l.exit)
    .map((l) => ({ id: l.id, name: l.name }));
  const suggestedName = () => {
    let base = "订阅";
    try {
      base = new URL(url.trim()).hostname || base;
    } catch {
      /* URL validation happens below. */
    }
    const names = new Set([
      ...(model.data?.subscriptions ?? [])
        .filter((s) => s.id !== subscription.id)
        .map((s) => s.name),
      ...exits.map((e) => e.name),
    ]);
    let next = base;
    for (let i = 2; names.has(next); i++) next = `${base} ${i}`;
    return next;
  };
  let validUrl = false;
  try {
    validUrl = ["https:", "http:"].includes(new URL(url.trim()).protocol);
  } catch {
    /* Incomplete input. */
  }
  const isEdit = model.data?.subscriptions.some(
    (s) => s.id === subscription.id,
  );
  const confirm = async (replacement: NodeRef | null) => {
    if (!preview) return;
    setBusy(true); setPreviewError("");
    try {
      const outcome = await actions.commit(preview, replacement);
      const problem = applicationProblem(outcome);
      notify(problem ?? `订阅已更新 · ${preview.subscription.nodes.length} 个节点`, problem ? "error" : "success");
      onClose();
    } catch (e) { setPreviewError(subscriptionError(e)); }
    finally { setBusy(false); }
  };
  if (preview) return <SubscriptionReviewDialog preview={preview} busy={busy} error={previewError} onConfirm={replacement => void confirm(replacement)} onClose={() => !busy && setPreview(null)} />;
  return (
    <FormDialog
      onClose={dismiss}
      busy={busy || save.isPending}
      fullWidth
      maxWidth="sm"
    >
      <DialogTitle>{isEdit ? "编辑订阅" : "新建订阅"}</DialogTitle>
      <DialogContent>
        <Stack component="fieldset" disabled={busy || save.isPending} spacing={2} sx={{ mt: 1, border: 0, p: 0, minWidth: 0 }}>
          {errs.form.length > 0 && (
            <Alert severity="error" role="alert">
              {errs.form.map((m) => (
                <div key={m}>{m}</div>
              ))}
            </Alert>
          )}
          <TextField
            size="small"
            label="名称（可选）"
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              setErrs((x) => ({ ...x, name: undefined }));
            }}
            error={!!errs.name}
            helperText={errs.name}
          />
          <TextField
            autoFocus
            size="small"
            label="URL"
            type={isEdit && !revealUrl ? "password" : "text"}
            slotProps={{ input: { endAdornment: isEdit ? <InputAdornment position="end"><Button size="small" onClick={() => setRevealUrl(!revealUrl)}>{revealUrl ? "隐藏" : "显示"}</Button></InputAdornment> : undefined } }}
            placeholder="https://…"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            error={url.length > 0 && !validUrl}
            helperText={
              url.length > 0 && !validUrl
                ? "请输入有效的 HTTP 或 HTTPS 订阅地址"
                : undefined
            }
          />
          <Box component="details" open={showAdvanced || undefined}>
            <Box
              component="summary"
              sx={{
                cursor: "pointer",
                color: "text.secondary",
                fontSize: 13,
                mb: 1.5,
              }}
            >
              高级选项
            </Box>
            <Stack spacing={2}>
              <IntervalInput
                label="自动更新"
                value={interval}
                onChange={setIntervalMins}
                onValidityChange={setIntervalValid}
                defaultMins={1440}
              />
              <FormControl size="small">
                <InputLabel id="s-fallback">备用代理</InputLabel>
                <Select
                  labelId="s-fallback"
                  label="备用代理"
                  value={fallback ?? ""}
                  onChange={(e) =>
                    setFallback(e.target.value === "" ? null : e.target.value)
                  }
                >
                  <MenuItem value="">不使用</MenuItem>
                  {fallback && !exits.some((e) => e.id === fallback) && (
                    <MenuItem value={fallback}>
                      原备用配置（请重新选择代理）
                    </MenuItem>
                  )}
                  {exits.map((x) => (
                    <MenuItem key={x.id} value={x.id}>
                      {x.name}
                    </MenuItem>
                  ))}
                </Select>
                <FormHelperText>直连拉取失败时，通过此代理重试</FormHelperText>
              </FormControl>
            </Stack>
          </Box>
          {discard && <Alert severity="warning" action={<Button onClick={onClose}>放弃修改</Button>}>有尚未保存的修改。<Button size="small" onClick={() => setDiscard(false)}>继续编辑</Button></Alert>}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button disabled={busy || save.isPending} onClick={dismiss}>取消</Button>
        <Button
          variant="contained"
          disabled={busy || save.isPending || !intervalValid || !validUrl}
          onClick={() => void submit()}
        >
          {busy ? "正在检查…" : !isEdit || url.trim() !== subscription.url ? "预览节点" : "保存"}
        </Button>
      </DialogActions>
    </FormDialog>
  );
}

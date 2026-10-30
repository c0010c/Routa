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
  MenuItem,
  Select,
  Stack,
  TextField,
} from "@mui/material";
import { useRef, useState } from "react";
import {
  errorMessage,
  isValidationError,
  useModel,
  useSaveSubscription,
} from "../api/client";
import type { Subscription } from "../bindings";
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
  focusFallback = false,
}: {
  subscription: Subscription;
  onClose: () => void;
  focusFallback?: boolean;
}) {
  const model = useModel();
  const save = useSaveSubscription();
  const [name, setName] = useState(subscription.name);
  const [url, setUrl] = useState(subscription.url);
  const [interval, setIntervalMins] = useState(
    subscription.update_interval_mins,
  );
  const [fallback, setFallback] = useState(
    subscription.fallback_listener ??
      model.data?.listeners.find((l) => l.exit === subscription.fallback_exit)
        ?.id ??
      null,
  );
  const [intervalValid, setIntervalValid] = useState(true);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });
  const fallbackRef = useRef<HTMLInputElement | null>(null);

  const submit = async () => {
    setErrs({ form: [] });
    try {
      await save.mutateAsync({
        ...subscription,
        name: name.trim() || suggestedName(),
        url: url.trim(),
        update_interval_mins: interval,
        fallback_listener: fallback,
        fallback_exit:
          model.data?.listeners.find((l) => l.id === fallback)?.exit ?? null,
      });
      onClose();
    } catch (e) {
      if (isValidationError(e)) setErrs(fieldErrors(e.errors));
      else setErrs({ form: [errorMessage(e)] });
    }
  };

  const exits = (model.data?.listeners ?? [])
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
  return (
    <FormDialog
      onClose={onClose}
      fullWidth
      maxWidth="sm"
      slotProps={{
        transition: {
          onEntered: () => {
            if (focusFallback) fallbackRef.current?.focus();
          },
        },
      }}
    >
      <DialogTitle>{isEdit ? "编辑订阅" : "新建订阅"}</DialogTitle>
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
          <Box component="details" open={focusFallback || undefined}>
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
                  inputRef={fallbackRef}
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
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>取消</Button>
        <Button
          variant="contained"
          disabled={save.isPending || !intervalValid || !validUrl}
          onClick={() => void submit()}
        >
          保存
        </Button>
      </DialogActions>
    </FormDialog>
  );
}

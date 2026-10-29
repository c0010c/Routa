import {
  Alert,
  Button,
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
import { isValidationError, useModel, useSaveSubscription } from "../api/client";
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

export function SubscriptionDialog({ subscription, onClose, focusFallback = false }: {
  subscription: Subscription;
  onClose: () => void;
  focusFallback?: boolean;
}) {
  const model = useModel();
  const save = useSaveSubscription();
  const [name, setName] = useState(subscription.name);
  const [url, setUrl] = useState(subscription.url);
  const [interval, setIntervalMins] = useState(subscription.update_interval_mins);
  const [fallback, setFallback] = useState(subscription.fallback_exit);
  const [intervalValid, setIntervalValid] = useState(true);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });
  const fallbackRef = useRef<HTMLInputElement | null>(null);

  const submit = async () => {
    setErrs({ form: [] });
    try {
      await save.mutateAsync({
        ...subscription,
        name,
        url,
        update_interval_mins: interval,
        fallback_exit: fallback,
      });
      onClose();
    } catch (e) {
      if (isValidationError(e)) setErrs(fieldErrors(e.errors));
    }
  };

  const exits = model.data?.exits ?? [];
  const isEdit = model.data?.subscriptions.some((s) => s.id === subscription.id);
  return (
    <FormDialog
      onClose={onClose}
      fullWidth
      maxWidth="sm"
      slotProps={{ transition: { onEntered: () => { if (focusFallback) fallbackRef.current?.focus(); } } }}
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
            label="名称"
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              setErrs((x) => ({ ...x, name: undefined }));
            }}
            error={!!errs.name}
            helperText={errs.name}
          />
          <TextField size="small" label="URL" value={url} onChange={(e) => setUrl(e.target.value)} />
          <IntervalInput label="自动更新" value={interval} onChange={setIntervalMins} onValidityChange={setIntervalValid} defaultMins={1440} />
          <FormControl size="small">
            <InputLabel id="s-fallback">备用出口</InputLabel>
            <Select
              labelId="s-fallback"
              label="备用出口"
              inputRef={fallbackRef}
              value={fallback ?? ""}
              onChange={(e) => setFallback(e.target.value === "" ? null : e.target.value)}
            >
              <MenuItem value="">不使用</MenuItem>
              {exits.map((x) => (
                <MenuItem key={x.id} value={x.id}>
                  {x.name}
                </MenuItem>
              ))}
            </Select>
            <FormHelperText>直连拉取失败时，通过此出口重试</FormHelperText>
          </FormControl>
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>取消</Button>
        <Button variant="contained" disabled={save.isPending || !intervalValid} onClick={() => void submit()}>
          保存
        </Button>
      </DialogActions>
    </FormDialog>
  );
}

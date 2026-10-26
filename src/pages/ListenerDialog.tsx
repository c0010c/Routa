import {
  Alert,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  FormControlLabel,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  Switch,
  TextField,
} from "@mui/material";
import { useState } from "react";
import { isValidationError, useModel, useSaveListener } from "../api/client";
import type { Listener, ListenerType } from "../bindings";
import { fieldErrors, type FieldErrors } from "../lib/validation";

/** Mirrors `Listener::new_default`; port is the highest used port + 1 (7891 when none). */
export function newListener(existing: Listener[]): Listener {
  return {
    id: crypto.randomUUID(),
    name: "",
    listener_type: "Mixed",
    listen: "0.0.0.0",
    port: existing.length === 0 ? 7891 : Math.max(...existing.map((l) => l.port)) + 1,
    auth: null,
    exit: null,
    user_enabled: true,
  };
}

const TYPES: { value: ListenerType; label: string }[] = [
  { value: "Mixed", label: "混合（HTTP + SOCKS）" },
  { value: "Http", label: "HTTP" },
  { value: "Socks", label: "SOCKS" },
];

export function ListenerDialog({ listener, onClose }: { listener: Listener; onClose: () => void }) {
  const model = useModel();
  const save = useSaveListener();
  const [l, setL] = useState<Listener>(listener);
  const [portText, setPortText] = useState(String(listener.port));
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });
  const [portLocal, setPortLocal] = useState<string>();

  const submit = async () => {
    const port = Number(portText);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      setPortLocal("端口需在 1–65535 之间");
      return;
    }
    setPortLocal(undefined);
    try {
      await save.mutateAsync({ ...l, port });
      onClose();
    } catch (e) {
      if (isValidationError(e)) setErrs(fieldErrors(e.errors));
    }
  };

  const portError = portLocal ?? errs.port;
  const isEdit = model.data?.listeners.some((x) => x.id === l.id);
  return (
    <Dialog open onClose={onClose} fullWidth maxWidth="sm">
      <DialogTitle>{isEdit ? "编辑 Listener" : "新建 Listener"}</DialogTitle>
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
            label="名称"
            size="small"
            value={l.name}
            onChange={(e) => setL({ ...l, name: e.target.value })}
            error={!!errs.name}
            helperText={errs.name}
          />
          <TextField
            label="端口"
            size="small"
            type="number"
            value={portText}
            onChange={(e) => setPortText(e.target.value)}
            error={!!portError}
            helperText={portError}
          />
          <FormControl size="small">
            <InputLabel id="l-type">类型</InputLabel>
            <Select
              labelId="l-type"
              label="类型"
              value={l.listener_type}
              onChange={(e) => setL({ ...l, listener_type: e.target.value as ListenerType })}
            >
              {TYPES.map((t) => (
                <MenuItem key={t.value} value={t.value}>
                  {t.label}
                </MenuItem>
              ))}
            </Select>
          </FormControl>
          <TextField label="监听地址" size="small" value={l.listen} onChange={(e) => setL({ ...l, listen: e.target.value })} />
          <FormControlLabel
            control={
              <Checkbox
                checked={l.auth !== null}
                onChange={(e) => setL({ ...l, auth: e.target.checked ? { username: "", password: "" } : null })}
              />
            }
            label="启用认证"
          />
          {l.auth && (
            <Stack direction="row" spacing={2}>
              <TextField
                label="用户名"
                size="small"
                value={l.auth.username}
                onChange={(e) => setL({ ...l, auth: { username: e.target.value, password: l.auth?.password ?? "" } })}
              />
              <TextField
                label="密码"
                size="small"
                value={l.auth.password}
                onChange={(e) => setL({ ...l, auth: { username: l.auth?.username ?? "", password: e.target.value } })}
              />
            </Stack>
          )}
          <FormControl size="small">
            <InputLabel id="l-exit">出口</InputLabel>
            <Select
              labelId="l-exit"
              label="出口"
              value={l.exit ?? ""}
              onChange={(e) => setL({ ...l, exit: e.target.value === "" ? null : e.target.value })}
            >
              <MenuItem value="">未选择</MenuItem>
              {(model.data?.exits ?? []).map((x) => (
                <MenuItem key={x.id} value={x.id}>
                  {x.name}
                </MenuItem>
              ))}
            </Select>
          </FormControl>
          <FormControlLabel
            control={<Switch checked={l.user_enabled} onChange={(e) => setL({ ...l, user_enabled: e.target.checked })} />}
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
    </Dialog>
  );
}

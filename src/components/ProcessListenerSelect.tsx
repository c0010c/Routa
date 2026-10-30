import { useState } from "react";
import {
  Box,
  Button,
  CircularProgress,
  MenuItem,
  Select,
  Typography,
} from "@mui/material";
import type { Listener, ProcessRule } from "../bindings";
import { desktopApi, useDesktopMutation } from "../api/desktop";
import { errorMessage } from "../api/client";
import { ListenerDialog, newListener } from "../pages/ListenerDialog";

export function ProcessListenerSelect({
  rule,
  listeners,
}: {
  rule: ProcessRule;
  listeners: Listener[];
}) {
  const save = useDesktopMutation((listener: string) =>
    desktopApi.changeListener(rule.id, listener),
  );
  const [failedTarget, setFailedTarget] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [editing, setEditing] = useState<Listener | null>(null);
  const [created, setCreated] = useState<Listener | null>(null);
  const choices = [
    ...listeners,
    ...(created && !listeners.some((l) => l.id === created.id)
      ? [created]
      : []),
  ].filter((l) => l.listener_type !== "Http");
  const value = pending ?? rule.listener;
  const change = async (id: string) => {
    setPending(id);
    setError("");
    try {
      await save.mutateAsync(id);
    } catch (e) {
      setFailedTarget(id);
      setError(errorMessage(e));
    } finally {
      setPending(null);
    }
  };
  return (
    <Box
      onClick={(e) => e.stopPropagation()}
      sx={{ minWidth: 195, maxWidth: 250 }}
    >
      <Select
        fullWidth
        size="small"
        value={value}
        disabled={save.isPending}
        aria-busy={save.isPending}
        inputProps={{
          "aria-label": `${rule.exe_path.split(/[\\/]/).pop()}监听器`,
        }}
        onChange={(e) => {
          if (e.target.value === "__new__") setEditing(newListener(listeners));
          else void change(e.target.value);
        }}
      >
        {!choices.some((l) => l.id === value) && (
          <MenuItem value={value} disabled>
            监听器不可用
          </MenuItem>
        )}
        {choices.map((l) => (
          <MenuItem key={l.id} value={l.id} disabled={!l.user_enabled}>
            {l.port} · {l.name}
            {!l.user_enabled ? "（已停用）" : ""}
          </MenuItem>
        ))}
        <MenuItem value="__new__">＋ 新建监听器…</MenuItem>
      </Select>
      {save.isPending && (
        <Typography role="status" variant="caption" color="text.secondary">
          <CircularProgress size={10} sx={{ mr: 0.5 }} />
          正在应用…
        </Typography>
      )}
      {error && (
        <Typography
          component="div"
          role="alert"
          variant="caption"
          color="error"
          sx={{ mt: 0.5 }}
        >
          {error}
          <Button
            size="small"
            onClick={() => void change(failedTarget ?? rule.listener)}
          >
            重试
          </Button>
        </Typography>
      )}
      {editing && (
        <ListenerDialog
          listener={editing}
          onClose={() => setEditing(null)}
          onSaved={(l) => {
            setCreated(l);
            void change(l.id);
          }}
        />
      )}
    </Box>
  );
}

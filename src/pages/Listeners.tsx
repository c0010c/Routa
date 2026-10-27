import {
  Button,
  Paper,
  Stack,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from "@mui/material";
import { useState } from "react";
import { useDeleteListener, useModel, useSaveListener, useStatus } from "../api/client";
import type { Listener } from "../bindings";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { ListenerStatusChip } from "../components/StatusChips";
import { listenerNotEffective } from "../lib/format";
import { ListenerDialog, newListener } from "./ListenerDialog";

const TYPE_TEXT = { Mixed: "混合", Http: "HTTP", Socks: "SOCKS" } as const;

export function Listeners() {
  const model = useModel();
  const status = useStatus();
  const save = useSaveListener();
  const del = useDeleteListener();
  const [editing, setEditing] = useState<Listener | null>(null);
  const [deleting, setDeleting] = useState<Listener | null>(null);

  if (!model.data || !status.data) return <Typography>加载中…</Typography>;
  const m = model.data;
  const s = status.data;
  const failedIds = new Set(s.failed_listeners.map((f) => f.id));

  return (
    <Stack spacing={2}>
      <Stack direction="row" sx={{ justifyContent: "space-between" }}>
        <Typography variant="h5">Listener</Typography>
        <Button variant="contained" onClick={() => setEditing(newListener(m.listeners))}>
          新建 Listener
        </Button>
      </Stack>
      <TableContainer component={Paper}>
        <Table size="small">
          <TableHead>
            <TableRow>
              {["名称", "端口", "类型", "认证", "出口", "状态", "启用", "操作"].map((h) => (
                <TableCell key={h}>{h}</TableCell>
              ))}
            </TableRow>
          </TableHead>
          <TableBody>
            {m.listeners.map((l) => {
              const ls = s.health.listeners[l.id];
              return (
                <TableRow key={l.id}>
                  <TableCell>{l.name}</TableCell>
                  <TableCell>{l.port}</TableCell>
                  <TableCell>{TYPE_TEXT[l.listener_type]}</TableCell>
                  <TableCell>{l.auth ? l.auth.username : "无"}</TableCell>
                  <TableCell>{m.exits.find((e) => e.id === l.exit)?.name ?? "—"}</TableCell>
                  <TableCell>{ls ? <ListenerStatusChip status={ls} isPortFailed={failedIds.has(l.id)} notEffective={listenerNotEffective(s, l.id)} /> : "—"}</TableCell>
                  <TableCell>
                    <Switch
                      checked={l.user_enabled}
                      slotProps={{ input: { "aria-label": `启用 ${l.name}` } }}
                      onChange={(e) => save.mutate({ ...l, user_enabled: e.target.checked })}
                    />
                  </TableCell>
                  <TableCell>
                    <Button size="small" aria-label={`编辑 ${l.name}`} onClick={() => setEditing(l)}>
                      编辑
                    </Button>
                    <Button size="small" color="error" aria-label={`删除 ${l.name}`} onClick={() => setDeleting(l)}>
                      删除
                    </Button>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </TableContainer>
      {editing && <ListenerDialog key={editing.id} listener={editing} onClose={() => setEditing(null)} />}
      {deleting && (
        <ConfirmDeleteDialog
          text={`确定删除 Listener「${deleting.name}」吗？`}
          onConfirm={() => del.mutateAsync(deleting.id)}
          onClose={() => setDeleting(null)}
        />
      )}
    </Stack>
  );
}

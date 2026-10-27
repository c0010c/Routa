import InputOutlined from "@mui/icons-material/InputOutlined";
import {
  Button,
  Link,
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
import { Link as RouterLink } from "react-router-dom";
import { useDeleteListener, useModel, useSaveListener, useStatus } from "../api/client";
import type { Listener } from "../bindings";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { EmptyState } from "../components/EmptyState";
import { RowActions } from "../components/RowActions";
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
        <Typography variant="h5">监听器</Typography>
        <Button variant="contained" onClick={() => setEditing(newListener(m.listeners))}>
          新建监听器
        </Button>
      </Stack>
      {m.listeners.length === 0 ? (
        <EmptyState
          icon={<InputOutlined />}
          title="还没有监听器"
          description="监听器是本地代理端口。新建后去概览页给它接出口。"
          action={
            <Link component={RouterLink} to="/">
              回概览页按步骤开始 →
            </Link>
          }
        />
      ) : (
          <TableContainer component={Paper}>
            <Table size="small">
              <TableHead>
                <TableRow>
                  {["名称", "端口", "类型", "认证", "出口", "状态", "启用", ""].map((h) => (
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
                      <TableCell>{m.exits.find((e) => e.id === l.exit)?.name ?? "未接出口"}</TableCell>
                      <TableCell>{ls ? <ListenerStatusChip status={ls} isPortFailed={failedIds.has(l.id)} notEffective={listenerNotEffective(s, l.id)} /> : "—"}</TableCell>
                      <TableCell>
                        <Switch
                          checked={l.user_enabled}
                          slotProps={{ input: { "aria-label": `启用 ${l.name}` } }}
                          onChange={(e) => save.mutate({ ...l, user_enabled: e.target.checked })}
                        />
                      </TableCell>
                      <TableCell>
                        <RowActions
                          label={l.name}
                          items={[
                            { label: "编辑", onClick: () => setEditing(l) },
                            { label: "删除", onClick: () => setDeleting(l), danger: true },
                          ]}
                        />
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </TableContainer>
      )}
      {editing && <ListenerDialog key={editing.id} listener={editing} onClose={() => setEditing(null)} />}
      {deleting && (
        <ConfirmDeleteDialog
          text={`确定删除监听器「${deleting.name}」吗？`}
          onConfirm={() => del.mutateAsync(deleting.id)}
          onClose={() => setDeleting(null)}
        />
      )}
    </Stack>
  );
}

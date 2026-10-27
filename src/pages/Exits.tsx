import {
  Button,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from "@mui/material";
import { useState } from "react";
import { useDeleteExit, useModel, useStatus, useTestDelays } from "../api/client";
import type { Exit, Model } from "../bindings";
import { nodeLabel } from "../lib/refs";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { ExitStatusChip } from "../components/StatusChips";
import { ExitDialog, newExit } from "./ExitDialog";

function kindText(e: Exit): string {
  if (e.kind === "Direct") return "直连";
  return "Node" in e.kind ? "节点" : "策略组";
}

function nodeOrMembers(e: Exit, model: Model): string {
  if (e.kind === "Direct") return "—";
  return "Node" in e.kind ? nodeLabel(model, e.kind.Node.node) :`${e.kind.Group.members.length} 个成员`;
}

export function Exits() {
  const model = useModel();
  const status = useStatus();
  const del = useDeleteExit();
  const test = useTestDelays();
  const [editing, setEditing] = useState<Exit | null>(null);
  const [deleting, setDeleting] = useState<Exit | null>(null);

  if (!model.data || !status.data) return <Typography>加载中…</Typography>;
  const m = model.data;
  const s = status.data;

  return (
    <Stack spacing={2}>
      <Stack direction="row" sx={{ justifyContent: "space-between" }}>
        <Typography variant="h5">出口</Typography>
        <Button variant="contained" onClick={() => setEditing(newExit())}>
          新建出口
        </Button>
      </Stack>
      <TableContainer component={Paper}>
        <Table size="small">
          <TableHead>
            <TableRow>
              {["名称", "类型", "节点/成员", "网卡", "延迟", "被引用的 Listener", "状态", "操作"].map((h) => (
                <TableCell key={h}>{h}</TableCell>
              ))}
            </TableRow>
          </TableHead>
          <TableBody>
            {m.exits.map((e) => {
              const h = s.health.exits[e.id];
              const d = s.delays[e.name];
              const iface = e.interface
                ? (s.interfaces.find((i) => i.guid === e.interface)?.friendly_name ??
                  h?.interface?.friendly_name ??
                  "（已移除的网卡）")
                : "系统默认";
              const users = m.listeners.filter((l) => l.exit === e.id).map((l) => l.name);
              return (
                <TableRow key={e.id}>
                  <TableCell>{e.name}</TableCell>
                  <TableCell>{kindText(e)}</TableCell>
                  <TableCell>{nodeOrMembers(e, m)}</TableCell>
                  <TableCell>{iface}</TableCell>
                  <TableCell>{!d ? "—" : d.ms === null ? "超时" : `${d.ms} ms`}</TableCell>
                  <TableCell>{users.length > 0 ? users.join("、") : "—"}</TableCell>
                  <TableCell>{h ? <ExitStatusChip status={h.status} /> : "—"}</TableCell>
                  <TableCell>
                    <Button
                      size="small"
                      aria-label={`测速 ${e.name}`}
                      disabled={test.isPending}
                      onClick={() => test.mutate([e.name])}
                    >
                      测速
                    </Button>
                    <Button size="small" aria-label={`编辑 ${e.name}`} onClick={() => setEditing(e)}>
                      编辑
                    </Button>
                    <Button size="small" color="error" aria-label={`删除 ${e.name}`} onClick={() => setDeleting(e)}>
                      删除
                    </Button>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </TableContainer>
      {editing && <ExitDialog key={editing.id} exit={editing} onClose={() => setEditing(null)} />}
      {deleting && (
        <ConfirmDeleteDialog
          text={`确定删除出口「${deleting.name}」吗？`}
          onConfirm={() => del.mutateAsync(deleting.id)}
          onClose={() => setDeleting(null)}
        />
      )}
    </Stack>
  );
}

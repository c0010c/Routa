import AltRouteOutlined from "@mui/icons-material/AltRouteOutlined";
import {
  Button,
  Link,
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
import { Link as RouterLink } from "react-router-dom";
import { useDeleteExit, useModel, useStatus, useTestDelays } from "../api/client";
import type { Exit, Model } from "../bindings";
import { isGroupMember, nodeLabel } from "../lib/refs";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { DelayText } from "../components/DelayText";
import { EmptyState } from "../components/EmptyState";
import { RowActions } from "../components/RowActions";
import { ExitStatusChip } from "../components/StatusChips";
import { ExitDialog, newExit } from "./ExitDialog";

function kindText(e: Exit): string {
  if (e.kind === "Direct") return "直连";
  return "Node" in e.kind ? "节点" : "策略组";
}

function nodeOrMembers(e: Exit, model: Model): string {
  if (e.kind === "Direct") return "—";
  if ("Node" in e.kind) return nodeLabel(model, e.kind.Node.node);
  const names = e.kind.Group.members.map((mem) => {
    if (!isGroupMember(mem)) return nodeLabel(model, mem);
    if ("Subscription" in mem.group) {
      const id = mem.group.Subscription;
      const n = model.subscriptions.find((x) => x.id === id)?.name;
      return n ? `${n}（整组）` : "（已删除的组）";
    }
    const id = mem.group.Custom;
    const n = model.node_groups.find((x) => x.id === id)?.name;
    return n ? `${n}（整组）` : "（已删除的组）";
  });
  return `${names.length} 个成员：${names.join("、")}`;
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
      {m.exits.length === 0 ? (
        <EmptyState
          icon={<AltRouteOutlined />}
          title="还没有出口"
          description="出口决定流量从哪出去。建好后在概览页把监听器接上它。"
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
                  {["名称", "类型", "节点/成员", "网卡", "延迟", "使用它的监听器", "状态", ""].map((h) => (
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
                      <TableCell>
                        <DelayText ms={d?.ms} />
                      </TableCell>
                      <TableCell>{users.length > 0 ? users.join("、") : "—"}</TableCell>
                      <TableCell>{h ? <ExitStatusChip status={h.status} /> : "—"}</TableCell>
                      <TableCell>
                        <RowActions
                          label={e.name}
                          items={[
                            { label: "测速", onClick: () => test.mutate([e.name]) },
                            { label: "编辑", onClick: () => setEditing(e) },
                            { label: "删除", onClick: () => setDeleting(e), danger: true },
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

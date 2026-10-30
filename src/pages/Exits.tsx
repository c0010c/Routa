import AltRouteOutlined from "@mui/icons-material/AltRouteOutlined";
import {
  Alert,
  Button,
  Link,
  Paper,
  Stack,
  Box,
  Divider,
  Typography,
} from "@mui/material";
import { useEffect, useState } from "react";
import { Link as RouterLink, useSearchParams } from "react-router-dom";
import {
  errorMessage,
  useModel,
  useStatus,
  useTestDelays,
} from "../api/client";
import type { Exit, Model } from "../bindings";
import { isGroupMember, nodeLabel } from "../lib/refs";
import { DelayText } from "../components/DelayText";
import { EmptyState } from "../components/EmptyState";
import { RowActions } from "../components/RowActions";
import { ExitStatusChip } from "../components/StatusChips";
import { ExitDialog, newExit } from "./ExitDialog";
import { ExitDeleteDialog } from "./ExitDeleteDialog";

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
  const test = useTestDelays();
  const [editing, setEditing] = useState<Exit | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [params, setParams] = useSearchParams();
  const requestedDelete = params.get("delete");

  useEffect(() => {
    if (!requestedDelete || !model.data) return;
    setDeleting(requestedDelete);
    setParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.delete("delete");
        return next;
      },
      { replace: true },
    );
  }, [requestedDelete, model.data, setParams]);

  if (!model.data || !status.data) {
    const error = model.error || status.error;
    if (error)
      return (
        <Alert
          severity="error"
          action={
            <Button
              onClick={() => {
                void model.refetch();
                void status.refetch();
              }}
            >
              重新加载
            </Button>
          }
        >
          {errorMessage(error)}
        </Alert>
      );
    return <Typography>加载中…</Typography>;
  }
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
        <Box
          sx={{
            display: "grid",
            gridTemplateColumns: { xs: "1fr", lg: "1fr 1fr" },
            gap: 2,
          }}
        >
          {m.exits.map((e) => {
            const h = s.health.exits[e.id];
            const iface = e.interface
              ? (s.interfaces.find((i) => i.guid === e.interface)
                  ?.friendly_name ??
                h?.interface?.friendly_name ??
                "（已移除的网卡）")
              : "系统默认";
            const users = m.listeners
              .filter((l) => l.exit === e.id)
              .map((l) => l.name);
            return (
              <Paper key={e.id} sx={{ p: 2.5 }}>
                <Stack direction="row" sx={{ alignItems: "center", mb: 2 }}>
                  <Typography sx={{ fontWeight: 600, fontSize: 17, flex: 1 }}>
                    {e.name}
                  </Typography>
                  {h ? <ExitStatusChip status={h.status} /> : "—"}
                  <RowActions
                    label={e.name}
                    items={[
                      {
                        label: "测速",
                        onClick: () => test.mutate([e.name]),
                        disabled: test.isPending,
                      },
                      { label: "编辑", onClick: () => setEditing(e) },
                      {
                        label: "删除",
                        onClick: () => setDeleting(e.id),
                        danger: true,
                      },
                    ]}
                  />
                </Stack>
                <Typography variant="body2" color="text.secondary">
                  {kindText(e)}
                </Typography>
                <Typography sx={{ my: 1, overflowWrap: "anywhere" }}>
                  {nodeOrMembers(e, m)}
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  网卡：{iface}
                </Typography>
                <Divider sx={{ my: 1.5 }} />
                <Stack
                  direction="row"
                  sx={{ alignItems: "center", justifyContent: "space-between" }}
                >
                  <Typography variant="caption" color="text.secondary">
                    {users.length ? users.join("、") : "暂无监听器使用"}
                  </Typography>
                  <DelayText result={s.delays[e.name]} />
                </Stack>
              </Paper>
            );
          })}
        </Box>
      )}
      {editing && (
        <ExitDialog
          key={editing.id}
          exit={editing}
          onClose={() => setEditing(null)}
        />
      )}
      {deleting && (
        <ExitDeleteDialog
          key={deleting}
          exitId={deleting}
          onClose={() => setDeleting(null)}
        />
      )}
    </Stack>
  );
}

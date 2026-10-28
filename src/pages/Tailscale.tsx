import VpnLockOutlined from "@mui/icons-material/VpnLockOutlined";
import { Alert, Box, Button, Card, CardContent, Stack, Typography } from "@mui/material";
import { useState } from "react";
import { useDeleteManualNode, useModel, useStatus, useTestDelays } from "../api/client";
import type { Exit, ManualNode, Model, TailscaleOpts } from "../bindings";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { DelayText } from "../components/DelayText";
import { EmptyState } from "../components/EmptyState";
import { RowActions } from "../components/RowActions";
import {
  TailscaleLoginLink,
  TailscaleStateChip,
  tailscaleProxyName,
  ExitNodeIssueAlert,
  useExitNodeIssue,
  useTailscaleState,
} from "../components/TailscaleState";
import { expandMembers } from "../lib/refs";
import { exitNodeText, isAutoExitNode, useTailnetPeers } from "../lib/tailnet";
import { ExitDialog, newExit } from "./ExitDialog";
import { TailscaleDialog, newTailscaleNode } from "./TailscaleDialog";

export const TAILSCALE_EMPTY_HELP = "通过 tailnet 中的出口节点访问网络，或直接访问 tailnet 内的设备。点击「新建实例」开始。";

type TsNode = ManualNode & { kind: { Tailscale: TailscaleOpts } };
const isTs = (n: ManualNode): n is TsNode => "Tailscale" in n.kind;

function referencingExits(model: Model, id: string): string[] {
  return model.exits
    .filter((e) => {
      if (typeof e.kind === "string") return false;
      if ("Node" in e.kind) {
        const r = e.kind.Node.node;
        return "manual_id" in r && r.manual_id === id;
      }
      return expandMembers(model, e.kind.Group.members).some((r) => "manual_id" in r && r.manual_id === id);
    })
    .map((e) => e.name);
}

function InstanceCard({
  node,
  model,
  onEdit,
  onDelete,
  onCreateExit,
}: {
  node: TsNode;
  model: Model;
  onEdit: () => void;
  onDelete: () => void;
  onCreateExit: () => void;
}) {
  const status = useStatus();
  const test = useTestDelays();
  const state = useTailscaleState(node);
  const exitIssue = useExitNodeIssue(node);
  const loggedIn = state !== "Unknown" && "LoggedIn" in state;
  const { peers } = useTailnetPeers(loggedIn);
  const o = node.kind.Tailscale;
  const name = tailscaleProxyName(node);
  const refs = referencingExits(model, node.id);
  const interfaces = status.data?.interfaces ?? [];
  const iface =
    o.interface === null
      ? "系统默认"
      : (interfaces.find((i) => i.guid === o.interface)?.friendly_name ?? (status.data ? "（已移除的网卡）" : "—"));
  const delay = status.data?.delays[name];

  return (
    <Card component="article" aria-label={node.name}>
      <CardContent>
        <Stack direction="row" sx={{ alignItems: "center", justifyContent: "space-between" }}>
          <Typography variant="h6">{node.name}</Typography>
          <RowActions
            label={node.name}
            items={[
              { label: "编辑", onClick: onEdit },
              { label: "删除", onClick: onDelete, danger: true },
            ]}
          />
        </Stack>
        <Stack direction="row" spacing={1.5} sx={{ alignItems: "center", my: 1 }}>
          <TailscaleStateChip node={node} />
          {loggedIn && <DelayText result={delay} hint="改配置或重启内核后，实例要半分钟到一分钟才能经出口节点上网，这段时间测速会失败" />}
        </Stack>
        <Box sx={{ mb: 1 }}>
          <TailscaleLoginLink node={node} hideChip />
        </Box>
        <Typography variant="body2">出口节点：{exitNodeText(peers, o.exit_node)}</Typography>
        <Typography variant="body2">被出口引用：{refs.length > 0 ? refs.join("、") : "无"}</Typography>
        <Typography variant="body2" color="text.secondary">
          机器名 {o.hostname} · 网卡 {iface} · UDP {o.udp ? "开" : "关"}
          {o.ephemeral ? " · 临时节点" : ""}
        </Typography>
        {exitIssue && (
          <Box sx={{ mt: 1 }}>
            <ExitNodeIssueAlert issue={exitIssue} onChangeExitNode={onEdit} />
          </Box>
        )}
        {!exitIssue && isAutoExitNode(o.exit_node) && (
          <Alert
            severity="warning"
            sx={{ mt: 1 }}
            action={
              <Button size="small" color="inherit" onClick={onEdit}>
                选择出口节点
              </Button>
            }
          >
            {o.exit_node} 在当前内核下无法自动选中出口节点，请改选一个出口节点
          </Alert>
        )}
        {loggedIn && o.exit_node.trim() === "" && (
          <Alert
            severity="info"
            sx={{ mt: 1 }}
            action={
              <Button size="small" color="inherit" onClick={onEdit}>
                设置出口节点
              </Button>
            }
          >
            未设置出口节点，只能访问 tailnet 内的设备
          </Alert>
        )}
        <Box sx={{ display: "flex", gap: 1, mt: 1.5 }}>
          <Button size="small" variant="outlined" disabled={test.isPending || !loggedIn} onClick={() => test.mutate([name])}>
            测速
          </Button>
          <Button size="small" onClick={onCreateExit}>
            用它新建出口
          </Button>
        </Box>
      </CardContent>
    </Card>
  );
}

export function Tailscale() {
  const model = useModel();
  const del = useDeleteManualNode();
  const [editing, setEditing] = useState<ManualNode | null>(null);
  const [deleting, setDeleting] = useState<ManualNode | null>(null);
  const [creatingExit, setCreatingExit] = useState<Exit | null>(null);

  if (!model.data) return <Typography>加载中…</Typography>;
  const m = model.data;
  const nodes = m.manual_nodes.filter(isTs);

  return (
    <Stack spacing={2}>
      <Stack direction="row" sx={{ justifyContent: "space-between" }}>
        <Typography variant="h5">Tailscale</Typography>
        <Button variant="contained" onClick={() => setEditing(newTailscaleNode())}>
          新建实例
        </Button>
      </Stack>
      {nodes.length === 0 && <EmptyState icon={<VpnLockOutlined />} title="还没有 Tailscale 实例" description={TAILSCALE_EMPTY_HELP} />}
      {nodes.map((n) => (
        <InstanceCard
          key={n.id}
          node={n}
          model={m}
          onEdit={() => setEditing(n)}
          onDelete={() => setDeleting(n)}
          onCreateExit={() =>
            setCreatingExit({ ...newExit(), name: n.name, kind: { Node: { node: { manual_id: n.id } } }, interface: n.kind.Tailscale.interface })
          }
        />
      ))}
      {editing && <TailscaleDialog key={editing.id} node={editing} onClose={() => setEditing(null)} />}
      {creatingExit && <ExitDialog key={creatingExit.id} exit={creatingExit} onClose={() => setCreatingExit(null)} />}
      {deleting && (
        <ConfirmDeleteDialog
          text={`确定删除实例「${deleting.name}」吗？登录状态也会一并删除。`}
          onConfirm={() => del.mutateAsync(deleting.id)}
          onClose={() => setDeleting(null)}
        />
      )}
    </Stack>
  );
}

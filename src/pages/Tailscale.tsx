import { Button, Card, CardActions, CardContent, Chip, type ChipProps, Stack, Typography } from "@mui/material";
import { useState } from "react";
import { useDeleteManualNode, useModel, useStatus } from "../api/client";
import type { ManualNode, Model, TailscaleOpts, TailscaleState } from "../bindings";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { MANUAL_PREFIX, expandMembers } from "../lib/refs";
import { TailscaleDialog, newTailscaleNode } from "./TailscaleDialog";

/** Login state of a Tailscale node from the runtime status (keyed by proxy name). */
function useTailscaleState(node: ManualNode): TailscaleState {
  const status = useStatus();
  return status.data?.tailscale?.[`${MANUAL_PREFIX}/${node.name}`] ?? "Unknown";
}

function stateChip(s: TailscaleState): { label: string; color: ChipProps["color"] } {
  if (s === "Unknown") return { label: "状态未知", color: "default" };
  if ("NeedsLogin" in s) return { label: "待登录", color: "warning" };
  if ("LoggedIn" in s) return { label: s.LoggedIn.ip ? `已登录 · ${s.LoggedIn.ip}` : "已登录", color: "success" };
  return { label: `登录失败：${s.Failed.reason}`, color: "error" };
}

/** Compact login state label (also used on the Nodes page's Tailscale rows). */
export function TailscaleStateChip({ node }: { node: ManualNode }) {
  const { label, color } = stateChip(useTailscaleState(node));
  return <Chip size="small" label={label} color={color} />;
}

/** Login state with the authorization link when one is pending. */
export function TailscaleLoginState({ node }: { node: ManualNode }) {
  const s = useTailscaleState(node);
  // Remembers which URL was copied, so a fresh URL offers 复制链接 again.
  const [copiedUrl, setCopiedUrl] = useState<string | null>(null);
  const url = s !== "Unknown" && "NeedsLogin" in s ? s.NeedsLogin.url : null;
  return (
    <Stack spacing={1} sx={{ mt: 1, alignItems: "flex-start" }}>
      <TailscaleStateChip node={node} />
      {s !== "Unknown" && "NeedsLogin" in s && url === null && (
        <Typography variant="body2" color="text.secondary">
          正在获取授权链接…
        </Typography>
      )}
      {url !== null && (
        <>
          <Typography variant="body2" sx={{ userSelect: "text", wordBreak: "break-all", fontFamily: "monospace" }}>
            {url}
          </Typography>
          <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
            <Button
              size="small"
              variant="outlined"
              onClick={() => {
                navigator.clipboard.writeText(url).then(
                  () => setCopiedUrl(url),
                  (e) => console.error("copy failed", e),
                );
              }}
            >
              {copiedUrl === url ? "已复制" : "复制链接"}
            </Button>
            <Typography variant="body2" color="text.secondary">
              在浏览器中打开此链接完成授权
            </Typography>
          </Stack>
        </>
      )}
    </Stack>
  );
}

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

export function Tailscale() {
  const model = useModel();
  const status = useStatus();
  const del = useDeleteManualNode();
  const [editing, setEditing] = useState<ManualNode | null>(null);
  const [deleting, setDeleting] = useState<ManualNode | null>(null);

  if (!model.data) return <Typography>加载中…</Typography>;
  const m = model.data;
  const interfaces = status.data?.interfaces ?? [];
  const nodes = m.manual_nodes.filter(isTs);

  return (
    <Stack spacing={2}>
      <Stack direction="row" sx={{ justifyContent: "space-between" }}>
        <Typography variant="h5">Tailscale</Typography>
        <Button variant="contained" onClick={() => setEditing(newTailscaleNode())}>
          新建实例
        </Button>
      </Stack>
      {nodes.length === 0 && <Typography color="text.secondary">还没有 Tailscale 实例，点击「新建实例」添加。</Typography>}
      {nodes.map((n) => {
        const o = n.kind.Tailscale;
        const refs = referencingExits(m, n.id);
        const iface = o.interface === null ? "系统默认" : (interfaces.find((i) => i.guid === o.interface)?.friendly_name ?? "（已移除的网卡）");
        return (
          <Card key={n.id} component="article" aria-label={n.name} variant="outlined">
            <CardContent>
              <Typography variant="h6">{n.name}</Typography>
              <Typography variant="body2">hostname：{o.hostname}</Typography>
              <Typography variant="body2">exit-node：{o.exit_node}</Typography>
              <Typography variant="body2">
                UDP：{o.udp ? "开" : "关"} · ephemeral：{o.ephemeral ? "是" : "否"}
              </Typography>
              <Typography variant="body2">网卡：{iface}</Typography>
              <Typography variant="body2">被出口引用：{refs.length > 0 ? refs.join("、") : "无"}</Typography>
              <TailscaleLoginState node={n} />
            </CardContent>
            <CardActions>
              <Button size="small" aria-label={`编辑 ${n.name}`} onClick={() => setEditing(n)}>
                编辑
              </Button>
              <Button size="small" color="error" aria-label={`删除 ${n.name}`} onClick={() => setDeleting(n)}>
                删除
              </Button>
            </CardActions>
          </Card>
        );
      })}
      {editing && <TailscaleDialog key={editing.id} node={editing} onClose={() => setEditing(null)} />}
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

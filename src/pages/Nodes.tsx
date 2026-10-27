import HubOutlined from "@mui/icons-material/HubOutlined";
import {
  Alert,
  Box,
  Button,
  Divider,
  List,
  ListItemButton,
  ListItemText,
  ListSubheader,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  TableSortLabel,
  TextField,
  Typography,
} from "@mui/material";
import { useState } from "react";
import { Link as RouterLink } from "react-router-dom";
import {
  useDeleteManualNode,
  useDeleteNodeGroup,
  useDeleteSubscription,
  useModel,
  useRefreshSubscription,
  useStatus,
  useTestDelays,
} from "../api/client";
import type { DelayBook, ManualNode, Model, NodeGroup, Subscription } from "../bindings";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { DelayText } from "../components/DelayText";
import { EmptyState } from "../components/EmptyState";
import { RowActions } from "../components/RowActions";
import { MANUAL_PREFIX } from "../lib/refs";
import { ManualNodeDialog, newManualNode, type ManualNodeMode } from "./ManualNodeDialog";
import { NodeGroupDialog, newNodeGroup } from "./NodeGroupDialog";
import { SubscriptionDialog, newSubscription } from "./SubscriptionDialog";
import { TailscaleStateChip } from "./Tailscale";

function formatInterval(mins: number | null): string {
  if (mins === null) return "已关闭";
  if (mins % 1440 === 0) return `每 ${mins / 1440} 天`;
  if (mins % 60 === 0) return `每 ${mins / 60} 小时`;
  return `每 ${mins} 分钟`;
}

function formatTime(secs: number | null): string {
  return secs === null ? "从未更新" : new Date(secs * 1000).toLocaleString("zh-CN");
}

const subProxyName = (s: Subscription, node: string) => `${s.name}/${node}`;
const manualProxyName = (n: ManualNode) => `${MANUAL_PREFIX}/${n.name}`;

type Selection = { kind: "sub"; id: string } | { kind: "ungrouped" } | { kind: "group"; id: string };

const sameSel = (a: Selection, b: Selection) =>
  a.kind === b.kind && (a.kind === "ungrouped" || (b.kind !== "ungrouped" && a.id === b.id));

export interface Row {
  key: string;
  name: string;
  type: string;
  server: string;
  proxyName: string;
  manual?: ManualNode;
}

function manualRow(n: ManualNode): Row {
  if ("Tailscale" in n.kind) {
    return { key: n.id, name: n.name, type: "tailscale", server: n.kind.Tailscale.hostname, proxyName: manualProxyName(n), manual: n };
  }
  const raw = n.kind.Proxy.raw;
  const o = typeof raw === "object" && raw !== null && !Array.isArray(raw) ? raw : {};
  const s = (v: unknown) => (typeof v === "string" || typeof v === "number" ? String(v) : "");
  return { key: n.id, name: n.name, type: s(o.type), server: s(o.server), proxyName: manualProxyName(n), manual: n };
}

/** Falls back to the first subscription (or 未分组) when the selected item is gone. */
function resolveSelection(model: Model, sel: Selection | null): Selection {
  if (sel?.kind === "sub" && model.subscriptions.some((s) => s.id === sel.id)) return sel;
  if (sel?.kind === "group" && model.node_groups.some((g) => g.id === sel.id)) return sel;
  if (sel?.kind === "ungrouped") return sel;
  const first = model.subscriptions[0];
  return first ? { kind: "sub", id: first.id } : { kind: "ungrouped" };
}

export function Nodes() {
  const model = useModel();
  const status = useStatus();
  const delSub = useDeleteSubscription();
  const delNode = useDeleteManualNode();
  const delGroup = useDeleteNodeGroup();
  const refresh = useRefreshSubscription();
  const test = useTestDelays();
  const apply = useApplyResult();
  const [picked, setPicked] = useState<Selection | null>(null);
  const [editingSub, setEditingSub] = useState<Subscription | null>(null);
  const [deletingSub, setDeletingSub] = useState<Subscription | null>(null);
  const [editingNode, setEditingNode] = useState<{ node: ManualNode; mode?: ManualNodeMode } | null>(null);
  const [deletingNode, setDeletingNode] = useState<ManualNode | null>(null);
  const [editingGroup, setEditingGroup] = useState<NodeGroup | null>(null);
  const [deletingGroup, setDeletingGroup] = useState<NodeGroup | null>(null);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);

  if (!model.data || !status.data) return <Typography>加载中…</Typography>;
  const m = model.data;
  const subs = m.subscriptions;
  const delays = status.data.delays;
  const sel = resolveSelection(m, picked);

  const doRefresh = async (s: Subscription) => {
    setRefreshingId(s.id);
    try {
      const r = await refresh.mutateAsync(s.id);
      if ("Ok" in r) apply.showMessage(`已更新，共 ${r.Ok.node_count} 个节点`, "success");
      else apply.showMessage(r.Failed.reason, "error");
    } catch {
      // Failures are already surfaced by the mutation's onError.
    } finally {
      setRefreshingId(null);
    }
  };

  const allNames = [
    ...subs.flatMap((s) => s.nodes.map((n) => subProxyName(s, n.name))),
    ...m.manual_nodes.map(manualProxyName),
  ];

  const ungrouped = m.manual_nodes.filter((n) => n.groups.length === 0);
  const inGroup = (id: string) => m.manual_nodes.filter((n) => n.groups.includes(id));

  let title: string;
  let rows: Row[];
  let selectedSub: Subscription | undefined;
  if (sel.kind === "sub") {
    selectedSub = subs.find((s) => s.id === sel.id)!;
    title = selectedSub.name;
    rows = selectedSub.nodes.map((n) => ({
      key: n.name,
      name: n.name,
      type: n.proxy_type,
      server: n.server,
      proxyName: subProxyName(selectedSub!, n.name),
    }));
  } else if (sel.kind === "ungrouped") {
    title = "未分组";
    rows = ungrouped.map(manualRow);
  } else {
    title = m.node_groups.find((g) => g.id === sel.id)!.name;
    rows = inGroup(sel.id).map(manualRow);
  }
  const newNodeGroups = sel.kind === "group" ? [sel.id] : [];

  return (
    <Stack spacing={2}>
      <Stack direction="row" sx={{ justifyContent: "space-between" }}>
        <Typography variant="h5">节点</Typography>
        <Button variant="outlined" disabled={test.isPending || allNames.length === 0} onClick={() => test.mutate(allNames)}>
          全部测速
        </Button>
      </Stack>

      <Stack direction="row" spacing={2} sx={{ alignItems: "flex-start" }}>
        <Paper component="nav" aria-label="节点分组" variant="outlined" sx={{ width: 260, flexShrink: 0 }}>
          <List dense disablePadding>
            <ListSubheader sx={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              订阅
              <Button size="small" variant="outlined" onClick={() => setEditingSub(newSubscription())}>
                新建订阅
              </Button>
            </ListSubheader>
            {subs.length === 0 && (
              <Typography variant="body2" color="text.secondary" sx={{ px: 2, pb: 1 }}>
                还没有订阅
              </Typography>
            )}
            {subs.map((s) => {
              const r = s.last_result;
              return (
                <Box key={s.id} sx={{ pb: 1 }}>
                  <Stack direction="row" sx={{ alignItems: "center", pr: 0.5 }}>
                    <ListItemButton
                      selected={sameSel(sel, { kind: "sub", id: s.id })}
                      onClick={() => setPicked({ kind: "sub", id: s.id })}
                      sx={{ flex: 1, minWidth: 0 }}
                    >
                      <ListItemText
                        primary={s.name}
                        secondary={`${s.nodes.length} 个节点 · 最后更新：${formatTime(s.last_updated)} · 自动更新：${formatInterval(s.update_interval_mins)}`}
                      />
                    </ListItemButton>
                    <RowActions
                      label={s.name}
                      items={[
                        { label: "立即更新", onClick: () => void doRefresh(s), disabled: refreshingId === s.id },
                        { label: "编辑", onClick: () => setEditingSub(s) },
                        { label: "删除", onClick: () => setDeletingSub(s), danger: true },
                      ]}
                    />
                  </Stack>
                  {r && "Ok" in r && (
                    <Typography variant="body2" sx={{ px: 2 }}>
                      上次更新成功，共 {r.Ok.node_count} 个节点
                    </Typography>
                  )}
                  {r && "Failed" in r && (
                    <Alert severity="error" sx={{ mx: 2, my: 0.5 }}>
                      更新失败：{r.Failed.reason}
                    </Alert>
                  )}
                </Box>
              );
            })}
            <Divider />
            <ListSubheader>手动节点</ListSubheader>
            <ListItemButton selected={sel.kind === "ungrouped"} onClick={() => setPicked({ kind: "ungrouped" })}>
              <ListItemText primary="未分组" secondary={`${ungrouped.length} 个节点`} />
            </ListItemButton>
            {m.node_groups.map((g) => (
              <Box key={g.id}>
                <Stack direction="row" sx={{ alignItems: "center", pr: 0.5 }}>
                  <ListItemButton
                    selected={sameSel(sel, { kind: "group", id: g.id })}
                    onClick={() => setPicked({ kind: "group", id: g.id })}
                    sx={{ flex: 1, minWidth: 0 }}
                  >
                    <ListItemText primary={g.name} secondary={`${inGroup(g.id).length} 个节点`} />
                  </ListItemButton>
                  <RowActions
                    label={`组 ${g.name}`}
                    items={[
                      { label: "改名", onClick: () => setEditingGroup(g) },
                      { label: "删除", onClick: () => setDeletingGroup(g), danger: true },
                    ]}
                  />
                </Stack>
              </Box>
            ))}
            <ListItemButton onClick={() => setEditingGroup(newNodeGroup())}>
              <ListItemText primary="+ 新建组" />
            </ListItemButton>
          </List>
        </Paper>

        <Stack spacing={1} sx={{ flexGrow: 1, minWidth: 0 }}>
          <Stack direction="row" sx={{ justifyContent: "space-between", alignItems: "center" }}>
            <Typography variant="subtitle1">{title}</Typography>
            {sel.kind !== "sub" && (
              <Stack direction="row" spacing={1}>
                <Button variant="contained" onClick={() => setEditingNode({ node: newManualNode(newNodeGroups), mode: "form" })}>
                  新建节点
                </Button>
                <Button variant="outlined" onClick={() => setEditingNode({ node: newManualNode(newNodeGroups), mode: "yaml" })}>
                  粘贴 YAML
                </Button>
              </Stack>
            )}
          </Stack>
          {rows.length === 0 ? (
            <EmptyState
              icon={<HubOutlined />}
              title={sel.kind === "sub" ? "这个订阅还没有节点" : "还没有节点"}
              description={sel.kind === "sub" ? undefined : "点击「新建节点」或「粘贴 YAML」添加。"}
            />
          ) : (
            <NodeTable
              label={`${title} 的节点`}
              rows={rows}
              delays={delays}
              testing={test.isPending}
              onTest={(pn) => test.mutate([pn])}
              onEdit={(n) => setEditingNode({ node: n })}
              onDelete={setDeletingNode}
            />
          )}
        </Stack>
      </Stack>

      {editingSub && <SubscriptionDialog key={editingSub.id} subscription={editingSub} onClose={() => setEditingSub(null)} />}
      {deletingSub && (
        <ConfirmDeleteDialog
          text={`确定删除订阅「${deletingSub.name}」吗？`}
          onConfirm={() => delSub.mutateAsync(deletingSub.id)}
          onClose={() => setDeletingSub(null)}
        />
      )}
      {editingNode && (
        <ManualNodeDialog
          key={editingNode.node.id}
          node={editingNode.node}
          mode={editingNode.mode}
          onClose={() => setEditingNode(null)}
        />
      )}
      {deletingNode && (
        <ConfirmDeleteDialog
          text={`确定删除节点「${deletingNode.name}」吗？`}
          onConfirm={() => delNode.mutateAsync(deletingNode.id)}
          onClose={() => setDeletingNode(null)}
        />
      )}
      {editingGroup && <NodeGroupDialog key={editingGroup.id} group={editingGroup} onClose={() => setEditingGroup(null)} />}
      {deletingGroup && (
        <ConfirmDeleteDialog
          text={`确定删除组「${deletingGroup.name}」吗？组内节点不会被删除。`}
          onConfirm={() => delGroup.mutateAsync(deletingGroup.id)}
          onClose={() => setDeletingGroup(null)}
        />
      )}
    </Stack>
  );
}

/** Filters by name/server (case-insensitive); optionally sorts by delay, untested and timed-out last. */
export function visibleRows(rows: Row[], delays: DelayBook, query: string, byDelay: boolean): Row[] {
  const q = query.trim().toLowerCase();
  const out = q ? rows.filter((r) => r.name.toLowerCase().includes(q) || r.server.toLowerCase().includes(q)) : rows;
  if (!byDelay) return out;
  const ms = (r: Row) => delays[r.proxyName]?.ms ?? Infinity;
  return [...out].sort((a, b) => ms(a) - ms(b));
}

const ellipsis = { overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" } as const;

function NodeTable({
  label,
  rows,
  delays,
  testing,
  onTest,
  onEdit,
  onDelete,
}: {
  label: string;
  rows: Row[];
  delays: DelayBook;
  testing: boolean;
  onTest: (proxyName: string) => void;
  onEdit: (n: ManualNode) => void;
  onDelete: (n: ManualNode) => void;
}) {
  const [query, setQuery] = useState("");
  const [byDelay, setByDelay] = useState(false);
  const shown = visibleRows(rows, delays, query, byDelay);
  // Manual rows carry edit/delete (or the Tailscale link); subscription rows only 测速.
  const actionsWidth = rows.some((r) => r.manual) ? 200 : 80;
  return (
    <Stack spacing={1}>
      <TextField
        size="small"
        placeholder="搜索名称或服务器"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        slotProps={{ htmlInput: { "aria-label": "搜索节点" } }}
        sx={{ maxWidth: 280 }}
      />
      <TableContainer component={Paper}>
        <Table size="small" aria-label={label} sx={{ tableLayout: "fixed", "& th": { whiteSpace: "nowrap" } }}>
          <TableHead>
            <TableRow>
              <TableCell>名称</TableCell>
              <TableCell sx={{ width: 110 }}>类型</TableCell>
              <TableCell sx={{ width: "28%" }}>服务器</TableCell>
              <TableCell sx={{ width: 90 }} sortDirection={byDelay ? "asc" : false}>
                <TableSortLabel active={byDelay} direction="asc" onClick={() => setByDelay((v) => !v)}>
                  延迟
                </TableSortLabel>
              </TableCell>
              <TableCell sx={{ width: actionsWidth }} />
            </TableRow>
          </TableHead>
          <TableBody>
            {shown.length === 0 && (
              <TableRow>
                <TableCell colSpan={5} sx={{ textAlign: "center", color: "text.secondary" }}>
                  没有匹配「{query.trim()}」的节点
                </TableCell>
              </TableRow>
            )}
            {shown.map((r) => {
              const d = delays[r.proxyName];
              const n = r.manual;
              return (
                <TableRow key={r.key}>
                  <TableCell sx={ellipsis} title={r.name}>
                    {r.name}
                  </TableCell>
                  <TableCell>
                    {r.type}
                    {n && "Tailscale" in n.kind && (
                      <Box component="span" sx={{ ml: 1 }}>
                        <TailscaleStateChip node={n} />
                      </Box>
                    )}
                  </TableCell>
                  <TableCell sx={ellipsis} title={r.server}>
                    {r.server}
                  </TableCell>
                  <TableCell sx={{ whiteSpace: "nowrap" }}>
                    <DelayText ms={d?.ms} />
                  </TableCell>
                  <TableCell sx={{ whiteSpace: "nowrap" }}>
                    <Button size="small" aria-label={`测速 ${r.name}`} disabled={testing} onClick={() => onTest(r.proxyName)}>
                      测速
                    </Button>
                    {n && "Tailscale" in n.kind && (
                      <Button size="small" component={RouterLink} to="/tailscale">
                        去 Tailscale 页编辑
                      </Button>
                    )}
                    {n && "Proxy" in n.kind && (
                      <RowActions
                        label={r.name}
                        items={[
                          { label: "编辑", onClick: () => onEdit(n) },
                          { label: "删除", onClick: () => onDelete(n), danger: true },
                        ]}
                      />
                    )}
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </TableContainer>
    </Stack>
  );
}

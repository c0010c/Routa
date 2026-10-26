import {
  Alert,
  Button,
  Card,
  CardActions,
  CardContent,
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
import {
  useDeleteSubscription,
  useModel,
  useRefreshSubscription,
  useStatus,
  useTestDelays,
} from "../api/client";
import type { Subscription } from "../bindings";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { SubscriptionDialog, newSubscription } from "./SubscriptionDialog";

function formatInterval(mins: number | null): string {
  if (mins === null) return "已关闭";
  if (mins % 1440 === 0) return `每 ${mins / 1440} 天`;
  if (mins % 60 === 0) return `每 ${mins / 60} 小时`;
  return `每 ${mins} 分钟`;
}

function formatTime(secs: number | null): string {
  return secs === null ? "从未更新" : new Date(secs * 1000).toLocaleString("zh-CN");
}

const proxyName = (s: Subscription, node: string) => `${s.name}/${node}`;

export function Nodes() {
  const model = useModel();
  const status = useStatus();
  const del = useDeleteSubscription();
  const refresh = useRefreshSubscription();
  const test = useTestDelays();
  const apply = useApplyResult();
  const [editing, setEditing] = useState<Subscription | null>(null);
  const [deleting, setDeleting] = useState<Subscription | null>(null);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);

  if (!model.data || !status.data) return <Typography>加载中…</Typography>;
  const subs = model.data.subscriptions;
  const delays = status.data.delays;

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

  const allNames = subs.flatMap((s) => s.nodes.map((n) => proxyName(s, n.name)));

  return (
    <Stack spacing={2}>
      <Stack direction="row" sx={{ justifyContent: "space-between" }}>
        <Typography variant="h5">节点</Typography>
        <Stack direction="row" spacing={1}>
          <Button disabled={test.isPending || allNames.length === 0} onClick={() => test.mutate(allNames)}>
            全部测速
          </Button>
          <Button variant="contained" onClick={() => setEditing(newSubscription())}>
            新建订阅
          </Button>
        </Stack>
      </Stack>

      {subs.length === 0 && <Typography color="text.secondary">还没有订阅，点击「新建订阅」添加。</Typography>}

      {subs.map((s) => {
        const r = s.last_result;
        return (
          <Card key={s.id} variant="outlined">
            <CardContent>
              <Typography variant="h6">{s.name}</Typography>
              <Typography variant="body2" color="text.secondary">
                {s.nodes.length} 个节点 · 最后更新：{formatTime(s.last_updated)} · 自动更新：
                {formatInterval(s.update_interval_mins)}
              </Typography>
              {r && "Ok" in r && <Typography variant="body2">上次更新成功，共 {r.Ok.node_count} 个节点</Typography>}
              {r && "Failed" in r && (
                <Alert severity="error" sx={{ mt: 1 }}>
                  更新失败：{r.Failed.reason}
                </Alert>
              )}
            </CardContent>
            <CardActions>
              <Button
                size="small"
                aria-label={`立即更新 ${s.name}`}
                disabled={refreshingId === s.id}
                onClick={() => void doRefresh(s)}
              >
                立即更新
              </Button>
              <Button size="small" aria-label={`编辑 ${s.name}`} onClick={() => setEditing(s)}>
                编辑
              </Button>
              <Button size="small" color="error" aria-label={`删除 ${s.name}`} onClick={() => setDeleting(s)}>
                删除
              </Button>
            </CardActions>
          </Card>
        );
      })}

      {subs
        .filter((s) => s.nodes.length > 0)
        .map((s) => (
          <Stack key={s.id} spacing={1}>
            <Typography variant="subtitle1">{s.name}</Typography>
            <TableContainer component={Paper}>
              <Table size="small" aria-label={`${s.name} 的节点`}>
                <TableHead>
                  <TableRow>
                    {["名称", "类型", "服务器", "延迟", "操作"].map((h) => (
                      <TableCell key={h}>{h}</TableCell>
                    ))}
                  </TableRow>
                </TableHead>
                <TableBody>
                  {s.nodes.map((n) => {
                    const pn = proxyName(s, n.name);
                    const d = delays[pn];
                    return (
                      <TableRow key={n.name}>
                        <TableCell>{n.name}</TableCell>
                        <TableCell>{n.proxy_type}</TableCell>
                        <TableCell>{n.server}</TableCell>
                        <TableCell>{!d ? "—" : d.ms === null ? "超时" : `${d.ms} ms`}</TableCell>
                        <TableCell>
                          <Button
                            size="small"
                            aria-label={`测速 ${n.name}`}
                            disabled={test.isPending}
                            onClick={() => test.mutate([pn])}
                          >
                            测速
                          </Button>
                        </TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
              </Table>
            </TableContainer>
          </Stack>
        ))}

      {editing && <SubscriptionDialog key={editing.id} subscription={editing} onClose={() => setEditing(null)} />}
      {deleting && (
        <ConfirmDeleteDialog
          text={`确定删除订阅「${deleting.name}」吗？`}
          onConfirm={() => del.mutateAsync(deleting.id)}
          onClose={() => setDeleting(null)}
        />
      )}
    </Stack>
  );
}

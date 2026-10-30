import { NodeCard } from "../components/NodeCard";
import { FixedCardGrid } from "../components/FixedCardGrid";
import { SearchField } from "../components/SearchField";
import { BatchNodeDialog } from "./BatchNodeDialog";
import { useEffect, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  Alert,
  Box,
  Button,
  Chip,
  Paper,
  Stack,
  Typography,
} from "@mui/material";
import {
  useModel,
  useStatus,
  useRefreshSubscription,
  useDeleteSubscription,
  useDeleteManualNode,
  useDeleteNodeGroup,
  useTestDelays,
} from "../api/client";
import type { ManualNode, NodeGroup, NodeRef, Subscription } from "../bindings";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { RowActions } from "../components/RowActions";
import { TailscaleStateChip } from "../components/TailscaleState";
import { useInterface } from "../components/InterfaceProvider";
import { desktopApi, useDesktopMutation } from "../api/desktop";
import { nodeKey, expandMembers } from "../lib/refs";
import { SubscriptionDialog, newSubscription } from "./SubscriptionDialog";
import { ManualNodeDialog, newManualNode } from "./ManualNodeDialog";
import { NodeGroupDialog, newNodeGroup } from "./NodeGroupDialog";
import { TailscaleDialog, newTailscaleNode } from "./TailscaleDialog";
import { ListenerDialog, newListener } from "./ListenerDialog";

export function ResourceCenter({
  subscriptionsOnly = false,
}: {
  subscriptionsOnly?: boolean;
}) {
  const [batch, setBatch] = useState(false);
  const model = useModel(),
    runtime = useStatus(),
    refresh = useRefreshSubscription(),
    test = useTestDelays(),
    delSub = useDeleteSubscription(),
    delNode = useDeleteManualNode(),
    delGroup = useDeleteNodeGroup();
  const [params, setParams] = useSearchParams();
  const simple = useInterface()?.state?.preferences.mode === "Simple";
  const choose = useDesktopMutation(desktopApi.select);
  const [missing, setMissing] = useState(false);
  const editId = params.get("editSubscription");
  useEffect(() => {
    if (!editId || !model.data) return;
    const found = model.data.subscriptions.find((s) => s.id === editId);
    setSub(found ?? null);
    setMissing(!found);
    setParams(
      (current) => {
        const next = new URLSearchParams(current);
        next.delete("editSubscription");
        return next;
      },
      { replace: true },
    );
  }, [editId, model.data, setParams]);
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState(false);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [sub, setSub] = useState<Subscription | null>(null),
    [manual, setManual] = useState<ManualNode | null>(null),
    [ts, setTs] = useState<ManualNode | null>(null),
    [group, setGroup] = useState<NodeGroup | null>(null),
    [proxy, setProxy] = useState<NodeRef | null>(null);
  const [deleting, setDeleting] = useState<{
    name: string;
    run: () => Promise<unknown>;
  } | null>(null);
  if (!model.data || !runtime.data)
    return <Typography>正在加载资源…</Typography>;
  const m = model.data,
    st = runtime.data;
  const source = params.get("source");
  const nodes = [
    ...m.subscriptions.flatMap((s) =>
      s.nodes.map((n) => ({
        ref: { subscription_id: s.id, name: n.name } as NodeRef,
        name: n.name,
        source: s.name,
        sourceId: s.id,
        type: n.proxy_type,
        proxy: `${s.name}/${n.name}`,
        manual: undefined as ManualNode | undefined,
      })),
    ),
    ...m.manual_nodes.map((n) => ({
      ref: { manual_id: n.id } as NodeRef,
      name: n.name,
      source: "Tailscale" in n.kind ? "Tailscale" : "手动节点",
      sourceId: "",
      type:
        "Tailscale" in n.kind
          ? "Tailscale"
          : String(
              (n.kind.Proxy.raw as Record<string, unknown>)?.type ?? "代理",
            ),
      proxy: `手动/${n.name}`,
      manual: n,
    })),
  ];
  const memberships = new Map(
    m.node_groups.map((g) => [
      g.id,
      new Set(expandMembers(m, [{ group: { Custom: g.id } }]).map(nodeKey)),
    ]),
  );
  const customMembers = new Set(
    Array.from(memberships.values()).flatMap((ids) => Array.from(ids)),
  );
  const grouped = [
    ...m.node_groups.map((g) => ({
      id: g.id,
      name: g.name,
      group: g,
      nodes: nodes.filter((n) => memberships.get(g.id)?.has(nodeKey(n.ref))),
    })),
    ...m.subscriptions
      .map((subscription) => ({
        id: `subscription:${subscription.id}`,
        name: subscription.name,
        group: undefined as NodeGroup | undefined,
        nodes: nodes.filter(
          (n) =>
            n.sourceId === subscription.id &&
            !customMembers.has(nodeKey(n.ref)),
        ),
      }))
      .filter((g) => g.nodes.length > 0),
    {
      id: "ungrouped",
      name: "未分组",
      group: undefined,
      nodes: nodes.filter(
        (n) => "manual_id" in n.ref && !customMembers.has(nodeKey(n.ref)),
      ),
    },
  ].filter(
    (g) => g.id !== "ungrouped" || g.nodes.length > 0 || nodes.length === 0,
  );
  const toggle = (id: string) =>
    setCollapsed((old) => {
      const next = new Set(old);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  const delay = (name: string) => {
    const d = st.delays[name];
    return d?.stale ? Infinity : d?.ms ?? Infinity;
  };
  return (
    <Stack spacing={2}>
      {params.get("resolveExit") && (
        <Alert
          severity="info"
          action={
            <Button
              component={Link}
              to={`/exits?delete=${encodeURIComponent(params.get("resolveExit")!)}`}
            >
              返回删除检查
            </Button>
          }
        >
          {missing
            ? "该订阅已不存在，请返回重新检查。"
            : "请修改订阅备用代理。"}
        </Alert>
      )}

      <Stack
        direction="row"
        sx={{
          justifyContent: "space-between",
          alignItems: "center",
          flexWrap: "wrap",
          gap: 1,
        }}
      >
        <Typography variant="h5">
          {subscriptionsOnly ? "订阅" : "节点"}
        </Typography>
        <Box>
          {subscriptionsOnly ? (
            <Button
              variant="contained"
              onClick={() => setSub(newSubscription())}
            >
              新增订阅
            </Button>
          ) : (
            <>
              {simple && (
                <Button component={Link} to="/subscriptions">
                  订阅
                </Button>
              )}
              <Button onClick={() => setGroup(newNodeGroup())}>新建分组</Button>
              <Button onClick={() => setTs(newTailscaleNode())}>
                新增 Tailscale
              </Button>
              <Button onClick={() => setBatch(true)}>导入 YAML</Button>
              <Button
                variant="contained"
                onClick={() => setManual(newManualNode())}
              >
                新增节点
              </Button>
            </>
          )}
        </Box>
      </Stack>
      {subscriptionsOnly ? (
        <Box
          sx={{
            height: "calc(100dvh - 150px)",
            minHeight: 300,
            overflowY: "auto",
          }}
        >
          <FixedCardGrid>
            {m.subscriptions.map((s) => (
              <Paper
                component="article"
                aria-label={s.name}
                key={s.id}
                sx={{
                  overflow: "hidden",
                  minWidth: 0,
                  display: "flex",
                  flexDirection: "column",
                }}
              >
                <Stack spacing={1.5} sx={{ p: 2 }}>
                  <Typography variant="h6" noWrap title={s.name}>
                    {s.name}
                  </Typography>
                  <Typography
                    variant="caption"
                    data-selectable="true"
                    sx={{
                      overflowWrap: "anywhere",
                      height: 40,
                      overflowY: "auto",
                    }}
                  >
                    {s.url}
                  </Typography>
                  <Typography>
                    {s.nodes.length} 个节点 ·{" "}
                    {s.update_interval_mins
                      ? `每 ${s.update_interval_mins} 分钟更新`
                      : "手动更新"}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    {s.last_updated
                      ? new Date(s.last_updated * 1000).toLocaleString("zh-CN")
                      : "尚未更新"}
                  </Typography>
                  <Typography variant="body2">
                    备用代理：
                    {m.listeners.find((l) => l.exit === s.fallback_exit)
                      ?.name ??
                      (s.fallback_exit ? "旧配置（请重新选择代理）" : "不使用")}
                  </Typography>
                  {s.last_result && "Failed" in s.last_result && (
                    <Alert severity="error">
                      {s.last_result.Failed.reason}
                    </Alert>
                  )}
                </Stack>
                <Box
                  sx={{
                    p: 1.5,
                    mt: "auto",
                    flexWrap: "wrap",
                    borderTop: 1,
                    borderColor: "divider",
                    display: "flex",
                    gap: 1,
                  }}
                >
                  <Button
                    disabled={refresh.isPending}
                    onClick={() => refresh.mutate(s.id)}
                  >
                    更新
                  </Button>
                  <Button
                    component={Link}
                    to={`/nodes?source=${encodeURIComponent(s.id)}`}
                  >
                    查看节点
                  </Button>
                  <Button onClick={() => setSub(s)}>编辑</Button>
                  <Button
                    color="error"
                    onClick={() =>
                      setDeleting({
                        name: s.name,
                        run: () => delSub.mutateAsync(s.id),
                      })
                    }
                  >
                    删除
                  </Button>
                </Box>
              </Paper>
            ))}
          </FixedCardGrid>
          {!m.subscriptions.length && (
            <Typography sx={{ p: 5, textAlign: "center" }}>
              暂无订阅，添加后可更新节点。
            </Typography>
          )}
        </Box>
      ) : (
        <>
          <Stack
            direction="row"
            sx={{ alignItems: "center", gap: 1, flexWrap: "wrap" }}
          >
            <SearchField
              label="搜索名称、来源、协议或分组"
              value={search}
              onChange={setSearch}
              sx={{ flex: 1, minWidth: 220 }}
            />
            <Button onClick={() => setSort(!sort)}>
              {sort ? "恢复默认排序" : "延迟排序"}
            </Button>
            <Button
              disabled={test.isPending}
              onClick={() =>
                test.mutate(
                  nodes
                    .filter((n) => !source || n.sourceId === source)
                    .map((n) => n.proxy),
                )
              }
            >
              {test.isPending ? "测量中…" : "测延迟"}
            </Button>
            <Button onClick={() => setCollapsed(new Set())}>全部展开</Button>
            <Button
              onClick={() => setCollapsed(new Set(grouped.map((g) => g.id)))}
            >
              全部折叠
            </Button>
          </Stack>
          {source && (
            <Chip
              sx={{ alignSelf: "flex-start" }}
              label={`来源：${m.subscriptions.find((s) => s.id === source)?.name ?? "已删除订阅"}`}
              onDelete={() => setParams({})}
            />
          )}
          <Box
            sx={{
              height: "calc(100dvh - 240px)",
              minHeight: 300,
              overflowY: "auto",
              scrollbarGutter: "stable",
            }}
          >
            {grouped.map((g) => {
              const q = search.trim().toLowerCase();
              let shown = g.nodes.filter(
                (n) =>
                  (!source || n.sourceId === source) &&
                  `${n.name} ${n.source} ${n.type} ${g.name}`
                    .toLowerCase()
                    .includes(q),
              );
              if (sort)
                shown = [...shown].sort(
                  (a, b) => delay(a.proxy) - delay(b.proxy),
                );
              if ((q || source) && !shown.length) return null;
              const open = !!q || !collapsed.has(g.id);
              return (
                <Paper key={g.id} sx={{ mb: 1.5, overflow: "hidden" }}>
                  <Box
                    sx={{
                      display: "flex",
                      alignItems: "center",
                      px: 1.5,
                      height: 40,
                      bgcolor: "action.hover",
                    }}
                  >
                    <Button
                      aria-expanded={open}
                      onClick={() => toggle(g.id)}
                      sx={{ flex: 1, justifyContent: "flex-start", fontSize: 13 }}
                    >
                      {open ? "▾" : "▸"} {g.name} · {shown.length}
                    </Button>
                    {g.id.startsWith("subscription:") && (
                      <Chip size="small" label="订阅" sx={{ mr: 1 }} />
                    )}
                    {g.group && (
                      <RowActions
                        label={g.name}
                        items={[
                          {
                            label: "编辑分组",
                            onClick: () => setGroup(g.group!),
                          },
                          {
                            label: "删除分组",
                            danger: true,
                            onClick: () =>
                              setDeleting({
                                name: g.name,
                                run: () => delGroup.mutateAsync(g.id),
                              }),
                          },
                        ]}
                      />
                    )}
                  </Box>
                  {open && (
                    <>
                      <FixedCardGrid width={220} stretch animate sx={{ p: 1.5, gap: 1.5 }}>
                        {shown.map((n) => (
                          <NodeCard
                            key={nodeKey(n.ref)}
                            name={n.name}
                            source={n.source}
                            protocol={n.type}
                            delay={st.delays[n.proxy]}
                            gridKey={nodeKey(n.ref)}
                            testTarget={n.proxy}
                            testUrl={m.settings.delay_test_url}
                            running={st.core === "Running"}
                            pending={test.isPending && !!test.variables?.[0]?.includes(n.proxy)}
                            status={
                              n.manual && "Tailscale" in n.manual.kind ? (
                                <TailscaleStateChip node={n.manual} compact />
                              ) : undefined
                            }
                            actions={[
                              {
                                label: simple ? "使用此节点" : "用于新代理",
                                onClick: () =>
                                  simple
                                    ? choose.mutate(n.ref)
                                    : setProxy(n.ref),
                              },
                              {
                                label: "测延迟",
                                disabled: test.isPending,
                                onClick: () => test.mutate([n.proxy]),
                              },
                              ...(n.manual
                                ? [
                                    {
                                      label:
                                        "Tailscale" in n.manual.kind
                                          ? "登录与出口设置"
                                          : "编辑",
                                      onClick: () =>
                                        "Tailscale" in n.manual!.kind
                                          ? setTs(n.manual!)
                                          : setManual(n.manual!),
                                    },
                                    {
                                      label: "删除",
                                      danger: true,
                                      onClick: () =>
                                        setDeleting({
                                          name: n.name,
                                          run: () =>
                                            delNode.mutateAsync(n.manual!.id),
                                        }),
                                    },
                                  ]
                                : []),
                            ]}
                          />
                        ))}
                      </FixedCardGrid>
                      {!shown.length && (
                        <Typography
                          sx={{
                            p: 4,
                            textAlign: "center",
                            color: "text.secondary",
                          }}
                        >
                          暂无节点
                        </Typography>
                      )}
                    </>
                  )}
                </Paper>
              );
            })}
            {(search || source) &&
              !grouped.some((g) =>
                g.nodes.some(
                  (n) =>
                    (!source || n.sourceId === source) &&
                    `${n.name} ${n.source} ${n.type} ${g.name}`
                      .toLowerCase()
                      .includes(search.trim().toLowerCase()),
                ),
              ) && (
                <Typography sx={{ p: 5, textAlign: "center" }}>
                  未找到匹配项
                </Typography>
              )}
          </Box>
          <Typography variant="caption" color="text.secondary">
            共 {nodes.length} 个节点（跨分组去重）
          </Typography>
        </>
      )}
      {batch && <BatchNodeDialog onClose={() => setBatch(false)} />}
      {sub && (
        <SubscriptionDialog
          key={sub.id}
          subscription={sub}
          focusFallback={!!params.get("resolveExit")}
          onClose={() => setSub(null)}
        />
      )}
      {manual && (
        <ManualNodeDialog
          key={manual.id}
          node={manual}
          onClose={() => setManual(null)}
        />
      )}
      {ts && (
        <TailscaleDialog key={ts.id} node={ts} onClose={() => setTs(null)} />
      )}
      {group && (
        <NodeGroupDialog
          key={group.id}
          group={group}
          onClose={() => setGroup(null)}
        />
      )}
      {proxy && (
        <ListenerDialog
          listener={{
            ...newListener(m.listeners),
            name:
              nodes.find((n) => nodeKey(n.ref) === nodeKey(proxy))?.name ??
              "新代理",
          }}
          initialNode={proxy}
          onClose={() => setProxy(null)}
        />
      )}
      {deleting && (
        <ConfirmDeleteDialog
          text={`确定删除「${deleting.name}」吗？被引用的节点需要先解除引用。`}
          onConfirm={deleting.run}
          onClose={() => setDeleting(null)}
        />
      )}
    </Stack>
  );
}

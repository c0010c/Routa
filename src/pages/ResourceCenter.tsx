import ArrowBackOutlined from "@mui/icons-material/ArrowBackOutlined";
import { visibleListeners } from "../lib/hidden";
import { NodeCard } from "../components/NodeCard";
import { FixedCardGrid } from "../components/FixedCardGrid";
import { SearchField } from "../components/SearchField";
import { BatchNodeDialog } from "./BatchNodeDialog";
import { useEffect, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import ArrowDropDownIcon from "@mui/icons-material/ArrowDropDown";
import NetworkCheck from "@mui/icons-material/NetworkCheck";
import SwapVert from "@mui/icons-material/SwapVert";
import UnfoldLess from "@mui/icons-material/UnfoldLess";
import UnfoldMore from "@mui/icons-material/UnfoldMore";
import {
  Alert,
  Box,
  Button,
  Chip,
  Divider,
  IconButton,
  Menu,
  MenuItem,
  Paper,
  Stack,
  Tooltip,
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
import { MonitorMark } from "../components/MonitorMark";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { RowActions } from "../components/RowActions";
import { TailscaleStateChip } from "../components/TailscaleState";
import { useInterface } from "../components/InterfaceProvider";
import { desktopApi, useDesktopMutation } from "../api/desktop";
import { nodeKey, expandMembers } from "../lib/refs";
import { clearMonitorFocus, useMonitorFocus } from "../api/monitorFocus";
import { useSettingsSave, settingsSaveMessage } from "../api/settings";
import { EMPTY_MONITOR_STATUS, fetchInUseNodes, skippedMessage, targetsForNode, withNode, withoutNode } from "../lib/monitor";
import { SubscriptionDialog, newSubscription } from "./SubscriptionDialog";
import { ManualNodeDialog, newManualNode } from "./ManualNodeDialog";
import { NodeGroupDialog, newNodeGroup } from "./NodeGroupDialog";
import { TailscaleDialog, newTailscaleNode } from "./TailscaleDialog";
import { ListenerDialog, newListener } from "./ListenerDialog";

const FOCUS_MS = 3000;

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
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState(false);
  // Per-group open state set by the user; groups without an entry use defaultOpen.
  const [openOverride, setOpenOverride] = useState<Map<string, boolean>>(new Map());
  // Open state chosen while searching; only applies to the query it was made under, so clearing the search restores openOverride.
  const [searchOverride, setSearchOverride] = useState<{ query: string; open: Map<string, boolean> }>({ query: "", open: new Map() });
  const saveSettings = useSettingsSave();
  const apply = useApplyResult();
  const focus = useMonitorFocus();
  // The node a toast click pointed at. Its sections stay open until the user toggles one;
  // the card is outlined for FOCUS_MS.
  const [highlight, setHighlight] = useState<{ key: string; lit: boolean; seq: number } | null>(null);
  const focusSeq = useRef(0);
  // Sort by the delays from before a test: results land one by one and must not reshuffle the grid mid-run.
  const sortDelays = useRef(runtime.data?.delays);
  if (!test.isPending) sortDelays.current = runtime.data?.delays;
  const loaded = !!model.data && !!runtime.data;
  useEffect(() => {
    if (subscriptionsOnly || !focus || typeof focus !== "object") return;
    // Clear whatever could hide the card.
    setSearch("");
    if (params.has("source")) setParams({});
    setHighlight({ key: nodeKey(focus.Node), lit: true, seq: ++focusSeq.current });
    clearMonitorFocus();
  }, [focus, subscriptionsOnly, params, setParams]);
  useEffect(() => {
    if (!highlight || !loaded) return;
    const card = Array.from(document.querySelectorAll<HTMLElement>("[data-grid-key]")).find(
      (el) => el.dataset.gridKey === highlight.key,
    );
    card?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [highlight?.seq, loaded]);
  useEffect(() => {
    if (!highlight?.lit) return;
    const timer = setTimeout(() => setHighlight({ ...highlight, lit: false }), FOCUS_MS);
    return () => clearTimeout(timer);
  }, [highlight]);
  const [newMenu, setNewMenu] = useState<HTMLElement | null>(null);
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
  const monitor = st.monitor ?? EMPTY_MONITOR_STATUS;
  const toggleMonitor = async (ref: NodeRef, watched: boolean) => {
    const nm = m.settings.node_monitor;
    let settings;
    if (watched) settings = withoutNode(nm, m, ref);
    else {
      // Turning monitoring on from a card: the nodes in use come along, as in settings.
      const inUse = !nm.enabled && nm.members.length === 0 ? await fetchInUseNodes() : { nodes: [], auto_exits: [] };
      if (inUse.auto_exits.length) apply.showMessage(skippedMessage(inUse.auto_exits), "info");
      settings = withNode(nm, ref, inUse.nodes);
    }
    saveSettings.mutate(
      { node_monitor: settings },
      { onError: (e) => apply.showMessage(settingsSaveMessage(e), "error") },
    );
  };
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
  const delay = (name: string) => {
    const d = (sortDelays.current ?? st.delays)[name];
    return d?.stale ? Infinity : d?.ms ?? Infinity;
  };
  const q = search.trim().toLowerCase();
  // Subscription sections start collapsed (unless opened from 订阅 → 查看节点); groups and 未分组 start open.
  const defaultOpen = (id: string) => !id.startsWith("subscription:") || !!source;
  // A search opens every matching section unless the user collapsed it during this same search.
  const searchOpen = searchOverride.query === q ? searchOverride.open : new Map<string, boolean>();
  const focusedSections = new Set(
    highlight ? grouped.filter((g) => g.nodes.some((n) => nodeKey(n.ref) === highlight.key)).map((g) => g.id) : [],
  );
  const isOpen = (id: string) =>
    focusedSections.has(id) ||
    (q ? searchOpen.get(id) ?? true : openOverride.get(id) ?? defaultOpen(id));
  const setOpen = (entries: [string, boolean][]) => {
    setHighlight(null);
    return q
      ? setSearchOverride({ query: q, open: new Map([...searchOpen, ...entries]) })
      : setOpenOverride((old) => new Map([...old, ...entries]));
  };
  const sections = grouped.flatMap((g) => {
    let shown = g.nodes.filter(
      (n) =>
        (!source || n.sourceId === source) &&
        `${n.name} ${n.source} ${n.type} ${g.name}`.toLowerCase().includes(q),
    );
    if (sort) shown = [...shown].sort((a, b) => delay(a.proxy) - delay(b.proxy));
    if ((q || source) && !shown.length) return [];
    return [{ ...g, shown, open: isOpen(g.id) }];
  });
  const toggle = (id: string) => setOpen([[id, !isOpen(id)]]);
  const anyCollapsed = sections.some((g) => !g.open);
  const setAll = (open: boolean) => setOpen(grouped.map((g) => [g.id, open]));
  const create = (run: () => void) => () => {
    setNewMenu(null);
    run();
  };
  return (
    <Stack spacing={2}>

      <Stack
        direction="row"
        sx={{
          justifyContent: "space-between",
          alignItems: "center",
          flexWrap: "wrap",
          gap: 1,
        }}
      >
        <Stack direction="row" spacing={1} sx={{ alignItems: "center" }}>
          {simple && (
            <IconButton
              component={Link}
              to={subscriptionsOnly ? "/nodes" : "/"}
              aria-label={subscriptionsOnly ? "返回节点" : "返回连接"}
            >
              <ArrowBackOutlined />
            </IconButton>
          )}
          <Typography variant="h5">
            {subscriptionsOnly ? "订阅" : "节点"}
          </Typography>
        </Stack>
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
              <Button
                variant="contained"
                endIcon={<ArrowDropDownIcon />}
                aria-haspopup="menu"
                aria-expanded={newMenu !== null}
                onClick={(e) => setNewMenu(e.currentTarget)}
              >
                新增
              </Button>
              <Menu
                anchorEl={newMenu}
                open={newMenu !== null}
                onClose={() => setNewMenu(null)}
                anchorOrigin={{ vertical: "bottom", horizontal: "right" }}
                transformOrigin={{ vertical: "top", horizontal: "right" }}
              >
                <MenuItem onClick={create(() => setManual(newManualNode()))}>新增节点</MenuItem>
                <MenuItem onClick={create(() => setTs(newTailscaleNode()))}>新增 Tailscale</MenuItem>
                <MenuItem onClick={create(() => setBatch(true))}>导入 YAML</MenuItem>
                <Divider />
                <MenuItem onClick={create(() => setGroup(newNodeGroup()))}>新建分组</MenuItem>
              </Menu>
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
                    {visibleListeners(m).find((l) => l.exit === s.fallback_exit)
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
            <Button
              variant="outlined"
              startIcon={<NetworkCheck />}
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
            <Tooltip title={sort ? "恢复默认排序" : "按延迟排序"}>
              <IconButton
                aria-label="按延迟排序"
                aria-pressed={sort}
                color={sort ? "primary" : "default"}
                onClick={() => setSort(!sort)}
                sx={sort ? { bgcolor: "action.selected" } : undefined}
              >
                <SwapVert />
              </IconButton>
            </Tooltip>
            <Tooltip title={anyCollapsed ? "全部展开" : "全部折叠"}>
              <IconButton
                aria-label={anyCollapsed ? "全部展开" : "全部折叠"}
                onClick={() => setAll(anyCollapsed)}
              >
                {anyCollapsed ? <UnfoldMore /> : <UnfoldLess />}
              </IconButton>
            </Tooltip>
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
            {sections.map(({ shown, open, ...g }) => {
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
                        {shown.map((n) => {
                          const mt = targetsForNode(monitor, n.ref);
                          return (
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
                              pending={test.waiting(n.proxy, st.delays[n.proxy])}
                              monitorFocus={!!highlight?.lit && highlight.key === nodeKey(n.ref)}
                              status={
                                // One mark slot: a down node outranks the Tailscale state.
                                mt.some((t) => t.down) ? (
                                  <MonitorMark targets={mt} />
                                ) : n.manual && "Tailscale" in n.manual.kind ? (
                                  <TailscaleStateChip node={n.manual} compact />
                                ) : mt.length ? (
                                  <MonitorMark targets={mt} />
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
                                ...(simple
                                  ? []
                                  : [
                                      {
                                        label: mt.length ? "移出监测" : "加入监测",
                                        disabled: saveSettings.isPending,
                                        onClick: () => void toggleMonitor(n.ref, mt.length > 0),
                                      },
                                    ]),
                              ]}
                            />
                          );
                        })}
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
            {(q || source) && !sections.length && (
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
            ...newListener(visibleListeners(m)),
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

import { useState, type KeyboardEvent } from "react";
import { Box, Button, Typography } from "@mui/material";
import { useModel, useStatus, useTestDelays } from "../api/client";
import { candidateNodeGroups } from "../lib/nodeGroups";
import { nodeKey, nodeLabel, proxyNameOf } from "../lib/refs";
import { FixedCardGrid } from "./FixedCardGrid";
import { NodeCard } from "./NodeCard";
import { SearchField } from "./SearchField";
import type { NodeRef } from "../bindings";

/** Shared state behind a searchable, filterable, sortable grid of node cards with delay testing. */
export function useNodeBrowser(members: NodeRef[]) {
  const model = useModel(),
    status = useStatus(),
    test = useTestDelays();
  const [search, setSearch] = useState("");
  const [filterGroup, setFilterGroup] = useState("");
  const [sort, setSort] = useState(false);
  const [measurementOrder, setMeasurementOrder] = useState<string[]>([]);
  const m = model.data;
  const nodes = members.map((ref) => {
    const sub =
      "subscription_id" in ref
        ? m?.subscriptions.find((s) => s.id === ref.subscription_id)
        : undefined;
    const manual =
      "manual_id" in ref
        ? m?.manual_nodes.find((n) => n.id === ref.manual_id)
        : undefined;
    const n =
      sub && "name" in ref
        ? sub.nodes.find((n) => n.name === ref.name)
        : undefined;
    const source =
      sub?.name ??
      (manual && "Tailscale" in manual.kind ? "Tailscale" : "手动节点");
    const type =
      n?.proxy_type ??
      (manual && "Tailscale" in manual.kind
        ? "Tailscale"
        : manual && "Proxy" in manual.kind
          ? String(
              (manual.kind.Proxy.raw as Record<string, unknown>)?.type ??
                "代理",
            )
          : "代理");
    const proxy = m ? proxyNameOf(m, ref) : null;
    return { ref, name: m ? nodeLabel(m, ref) : "", source, type, proxy };
  });
  const filters = m ? candidateNodeGroups(m, members) : [];
  const activeFilter = filters.find((item) => item.id === filterGroup);
  const shown = nodes.filter((n) =>
    (!activeFilter || activeFilter.keys.has(nodeKey(n.ref))) && `${n.name} ${n.source} ${n.type}`
      .toLowerCase()
      .includes(search.trim().toLowerCase()),
  );
  if (sort) {
    const score = (proxy: string | null) => {
      const delay = proxy ? status.data?.delays[proxy] : undefined;
      return delay?.stale ? Infinity : delay?.ms ?? Infinity;
    };
    shown.sort((a, b) => test.isPending
      ? measurementOrder.indexOf(nodeKey(a.ref)) - measurementOrder.indexOf(nodeKey(b.ref))
      : score(a.proxy) - score(b.proxy));
  }
  const measure = (proxies: string[]) => {
    setMeasurementOrder(nodes.map((n) => n.ref).sort((a, b) => {
      const delay = (ref: NodeRef) => {
        const proxy = m ? proxyNameOf(m, ref) : null;
        const result = proxy ? status.data?.delays[proxy] : undefined;
        return result?.stale ? Infinity : result?.ms ?? Infinity;
      };
      return delay(a) - delay(b);
    }).map(nodeKey));
    test.mutate(proxies);
  };
  return {
    m, status, test, nodes, shown, filters, activeFilter,
    search, setSearch, setFilterGroup, sort, setSort, measure,
  };
}
export type NodeBrowserState = ReturnType<typeof useNodeBrowser>;

export function NodeBrowserToolbar({ browser }: { browser: NodeBrowserState }) {
  const { search, setSearch, sort, setSort, test, shown, measure, filters, activeFilter, setFilterGroup } = browser;
  return (
    <Box sx={{ px: 3, pb: 2, flexShrink: 0 }}>
      <Box sx={{ display: "flex", gap: 1, alignItems: "center", flexWrap: "wrap" }}>
        <SearchField
          label="搜索节点、来源或协议"
          value={search}
          onChange={setSearch}
          sx={{ flex: 1, minWidth: 180 }}
        />
        <Button disabled={test.isPending} onClick={() => setSort(!sort)}>{sort ? "恢复默认排序" : "延迟排序"}</Button>
        <Button
          disabled={test.isPending || !shown.length}
          onClick={() => measure(shown.flatMap((n) => (n.proxy ? [n.proxy] : [])))}
        >
          {test.isPending ? "测量中…" : "测延迟"}
        </Button>
      </Box>
      <Box role="group" aria-label="按分组筛选" sx={{ display: "flex", gap: 0.75, alignItems: "center", flexWrap: "wrap", mt: 1.5 }}>
        <Typography variant="caption" color="text.secondary">分组</Typography>
        {[{ id: "", name: "全部分组" }, ...filters].map((item) => <Button key={item.id} size="small"
          aria-pressed={item.id === (activeFilter?.id ?? "")} onClick={() => setFilterGroup(item.id)}
          variant={item.id === (activeFilter?.id ?? "") ? "contained" : "outlined"}
          sx={{ borderRadius: 5, py: 0.25, minHeight: 28, fontSize: 12 }}>{item.name}</Button>)}
      </Box>
    </Box>
  );
}

export function NodeBrowserGrid({
  browser, chosenKey, currentKey, disabled = false, arrowsSelect, onPick,
}: {
  browser: NodeBrowserState;
  chosenKey: string | null;
  currentKey: string | null;
  disabled?: boolean;
  /** Arrow keys also pick the focused card (draft selection) instead of only moving focus. */
  arrowsSelect: boolean;
  onPick: (ref: NodeRef) => void;
}) {
  const { shown, status, test, m, measure } = browser;
  const tabKey = shown.some((n) => nodeKey(n.ref) === chosenKey) ? chosenKey : shown[0] ? nodeKey(shown[0].ref) : null;
  const navigate = (e: KeyboardEvent<HTMLButtonElement>) => {
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(e.key)) return;
    const grid = e.currentTarget.closest('[role="radiogroup"]');
    if (!grid) return;
    const buttons = Array.from(grid.querySelectorAll<HTMLButtonElement>('button[role="radio"]:not(:disabled)'));
    const columns = getComputedStyle(grid).gridTemplateColumns.split(/\s+/).filter((v) => v && v !== "none").length || 1;
    const step = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -columns, ArrowDown: columns }[e.key] ?? 0;
    const at = buttons.indexOf(e.currentTarget);
    const next = e.key === "Home" ? 0 : e.key === "End" ? buttons.length - 1 : Math.max(0, Math.min(buttons.length - 1, at + step));
    e.preventDefault();
    // When picking acts immediately, arrows only move focus; Enter/Space pick.
    if (arrowsSelect) buttons[next]?.click();
    buttons[next]?.focus();
  };
  return (
    <FixedCardGrid width={220} stretch animate sx={{ gap: 1.5 }} role="radiogroup" aria-label="候选节点">
      {shown.map((n) => {
        const key = nodeKey(n.ref);
        const delay = n.proxy ? status.data?.delays[n.proxy] : undefined;
        return (
          <NodeCard
            key={key}
            gridKey={key}
            name={n.name} source={n.source} protocol={n.type} delay={delay}
            selected={key === chosenKey} current={key === currentKey}
            disabled={disabled}
            selectionTabIndex={key === tabKey ? 0 : -1}
            onSelectionKeyDown={navigate}
            running={status.data?.core === "Running"}
            testTarget={n.proxy ?? undefined} testUrl={m?.settings.delay_test_url}
            pending={test.isPending && !!n.proxy && !!test.variables?.[0]?.includes(n.proxy)}
            actions={[{ label: "测延迟", disabled: test.isPending || !n.proxy, onClick: () => n.proxy && measure([n.proxy]) }]}
            onSelect={() => onPick(n.ref)}
          />
        );
      })}
    </FixedCardGrid>
  );
}

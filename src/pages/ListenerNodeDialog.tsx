import { useState, type KeyboardEvent } from "react";
import {
  Alert,
  Box,
  Button,
  DialogActions,
  DialogContent,
  DialogTitle,
  Typography,
} from "@mui/material";
import {
  errorMessage,
  isValidationError,
  useModel,
  useSaveExit,
  useSelectGroupMember,
  useStatus,
  useTestDelays,
} from "../api/client";
import { FormDialog } from "../components/FormDialog";
import { FixedCardGrid } from "../components/FixedCardGrid";
import { NodeCard } from "../components/NodeCard";
import { SearchField } from "../components/SearchField";
import { candidateNodeGroups, ungroupedManualNodes } from "../lib/nodeGroups";
import { expandMembers, nodeKey, nodeLabel, proxyNameOf } from "../lib/refs";
import { applyOutcomeMessages } from "../lib/format";
import { fieldErrors } from "../lib/validation";
import type { ExitMember, NodeRef } from "../bindings";

export function ListenerNodeDialog({
  exitId,
  listenerName,
  onClose,
  draft,
  onSelect,
  onSwitched,
}: {
  exitId: string;
  listenerName: string;
  onClose: () => void;
  draft?: { members: ExitMember[]; selected: NodeRef | null };
  onSelect?: (node: NodeRef, members: ExitMember[]) => void;
  /** Called after a saved (non-draft) switch has taken effect. */
  onSwitched?: () => void;
}) {
  const model = useModel(),
    status = useStatus(),
    change = useSelectGroupMember(),
    saveExit = useSaveExit(),
    test = useTestDelays();
  const [search, setSearch] = useState("");
  const [filterGroup, setFilterGroup] = useState("");
  const [sort, setSort] = useState(false);
  const [measurementOrder, setMeasurementOrder] = useState<string[]>([]);
  const [selected, setSelected] = useState<NodeRef | null>(null);
  const [error, setError] = useState("");
  const m = model.data;
  const target = m?.exits.find((e) => e.id === exitId);
  const savedGroup =
    target &&
    typeof target.kind === "object" &&
    "Group" in target.kind &&
    target.kind.Group.strategy === "Select"
      ? target.kind.Group
      : null;
  const group = draft ?? savedGroup;
  // Without a manual group, a fixed-node exit offers every node; confirming re-points the exit at the chosen one.
  const fixedNode =
    !group && target && typeof target.kind === "object" && "Node" in target.kind
      ? target.kind.Node.node
      : null;
  const available = !!group || !!fixedNode;
  const configuredMembers = m && group ? expandMembers(m, group.members) : [];
  const members = !m ? []
    : group ? expandMembers(m, [...group.members, ...ungroupedManualNodes(m)])
    : fixedNode ? expandMembers(m, [
      ...m.subscriptions.map((s): ExitMember => ({ group: { Subscription: s.id } })),
      ...m.manual_nodes.map((n): ExitMember => ({ manual_id: n.id })),
    ])
    : [];
  const current = group?.selected ?? fixedNode;
  const currentKey = current ? nodeKey(current) : null;
  const chosen = selected ?? current;
  const chosenKey = chosen ? nodeKey(chosen) : null;
  // Only a manual group can grow: picking a node outside its members adds it on confirmation.
  const joinsGroup = !!group && chosenKey !== null && !configuredMembers.some((ref) => nodeKey(ref) === chosenKey);
  const busy = change.isPending || saveExit.isPending;
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
  const valid =
    chosenKey !== null && members.some((r) => nodeKey(r) === chosenKey);
  // On the proxy page a click switches right away; inside the editing form it only picks a draft value.
  const direct = !draft;
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
    // Picking switches immediately on the proxy page, so arrows only move focus there; Enter/Space switch.
    if (!direct) buttons[next]?.click();
    buttons[next]?.focus();
  };
  const submit = async (node: NodeRef | null = chosen) => {
    if (!available || !node || busy || !members.some((r) => nodeKey(r) === nodeKey(node))) return;
    setError("");
    const joins = !!group && !configuredMembers.some((r) => nodeKey(r) === nodeKey(node));
    if (draft) {
      onSelect?.(node, joins ? [...draft.members, node] : draft.members);
      onClose();
      return;
    }
    if (!target) return;
    try {
      const outcome = savedGroup
        // Preserve the strict member-selection command; newly exposed nodes join only when picked.
        ? joins
          ? await saveExit.mutateAsync({ ...target, kind: { Group: { ...savedGroup, members: [...savedGroup.members, node], selected: node } } })
          : await change.mutateAsync(exitId, node)
        : await saveExit.mutateAsync({ ...target, kind: { Node: { node } } });
      if ("NotRunning" in outcome) {
        onClose();
        return;
      }
      if (applyOutcomeMessages(outcome).length) {
        setError("选择已保存，尚未生效，请检查内核状态。");
        return;
      }
      onClose();
      onSwitched?.();
    } catch (e) {
      if (isValidationError(e)) {
        const f = fieldErrors(e.errors);
        setError([f.name, f.port, ...f.form].filter(Boolean).join("；"));
      } else setError(errorMessage(e));
    }
  };
  return (
    <FormDialog onClose={onClose} fullWidth maxWidth={false} transitionDuration={160} sx={{
      "& .MuiDialog-paper": { width: 1008, maxWidth: "calc(100vw - 40px)", animation: "routa-node-dialog-in 160ms cubic-bezier(.2,.8,.2,1)" },
      "@keyframes routa-node-dialog-in": { from: { opacity: 0, transform: "translateY(6px)" }, to: { opacity: 1, transform: "translateY(0)" } },
      "@media (prefers-reduced-motion: reduce)": { "& .MuiDialog-paper": { animation: "none" }, "& .MuiDialog-container, & .MuiBackdrop-root": { transition: "none !important" } },
    }}>
      <DialogTitle>
        {draft ? "选择节点" : "切换节点"} · {listenerName}
      </DialogTitle>
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
          onClick={() =>
            measure(shown.flatMap((n) => (n.proxy ? [n.proxy] : [])))
          }
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
      <DialogContent dividers sx={{ px: 3, py: 2, borderBottom: 0, scrollbarGutter: "stable" }}>
        {!available && (
          <Alert severity="warning">
            该代理的出口已变更，请关闭后重新打开。
          </Alert>
        )}
        {error && (
          <Alert severity="error" sx={{ mb: 2 }}>
            {error}
          </Alert>
        )}
        <FixedCardGrid width={220} stretch animate sx={{ gap: 1.5 }} role="radiogroup" aria-label="候选节点">
          {shown.map((n) => {
            const key = nodeKey(n.ref);
            const active = key === chosenKey;
            const delay = n.proxy ? status.data?.delays[n.proxy] : undefined;
            return (
              <NodeCard
                key={key}
                gridKey={key}
                name={n.name} source={n.source} protocol={n.type} delay={delay}
                selected={active} current={key === currentKey}
                disabled={busy}
                selectionTabIndex={key === tabKey ? 0 : -1}
                onSelectionKeyDown={navigate}
                running={status.data?.core === "Running"}
                testTarget={n.proxy ?? undefined} testUrl={m?.settings.delay_test_url}
                pending={test.isPending && !!n.proxy && !!test.variables?.[0]?.includes(n.proxy)}
                actions={[{ label: "测延迟", disabled: test.isPending || !n.proxy, onClick: () => n.proxy && measure([n.proxy]) }]}
                onSelect={() => {
                  setSelected(n.ref);
                  setError("");
                  if (!direct) return;
                  if (key === currentKey) onClose();
                  else void submit(n.ref);
                }}
              />
            );
          })}
        </FixedCardGrid>
        {!shown.length && (
          <Typography
            sx={{ p: 5, textAlign: "center", color: "text.secondary" }}
          >
            {search || activeFilter ? "未找到匹配节点" : group ? "策略暂无候选节点，请先编辑代理配置。" : "暂无可选节点，请先添加订阅或节点。"}
          </Typography>
        )}
      </DialogContent>
      <DialogActions sx={{ justifyContent: "space-between" }}>
        <Box sx={{ mr: "auto", minWidth: 0, flex: 1, height: 44, overflow: "hidden" }} aria-live="polite">
        <Typography variant="body2" noWrap>
          {direct ? `正在使用：${current && m ? nodeLabel(m, current) : "未选择"}` : `已选择：${chosen && m ? nodeLabel(m, chosen) : "未选择"}`}
        </Typography>
        <Box sx={{ display: "flex", gap: 1, alignItems: "center" }}>
        <Typography variant="caption" color="text.secondary" noWrap sx={{ minWidth: 0 }}>
          {direct
            ? busy ? `正在切换到 ${chosen && m ? nodeLabel(m, chosen) : ""}…` : "点击节点即切换"
            : joinsGroup ? "保存时加入当前代理" : "保存代理后生效"}
        </Typography>
        <Typography component="span"
          variant="caption"
          color="text.secondary"
          sx={{ whiteSpace: "nowrap", flexShrink: 0 }}
        >
          {shown.length} / {nodes.length} 个候选节点
        </Typography>
        </Box>
        </Box>
        <Box sx={{ display: "flex", gap: 1, flexShrink: 0 }}>
        <Button onClick={onClose}>{direct ? "关闭" : "取消"}</Button>
        {!direct && <Button
          variant="contained"
          disabled={!available || !valid || chosenKey === currentKey}
          onClick={() => void submit()}
        >
          选择节点
        </Button>}
        </Box>
      </DialogActions>
    </FormDialog>
  );
}

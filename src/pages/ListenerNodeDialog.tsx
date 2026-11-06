import { useState } from "react";
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
} from "../api/client";
import { FormDialog } from "../components/FormDialog";
import { NodeBrowserGrid, NodeBrowserToolbar, useNodeBrowser } from "../components/NodeBrowser";
import { ungroupedManualNodes } from "../lib/nodeGroups";
import { expandMembers, nodeKey, nodeLabel } from "../lib/refs";
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
    change = useSelectGroupMember(),
    saveExit = useSaveExit();
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
  const browser = useNodeBrowser(members);
  const { nodes, shown, search: searchText, activeFilter } = browser;
  const valid =
    chosenKey !== null && members.some((r) => nodeKey(r) === chosenKey);
  // On the proxy page a click switches right away; inside the editing form it only picks a draft value.
  const direct = !draft;
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
      <NodeBrowserToolbar browser={browser} />
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
        <NodeBrowserGrid
          browser={browser} chosenKey={chosenKey} currentKey={currentKey} disabled={busy} arrowsSelect={!direct}
          onPick={(ref) => {
            setSelected(ref);
            setError("");
            if (!direct) return;
            if (nodeKey(ref) === currentKey) onClose();
            else void submit(ref);
          }}
        />
        {!shown.length && (
          <Typography
            sx={{ p: 5, textAlign: "center", color: "text.secondary" }}
          >
            {searchText || activeFilter ? "未找到匹配节点" : group ? "策略暂无候选节点，请先编辑代理配置。" : "暂无可选节点，请先添加订阅或节点。"}
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

import { Box, DialogActions, DialogContent, DialogTitle, Button, Typography } from "@mui/material";
import { useModel } from "../api/client";
import { expandMembers, nodeKey } from "../lib/refs";
import { FormDialog } from "./FormDialog";
import { NodeBrowserGrid, NodeBrowserToolbar, useNodeBrowser } from "./NodeBrowser";
import type { ExitMember, NodeRef } from "../bindings";

/** Picks one single node (subscription or manual, incl. Tailscale); a click calls `onPick`, the caller closes. */
export function NodePickerDialog({ title, value, onClose, onPick }: {
  title: string;
  value: NodeRef | null;
  onClose: () => void;
  onPick: (node: NodeRef) => void;
}) {
  const m = useModel().data;
  const members = m ? expandMembers(m, [
    ...m.subscriptions.map((s): ExitMember => ({ group: { Subscription: s.id } })),
    ...m.manual_nodes.map((n): ExitMember => ({ manual_id: n.id })),
  ]) : [];
  const browser = useNodeBrowser(members);
  const valueKey = value ? nodeKey(value) : null;
  const { shown, nodes, search, activeFilter } = browser;
  return (
    <FormDialog onClose={onClose} fullWidth maxWidth={false} sx={{ "& .MuiDialog-paper": { width: 1008, maxWidth: "calc(100vw - 40px)" } }}>
      <DialogTitle>{title}</DialogTitle>
      <NodeBrowserToolbar browser={browser} />
      <DialogContent dividers sx={{ px: 3, py: 2, borderBottom: 0, scrollbarGutter: "stable" }}>
        <NodeBrowserGrid browser={browser} chosenKey={valueKey} currentKey={valueKey} arrowsSelect={false} onPick={onPick} />
        {!shown.length && (
          <Typography sx={{ p: 5, textAlign: "center", color: "text.secondary" }}>
            {search || activeFilter ? "未找到匹配节点" : "暂无可选节点，请先添加订阅或节点。"}
          </Typography>
        )}
      </DialogContent>
      <DialogActions sx={{ justifyContent: "space-between" }}>
        <Typography component="span" variant="caption" color="text.secondary">
          {shown.length} / {nodes.length} 个候选节点
        </Typography>
        <Box><Button onClick={onClose}>取消</Button></Box>
      </DialogActions>
    </FormDialog>
  );
}

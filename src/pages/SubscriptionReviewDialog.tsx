import { Alert, Box, Button, DialogActions, DialogContent, DialogTitle, MenuItem, Stack, TextField, Typography } from "@mui/material";
import { useState } from "react";
import type { NodeRef, SubscriptionPreview } from "../bindings";
import { FormDialog } from "../components/FormDialog";
import { BusyLabel } from "../components/BusyLabel";
import { subscriptionHost } from "../lib/subscriptions";

export function SubscriptionReviewDialog({ preview, busy, onConfirm, onClose, error }: {
  preview: SubscriptionPreview; busy: boolean; onConfirm: (replacement: NodeRef | null) => void; onClose: () => void; error?: string;
}) {
  const [replacement, setReplacement] = useState("");
  const p = preview;
  return <FormDialog onClose={onClose} busy={busy} fullWidth maxWidth="sm">
    <DialogTitle>预览并确认订阅</DialogTitle>
    <DialogContent><Stack spacing={2} sx={{ pt: 1 }}>
      <Box sx={{ p: 2, bgcolor: "action.hover", borderRadius: 1 }}>
        <Typography sx={{ fontWeight: 600 }}>{p.subscription.name}</Typography>
        <Typography variant="caption" color="text.secondary">{subscriptionHost(p.subscription.url)}</Typography>
        <Typography sx={{ mt: 1.5, fontSize: 23, fontWeight: 600 }}>{p.subscription.nodes.length} <Box component="span" sx={{ fontSize: 13, fontWeight: 400 }}>个节点</Box></Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mt: .5 }}>新增 {p.added.length} 个 · 移除 {p.removed.length} 个</Typography>
      </Box>
      {!!p.skipped_tailscale && <Alert severity="info">已跳过 {p.skipped_tailscale} 个 Tailscale 节点，请在本地单独添加。</Alert>}
      {!!p.affected.length && <>
        <Alert severity="warning">被移除的节点仍由以下位置使用：{p.affected.join("、")}。选择替代节点后应用，相关连接可能重建。</Alert>
        <TextField select size="small" label="替代节点" value={replacement} onChange={e => setReplacement(e.target.value)} disabled={busy}>
          {p.subscription.nodes.map(n => <MenuItem key={n.name} value={n.name}>{n.name}</MenuItem>)}
        </TextField>
      </>}
      <Typography sx={{ fontWeight: 600 }}>节点预览</Typography>
      <Box sx={{ border: 1, borderColor: "divider", borderRadius: 1 }}>
        {p.subscription.nodes.slice(0, 8).map(n => <Box key={n.name} sx={{ display: "flex", justifyContent: "space-between", gap: 2, px: 1.5, py: 1, borderBottom: 1, borderColor: "divider" }}>
          <Typography variant="body2" sx={{ overflowWrap: "anywhere" }}>{n.name}</Typography><Typography variant="caption" color="text.secondary">{n.proxy_type}</Typography>
        </Box>)}
      </Box>
      {p.subscription.nodes.length > 8 && <Typography variant="caption" color="text.secondary">另有 {p.subscription.nodes.length - 8} 个节点，确认后可查看全部。</Typography>}
      {!!p.removed.length && <Box component="details"><Typography component="summary" variant="body2" sx={{ cursor: "pointer" }}>查看移除的 {p.removed.length} 个节点</Typography><Typography variant="body2" sx={{ mt: 1, overflowWrap: "anywhere" }}>{p.removed.join("、")}</Typography></Box>}
      {error && <Alert severity="error">{error}</Alert>}
    </Stack></DialogContent>
    <DialogActions><Button disabled={busy} onClick={onClose}>保留原配置</Button><Button variant="contained" disabled={busy || !!p.affected.length && !replacement} onClick={() => onConfirm(replacement ? { subscription_id: p.subscription.id, name: replacement } : null)}><BusyLabel busy={busy} idle="确认并应用" pending="正在应用…" /></Button></DialogActions>
  </FormDialog>;
}

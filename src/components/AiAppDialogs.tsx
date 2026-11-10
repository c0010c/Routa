import { useState } from "react";
import { Box, Button, Checkbox, Dialog, DialogActions, DialogContent, DialogTitle, FormControlLabel, Typography } from "@mui/material";
import type { AiAppConflict } from "../bindings";

/** Lists the process rules that also route the app's processes; confirming removes them from those rules. */
export function ConflictDialog({ conflicts, onCancel, onConfirm }: {
  conflicts: AiAppConflict[];
  onCancel(): void;
  onConfirm(): void;
}) {
  return (
    <Dialog open onClose={onCancel}>
      <DialogTitle>与进程分流规则冲突</DialogTitle>
      <DialogContent>
        {conflicts.map((c) => (
          <Box key={`${c.rule_id}:${c.process}`} sx={{ mb: 1.5 }}>
            <Typography variant="body2">{`规则「${c.rule_name}」里的 ${c.process} 也在分流。`}</Typography>
            <Typography variant="caption" color="text.secondary">
              {c.removes_rule ? "开启后会删除该规则。" : "开启后会把它从该规则中移除，规则里的其他进程不受影响。"}
            </Typography>
          </Box>
        ))}
      </DialogContent>
      <DialogActions>
        <Button onClick={onCancel}>取消</Button>
        <Button variant="contained" onClick={onConfirm}>移除并开启</Button>
      </DialogActions>
    </Dialog>
  );
}

/** Warns that the app's existing connections get cut so it reconnects through the new route. */
export function ResetDialog({ appName, onCancel, onConfirm }: {
  appName: string;
  onCancel(): void;
  onConfirm(dontAskAgain: boolean): void;
}) {
  const [dontAsk, setDontAsk] = useState(false);
  return (
    <Dialog open onClose={onCancel}>
      <DialogTitle>{`断开 ${appName} 的现有连接`}</DialogTitle>
      <DialogContent>
        <Typography variant="body2">{`会断开 ${appName} 已有的连接，让它重新连接。应用可能会短暂显示重连。`}</Typography>
        <FormControlLabel
          sx={{ mt: 1 }}
          control={<Checkbox checked={dontAsk} onChange={(e) => setDontAsk(e.target.checked)} />}
          label="下次不再提示"
        />
      </DialogContent>
      <DialogActions>
        <Button onClick={onCancel}>取消</Button>
        <Button variant="contained" onClick={() => onConfirm(dontAsk)}>继续</Button>
      </DialogActions>
    </Dialog>
  );
}

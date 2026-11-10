// Shown when TUN is chosen without admin rights, in both the simple and full views.
import { useState } from "react";
import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Typography,
} from "@mui/material";
import { errorMessage } from "../api/client";
import { desktopApi } from "../api/desktop";
import { BusyLabel } from "./BusyLabel";

export function AdminRelaunchDialog({ onClose }: { onClose: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  return (
    <Dialog open onClose={() => !busy && onClose()} maxWidth="xs" fullWidth>
      <DialogTitle>TUN 需要管理员权限</DialogTitle>
      <DialogContent>
        <Typography variant="body2">
          Routa 会以管理员身份重新启动，重启后自动开启 TUN。Windows 会请求授权；拒绝授权则保持当前状态。
        </Typography>
        {error && (
          <Alert severity="error" sx={{ mt: 2 }}>
            {error}
          </Alert>
        )}
      </DialogContent>
      <DialogActions>
        <Button disabled={busy} onClick={onClose}>
          取消
        </Button>
        <Button
          variant="contained"
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            setError("");
            try {
              await desktopApi.relaunchAsAdmin(true);
            } catch (e) {
              setError(errorMessage(e) === "已取消" ? "已取消授权，保持当前连接方式。" : errorMessage(e));
              setBusy(false);
            }
          }}
        >
          <BusyLabel busy={busy} idle="以管理员身份重启" pending="正在重启…" />
        </Button>
      </DialogActions>
    </Dialog>
  );
}

import InfoOutlined from "@mui/icons-material/InfoOutlined";
import { Button, IconButton, Paper, Stack, Tooltip, Typography } from "@mui/material";
import { useState } from "react";
import type { UpdateState } from "../bindings";
import { checkUpdate, installUpdate, useUpdate } from "../api/update";
import { errorMessage } from "../api/client";
import { formatBytes } from "../lib/format";
import { UAC_NOTE } from "../lib/updateCopy";

function statusText(state: UpdateState, last: number | null): string | null {
  switch (state.kind) {
    case "Idle":
      return last == null ? null : "已是最新版本";
    case "Checking":
      return "正在检查…";
    case "Downloading": {
      const p =
        state.total != null && state.total > 0
          ? ` ${Math.min(100, Math.floor((state.received / state.total) * 100))}%`
          : ` ${formatBytes(state.received)}`;
      return `正在下载 v${state.version}${p}`;
    }
    case "Ready":
      return `v${state.version} 已就绪`;
    case "Failed":
      return `更新失败：${state.message}`;
    default:
      return null;
  }
}

export function UpdatePanel() {
  const { data } = useUpdate();
  const [error, setError] = useState<string | null>(null);
  if (!data) return null;
  const { state } = data;
  const busy = state.kind === "Checking" || state.kind === "Downloading";
  const run = async (f: () => Promise<unknown>) => {
    setError(null);
    try {
      await f();
    } catch (e) {
      setError(errorMessage(e));
    }
  };
  const text = statusText(state, data.last_checked_unix);
  return (
    <Stack spacing={2}>
      <Stack direction="row" spacing={0.5} sx={{ alignItems: "center" }}>
        <Typography sx={{ fontWeight: 600 }}>更新</Typography>
        <Tooltip title="仅安装版（NSIS）支持自动更新；安装前会校验签名">
          <IconButton size="small" aria-label="更新说明" sx={{ p: 0.25 }}>
            <InfoOutlined sx={{ fontSize: 16 }} />
          </IconButton>
        </Tooltip>
      </Stack>
      <Paper>
        <Stack spacing={1} sx={{ p: 2, alignItems: "flex-start" }}>
          <Typography>当前版本 v{data.current_version}</Typography>
          {state.kind === "Unsupported" ? (
            <Typography variant="body2" color="text.secondary">
              当前安装方式不支持自动更新
            </Typography>
          ) : (
            <>
              <Stack direction="row" spacing={1} sx={{ alignItems: "center", minHeight: 32 }}>
                <Button
                  size="small"
                  variant="outlined"
                  disabled={busy}
                  onClick={() => void run(checkUpdate)}
                >
                  检查更新
                </Button>
                {state.kind === "Ready" && (
                  <Button size="small" variant="contained" onClick={() => void run(installUpdate)}>
                    现在安装
                  </Button>
                )}
              </Stack>
              {state.kind === "Ready" && (
                <Typography variant="body2" color="text.secondary">
                  {UAC_NOTE}
                </Typography>
              )}
              {text && (
                <Typography variant="body2" color="text.secondary">
                  {text}
                </Typography>
              )}
              {error && (
                <Typography variant="body2" color="error">
                  {error}
                </Typography>
              )}
            </>
          )}
        </Stack>
      </Paper>
    </Stack>
  );
}

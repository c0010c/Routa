import InfoOutlined from "@mui/icons-material/InfoOutlined";
import { Box, Button, IconButton, Tooltip, Typography } from "@mui/material";
import { Row, Section } from "./SettingsSection";
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
  const unsupported = state.kind === "Unsupported";
  return (
    <Section
      title="更新"
      action={
        <Tooltip title="仅安装版（NSIS）支持自动更新；安装前会校验签名">
          <IconButton size="small" aria-label="更新说明" sx={{ p: 0.25 }}>
            <InfoOutlined sx={{ fontSize: 16 }} />
          </IconButton>
        </Tooltip>
      }
    >
      {[
        <Row
          key="version"
          label={`当前版本 v${data.current_version}`}
          description={
            unsupported ? (
              "当前安装方式不支持自动更新"
            ) : (
              (text || state.kind === "Ready") && (
                <>
                  {text}
                  {state.kind === "Ready" && <Box component="span" sx={{ display: "block" }}>{UAC_NOTE}</Box>}
                </>
              )
            )
          }
          below={
            error && (
              <Typography variant="body2" color="error">
                {error}
              </Typography>
            )
          }
        >
          {!unsupported && (
            <>
              <Button
                variant="outlined"
                disabled={busy}
                onClick={() => void run(checkUpdate)}
              >
                检查更新
              </Button>
              {state.kind === "Ready" && (
                <Button variant="contained" onClick={() => void run(installUpdate)}>
                  现在安装
                </Button>
              )}
            </>
          )}
        </Row>,
      ]}
    </Section>
  );
}

import ArrowUpwardRounded from "@mui/icons-material/ArrowUpwardRounded";
import { Button, CircularProgress, IconButton, Popover, Stack, Tooltip, Typography } from "@mui/material";
import { alpha } from "@mui/material/styles";
import { useState } from "react";
import { errorMessage } from "../api/client";
import { installUpdate, useUpdate } from "../api/update";
import { UAC_NOTE } from "../lib/updateCopy";

/** Round "update" button that sits on the sidebar's 设置 row once an installer is Ready. */
export function UpdateNavButton() {
  const { data } = useUpdate();
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const state = data?.state;
  if (!state || state.kind !== "Ready") return null;
  const install = async () => {
    setError(null);
    setInstalling(true);
    try {
      await installUpdate();
    } catch (e) {
      setError(errorMessage(e));
      setInstalling(false);
    }
  };
  return (
    <>
      <Tooltip title={installing ? "正在安装…" : `更新到 v${state.version}`} placement="right">
        <span>
          <IconButton
            aria-label={`更新到 v${state.version}`}
            disabled={installing}
            onClick={(e) => setAnchor(e.currentTarget)}
            sx={{
              width: 26,
              height: 26,
              p: 0,
              color: "primary.main",
              bgcolor: (t) => alpha(t.palette.primary.main, 0.12),
              "&:hover": { bgcolor: (t) => alpha(t.palette.primary.main, 0.2) },
            }}
          >
            {installing ? (
              <CircularProgress size={14} />
            ) : (
              <ArrowUpwardRounded sx={{ fontSize: 16 }} />
            )}
          </IconButton>
        </span>
      </Tooltip>
      <Popover
        open={anchor != null && !installing}
        anchorEl={anchor}
        onClose={() => setAnchor(null)}
        anchorOrigin={{ vertical: "center", horizontal: "right" }}
        transformOrigin={{ vertical: "bottom", horizontal: "left" }}
        slotProps={{ paper: { sx: { ml: 1, p: 2, width: 280 } } }}
      >
        <Stack spacing={1}>
          <Typography sx={{ fontWeight: 600 }}>v{state.version} 已就绪</Typography>
          {state.notes && (
            <Typography variant="body2" sx={{ whiteSpace: "pre-line", maxHeight: 160, overflow: "auto" }}>
              {state.notes}
            </Typography>
          )}
          <Typography variant="body2" color="text.secondary">
            安装时 Routa 会退出，代理会短暂断开，装完自动重启。{UAC_NOTE}
          </Typography>
          {error && (
            <Typography variant="body2" color="error">
              {error}
            </Typography>
          )}
          <Stack direction="row" spacing={1} sx={{ pt: 0.5 }}>
            <Button size="small" variant="contained" onClick={() => void install()}>
              现在安装
            </Button>
            <Button size="small" onClick={() => setAnchor(null)}>
              稍后
            </Button>
          </Stack>
        </Stack>
      </Popover>
    </>
  );
}

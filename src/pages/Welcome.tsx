import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Radio,
  Stack,
  Typography,
} from "@mui/material";
import RouteOutlined from "@mui/icons-material/RouteOutlined";
import { useState } from "react";
import { useInterface } from "../components/InterfaceProvider";
import { errorMessage } from "../api/client";
import type { InterfaceMode } from "../api/desktop";

export function Welcome() {
  const ui = useInterface()!;
  const [mode, setMode] = useState<InterfaceMode>("Simple");
  const [error, setError] = useState("");
  return (
    <Stack
      spacing={3}
      sx={{ maxWidth: 520, mx: "auto", p: 3.5, minHeight: "100vh" }}
    >
      <Stack direction="row" spacing={1.5} sx={{ alignItems: "center" }}>
        <Box
          sx={{
            p: 0.75,
            borderRadius: 1,
            bgcolor: "primary.main",
            color: "primary.contrastText",
            display: "flex",
          }}
        >
          <RouteOutlined />
        </Box>
        <Typography sx={{ fontSize: 20, fontWeight: 600 }}>Routa</Typography>
      </Stack>
      <Box sx={{ pt: 2 }}>
        <Typography variant="h5">先选一种使用方式</Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
          以后可在设置中更改
        </Typography>
      </Box>
      {(
        [
          ["Simple", "简洁版", "选节点、开代理，按需选择应用"],
          ["Full", "完整版", "多代理端口与精细分流"],
        ] as const
      ).map(([value, title, description]) => (
        <Button
          key={value}
          variant="outlined"
          onClick={() => setMode(value)}
          aria-pressed={mode === value}
          sx={{
            textAlign: "left",
            justifyContent: "flex-start",
            p: 2,
            gap: 1.5,
            borderColor: mode === value ? "primary.main" : "divider",
            bgcolor: mode === value ? "action.selected" : "background.paper",
          }}
        >
          <Radio checked={mode === value} tabIndex={-1} sx={{ p: 0 }} />
          <Box sx={{ flex: 1 }}>
            <Typography sx={{ fontWeight: 600, fontSize: 19 }}>
              {title}
            </Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
              {description}
            </Typography>
          </Box>
          {value === "Simple" && (
            <Typography variant="caption" color="text.secondary">
              推荐
            </Typography>
          )}
        </Button>
      ))}
      {error && <Alert severity="error">{error}</Alert>}
      <Button
        variant="contained"
        disabled={ui.saving}
        sx={{ minHeight: 44 }}
        onClick={async () => {
          try {
            await ui.chooseMode(mode);
          } catch (e) {
            setError(errorMessage(e));
          }
        }}
      >
        {ui.saving ? <CircularProgress size={20} /> : "开始使用"}
      </Button>
      <Typography
        variant="caption"
        color="text.secondary"
        sx={{ textAlign: "center" }}
      >
        切换界面会保留已有配置
      </Typography>
    </Stack>
  );
}

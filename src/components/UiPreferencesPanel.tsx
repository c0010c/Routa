import {
  Box,
  MenuItem,
  Paper,
  Select,
  Stack,
  Switch,
  Typography,
} from "@mui/material";
import { useInterface } from "./InterfaceProvider";
import { type InterfaceMode } from "../api/desktop";
import { SaveFeedback } from "./SaveFeedback";

export function UiPreferencesPanel() {
  const ui = useInterface();
  if (!ui?.state) return null;
  const p = ui.state.preferences;
  const feedback = ui.preferenceFeedback;
  const update = async (next: Partial<typeof p>) => {
    await ui.updatePreference(next, Object.keys(next)[0] as keyof typeof p);
  };
  return (
    <Stack spacing={2}>
      <Typography sx={{ fontWeight: 600 }}>界面</Typography>
      <Paper>
        <Stack direction="column" spacing={2} sx={{ p: 2, alignItems: "flex-start" }}>
          <Box sx={{ flex: 1 }}>
            <Typography>界面模式</Typography>
            <Typography variant="body2" color="text.secondary">
              切换后保留现有连接和配置
            </Typography>
          </Box>
          <Select
            aria-label="界面模式"
            size="small"
            value={p.mode ?? "Full"}
            disabled={feedback.pending || ui.saving}
            onChange={(e) =>
              void update({ mode: e.target.value as InterfaceMode })
            }
          >
            <MenuItem value="Simple">简洁版</MenuItem>
            <MenuItem value="Full">完整版</MenuItem>
          </Select>
          <SaveFeedback notice={feedback.notices.mode} />
        </Stack>
        <Stack
          direction="column"
          sx={{
            p: 2,
            borderTop: 1,
            borderColor: "divider",
            alignItems: "flex-start",
          }}
        >
          <Box sx={{ flex: 1 }}>
            <Typography>启动后自动连接</Typography>
            <Typography variant="body2" color="text.secondary">
              使用上次选择的节点和接管方式
            </Typography>
          </Box>
          <Switch
            checked={p.auto_connect}
            disabled={feedback.pending || ui.saving}
            onChange={(_, v) => void update({ auto_connect: v })}
            slotProps={{ input: { "aria-label": "启动后自动连接" } }}
          />
          <SaveFeedback notice={feedback.notices.auto_connect} />
        </Stack>
        <Stack
          direction="column"
          sx={{
            p: 2,
            borderTop: 1,
            borderColor: "divider",
            alignItems: "flex-start",
          }}
        >
          <Typography sx={{ flex: 1 }}>关闭窗口时</Typography>
          <Select
            size="small"
            aria-label="关闭窗口时"
            value={p.close_to_tray ? "tray" : "exit"}
            disabled={feedback.pending || ui.saving}
            onChange={(e) =>
              void update({ close_to_tray: e.target.value === "tray" })
            }
          >
            <MenuItem value="tray">最小化到托盘</MenuItem>
            <MenuItem value="exit">退出程序</MenuItem>
          </Select>
          <SaveFeedback notice={feedback.notices.close_to_tray} />
        </Stack>
      </Paper>
    </Stack>
  );
}

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
import { useModel } from "../api/client";
import { type InterfaceMode } from "../api/desktop";
import { SaveFeedback } from "./SaveFeedback";

export function UiPreferencesPanel() {
  const ui = useInterface();
  const listeners = useModel().data?.listeners ?? [];
  if (!ui?.state) return null;
  const p = ui.state.preferences;
  const feedback = ui.preferenceFeedback;
  const update = async (next: Partial<typeof p>) => {
    await ui.updatePreference(next, Object.keys(next)[0] as keyof typeof p);
  };
  const taskbar = p.taskbar_status;
  const supported = ui.state.taskbar_status_supported;
  const pinned = typeof taskbar.mode === "object" ? taskbar.mode.Fixed : null;
  const shown = pinned && listeners.some((l) => l.id === pinned) ? pinned : "rotate";
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
            <Typography>任务栏状态栏</Typography>
            <Typography variant="body2" color="text.secondary">
              {supported ? "在托盘旁显示流量、节点和延迟" : "仅支持 Windows 11"}
            </Typography>
          </Box>
          <Switch
            checked={supported && taskbar.enabled}
            disabled={!supported || feedback.pending || ui.saving}
            onChange={(_, v) =>
              void update({ taskbar_status: { ...taskbar, enabled: v } })
            }
            slotProps={{ input: { "aria-label": "任务栏状态栏" } }}
          />
          {supported && taskbar.enabled && (
            <Select
              size="small"
              aria-label="显示内容"
              value={shown}
              disabled={feedback.pending || ui.saving}
              onChange={(e) =>
                void update({
                  taskbar_status: {
                    ...taskbar,
                    mode: e.target.value === "rotate" ? "Rotate" : { Fixed: e.target.value },
                  },
                })
              }
            >
              <MenuItem value="rotate">轮播</MenuItem>
              {listeners.map((l) => (
                <MenuItem key={l.id} value={l.id}>
                  固定：{l.name}
                </MenuItem>
              ))}
            </Select>
          )}
          <SaveFeedback notice={feedback.notices.taskbar_status} />
        </Stack>
      </Paper>
    </Stack>
  );
}

import { MenuItem, Select, Stack, Switch } from "@mui/material";
import type { ReactNode } from "react";
import { useInterface } from "./InterfaceProvider";
import { useModel } from "../api/client";
import { type InterfaceMode } from "../api/desktop";
import { SaveFeedback } from "./SaveFeedback";
import { Row, Section } from "./SettingsSection";

/** Startup, window and taskbar preferences; `startupRows` are placed right after the interface mode. */
export function UiPreferencesPanel({ startupRows = [] }: { startupRows?: ReactNode[] }) {
  const ui = useInterface();
  // A disabled listener never rotates in, so pinning it would look ignored.
  const listeners = (useModel().data?.listeners ?? []).filter((l) => l.user_enabled);
  // Rows passed in by the page don't depend on the interface state, so they stay usable without it.
  if (!ui?.state) {
    return startupRows.length > 0 ? <Section title="启动与窗口">{startupRows}</Section> : null;
  }
  const p = ui.state.preferences;
  const feedback = ui.preferenceFeedback;
  const busy = feedback.pending || ui.saving;
  const update = async (next: Partial<typeof p>) => {
    await ui.updatePreference(next, Object.keys(next)[0] as keyof typeof p);
  };
  const taskbar = p.taskbar_status;
  const supported = ui.state.taskbar_status_supported;
  const pinned = typeof taskbar.mode === "object" ? taskbar.mode.Fixed : null;
  const shown = pinned && listeners.some((l) => l.id === pinned) ? pinned : "rotate";
  return (
    <>
      <Section title="启动与窗口">
        {[
          <Row
            key="mode"
            label="界面模式"
            description="切换后保留现有连接和配置"
            feedback={<SaveFeedback notice={feedback.notices.mode} />}
          >
            <Select
              aria-label="界面模式"
              size="small"
              value={p.mode ?? "Full"}
              disabled={busy}
              onChange={(e) => void update({ mode: e.target.value as InterfaceMode })}
            >
              <MenuItem value="Simple">简洁版</MenuItem>
              <MenuItem value="Full">完整版</MenuItem>
            </Select>
          </Row>,
          ...startupRows,
          <Row
            key="auto_connect"
            label="启动后自动连接"
            description="使用上次选择的节点和接管方式"
            feedback={<SaveFeedback notice={feedback.notices.auto_connect} />}
          >
            <Switch
              checked={p.auto_connect}
              disabled={busy}
              onChange={(_, v) => void update({ auto_connect: v })}
              slotProps={{ input: { "aria-label": "启动后自动连接" } }}
            />
          </Row>,
          <Row
            key="close_to_tray"
            label="关闭窗口时"
            feedback={<SaveFeedback notice={feedback.notices.close_to_tray} />}
          >
            <Select
              size="small"
              aria-label="关闭窗口时"
              value={p.close_to_tray ? "tray" : "exit"}
              disabled={busy}
              onChange={(e) => void update({ close_to_tray: e.target.value === "tray" })}
            >
              <MenuItem value="tray">最小化到托盘</MenuItem>
              <MenuItem value="exit">退出程序</MenuItem>
            </Select>
          </Row>,
        ]}
      </Section>
      <Section title="任务栏">
        {[
          <Row
            key="taskbar"
            label="任务栏状态栏"
            description={supported ? "在托盘旁显示流量、节点和延迟" : "仅支持 Windows 11"}
            feedback={<SaveFeedback notice={feedback.notices.taskbar_status} />}
            below={supported && taskbar.enabled && (
              <Stack direction="row" spacing={1} sx={{ flexWrap: "wrap", rowGap: 1 }}>
                <Select
                  size="small"
                  aria-label="流量显示"
                  value={taskbar.traffic}
                  disabled={busy}
                  onChange={(e) =>
                    void update({ taskbar_status: { ...taskbar, traffic: e.target.value as typeof taskbar.traffic } })
                  }
                >
                  <MenuItem value="Values">数值</MenuItem>
                  <MenuItem value="Chart">折线图</MenuItem>
                </Select>
                <Select
                  size="small"
                  aria-label="显示内容"
                  value={shown === "rotate" ? "rotate" : "fixed"}
                  disabled={busy}
                  onChange={(e) => {
                    const first = listeners[0]?.id;
                    const mode = e.target.value === "fixed" && first ? { Fixed: first } : "Rotate";
                    void update({ taskbar_status: { ...taskbar, mode } });
                  }}
                >
                  <MenuItem value="rotate">轮播</MenuItem>
                  <MenuItem value="fixed" disabled={listeners.length === 0}>
                    固定
                  </MenuItem>
                </Select>
                {shown !== "rotate" && (
                  <Select
                    size="small"
                    aria-label="固定的代理"
                    value={shown}
                    disabled={busy}
                    onChange={(e) =>
                      void update({ taskbar_status: { ...taskbar, mode: { Fixed: e.target.value } } })
                    }
                  >
                    {listeners.map((l) => (
                      <MenuItem key={l.id} value={l.id}>
                        {l.name}（{l.port}）
                      </MenuItem>
                    ))}
                  </Select>
                )}
              </Stack>
            )}
          >
            <Switch
              checked={supported && taskbar.enabled}
              disabled={!supported || busy}
              onChange={(_, v) => void update({ taskbar_status: { ...taskbar, enabled: v } })}
              slotProps={{ input: { "aria-label": "任务栏状态栏" } }}
            />
          </Row>,
        ]}
      </Section>
    </>
  );
}

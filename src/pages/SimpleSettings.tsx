import {
  Box,
  Button,
  Paper,
  Stack,
  Switch,
  Typography,
} from "@mui/material";
import { Link } from "react-router-dom";
import { BackHeading } from "../components/BackHeading";
import { UiPreferencesPanel } from "../components/UiPreferencesPanel";
import { useModel } from "../api/client";
import { settingsSaveMessage, useSettingsSave } from "../api/settings";
import { SaveFeedback, useSaveFeedback } from "../components/SaveFeedback";
import pkg from "../../package.json";
export function SimpleSettings() {
  const model = useModel();
  const save = useSettingsSave();
  const feedback = useSaveFeedback<"autostart">();
  return (
    <Stack spacing={3} sx={{ maxWidth: 720, mx: "auto" }}>
      <BackHeading title="设置" />
      <UiPreferencesPanel />
      <Paper>
        <Stack direction="row" sx={{ p: 2, alignItems: "center" }}>
          <Box sx={{ flex: 1 }}>
            <Typography>开机启动</Typography>
            <Typography variant="body2" color="text.secondary">
              登录 Windows 后启动 Routa
            </Typography>
          </Box>
          <Switch
            checked={model.data?.settings.autostart ?? false}
            disabled={!model.data || save.isPending || feedback.pending}
            slotProps={{ input: { "aria-label": "开机启动" } }}
            onChange={(_, v) => void feedback.run("autostart", async () => {
              try { await save.mutateAsync({ autostart: v }); }
              catch (error) { throw new Error(settingsSaveMessage(error)); }
            })}
          />
        </Stack>
        <Box sx={{ px: 2, pb: 1 }}><SaveFeedback notice={feedback.notices.autostart} /></Box>
      </Paper>
      <Paper
        sx={{
          p: 2,
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
        }}
      >
        <Typography>故障诊断</Typography>
        <Button component={Link} to="/diagnostics">
          打开
        </Button>
      </Paper>
      <Typography variant="caption" color="text.secondary">
        Routa v{pkg.version}
      </Typography>
    </Stack>
  );
}

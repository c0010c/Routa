import { Button, Stack, Switch, Typography } from "@mui/material";
import { Link } from "react-router-dom";
import { BackHeading } from "../components/BackHeading";
import { UpdatePanel } from "../components/UpdatePanel";
import { UiPreferencesPanel } from "../components/UiPreferencesPanel";
import { Row, Section } from "../components/SettingsSection";
import { PROJECT_LINKS, ProjectLinkRow } from "../components/ProjectLinkRow";
import { useModel } from "../api/client";
import { settingsSaveMessage, useSettingsSave } from "../api/settings";
import { SaveFeedback, useSaveFeedback } from "../components/SaveFeedback";
import pkg from "../../package.json";
export function SimpleSettings() {
  const model = useModel();
  const save = useSettingsSave();
  const feedback = useSaveFeedback<"autostart">();
  return (
    <Stack spacing={2.5} sx={{ maxWidth: 720, mx: "auto" }}>
      <BackHeading title="设置" />
      <UiPreferencesPanel
        startupRows={[
          <Row
            key="autostart"
            label="开机启动"
            description="登录 Windows 后启动 Routa"
            feedback={<SaveFeedback notice={feedback.notices.autostart} />}
          >
            <Switch
              checked={model.data?.settings.autostart ?? false}
              disabled={!model.data || save.isPending || feedback.pending}
              slotProps={{ input: { "aria-label": "开机启动" } }}
              onChange={(_, v) => void feedback.run("autostart", async () => {
                try { await save.mutateAsync({ autostart: v }); }
                catch (error) { throw new Error(settingsSaveMessage(error)); }
              })}
            />
          </Row>,
        ]}
      />
      <UpdatePanel />
      <Section title="支持">
        {[
          <Row key="diagnostics" label="故障诊断">
            <Button component={Link} to="/diagnostics">
              打开
            </Button>
          </Row>,
          ...PROJECT_LINKS.map((link) => <ProjectLinkRow key={link} link={link} compact />),
        ]}
      </Section>
      <Typography variant="caption" color="text.secondary">
        Routa v{pkg.version}
      </Typography>
    </Stack>
  );
}

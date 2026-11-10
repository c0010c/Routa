import mihomoVersion from "../../src-tauri/binaries/MIHOMO_VERSION?raw";
import { visibleListeners } from "../lib/hidden";
import ChevronRight from "@mui/icons-material/ChevronRight";
import {
  Alert,
  Box,
  Button,
  Stack,
  Switch,
  Tab,
  Tabs,
  TextField,
  Typography,
} from "@mui/material";
import { useState, type ReactNode } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  errorMessage,
  isValidationError,
  useModel,
  useOpenLogDir,
} from "../api/client";
import {
  useProcRouteCleanup,
  useProcRouteStatus,
  useProcRouteUninstall,
  useSetExitPrompt,
} from "../api/procroute";
import { desktopApi, useDesktopMutation, useDesktopState } from "../api/desktop";
import { settingsSaveMessage, useSettingsSave } from "../api/settings";
import type {
  Listener,
  Settings as SettingsModel,
} from "../bindings";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { ConfirmDeleteDialog } from "../components/ConfirmDeleteDialog";
import { SaveFeedback, useSaveFeedback } from "../components/SaveFeedback";
import { IntervalInput } from "../components/IntervalInput";
import { fieldErrors, type FieldErrors } from "../lib/validation";
import { applyOutcomeMessages } from "../lib/format";
import { UpdatePanel } from "../components/UpdatePanel";
import { UiPreferencesPanel } from "../components/UiPreferencesPanel";
import { Row, Section } from "../components/SettingsSection";
import { NodeMonitorSection } from "../components/NodeMonitorSection";
import { PROJECT_LINKS, ProjectLinkRow } from "../components/ProjectLinkRow";

const TABS = [
  { value: "general", label: "常规" },
  { value: "network", label: "网络与内核" },
  { value: "procroute", label: "进程分流" },
  { value: "about", label: "关于" },
] as const;
type TabValue = (typeof TABS)[number]["value"];

/** Keeps every panel mounted so drafts such as the control port survive switching tabs. */
function TabPanel({ value, current, children }: { value: TabValue; current: TabValue; children: ReactNode }) {
  // The `hidden` attribute sits on a plain Box: a flex Stack would override its display: none.
  return (
    <Box
      role="tabpanel"
      id={`settings-panel-${value}`}
      aria-labelledby={`settings-tab-${value}`}
      hidden={value !== current}
    >
      <Stack spacing={2.5}>{children}</Stack>
    </Box>
  );
}

function SettingsForm({
  initial,
  listeners,
}: {
  initial: SettingsModel;
  listeners: Listener[];
}) {
  const save = useSettingsSave();
  const openLogDir = useOpenLogDir();
  const feedback = useSaveFeedback<"autostart" | "interval" | "url" | "core">();
  const [port, setPort] = useState(String(initial.controller_port));
  const [url, setUrl] = useState(initial.delay_test_url);
  const [intervalRevision, setIntervalRevision] = useState(0);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });
  const [portLocal, setPortLocal] = useState<string>();
  const portError = portLocal ?? errs.port;
  const [params, setParams] = useSearchParams();
  const tab: TabValue = TABS.find((t) => t.value === params.get("tab"))?.value ?? "general";

  const dirty = port !== String(initial.controller_port);
  const pending = save.isPending || feedback.pending;

  const saveImmediate = (key: "autostart" | "interval" | "url", patch: Partial<SettingsModel>) =>
    feedback.run(key, async () => {
      try {
        await save.mutateAsync(patch);
      } catch (error) {
        if (key === "interval") setIntervalRevision((value) => value + 1);
        throw new Error(settingsSaveMessage(error));
      }
    });

  const saveUrl = () => {
    if (url === initial.delay_test_url || pending) return;
    void saveImmediate("url", { delay_test_url: url });
  };

  const submit = async () => {
    const p = Number(port);
    if (!Number.isInteger(p) || p < 1 || p > 65535) {
      setPortLocal("端口必须在 1–65535 之间");
      return;
    }
    setPortLocal(undefined);
    setErrs({ form: [] });
    await feedback.run("core", async () => {
      try {
        const outcome = await save.mutateAsync({ controller_port: p });
        const warnings = applyOutcomeMessages(outcome);
        if (warnings.length) return { kind: "warning", text: warnings.join("；") };
        return { kind: "success", text: "已保存并生效" };
      } catch (e) {
        if (isValidationError(e)) {
          const fields = fieldErrors(
            e.errors.filter((error) => !("CoreRejected" in error)),
          );
          if (e.errors.some((error) => "CoreRejected" in error)) {
            fields.form.push("设置无法使用，请检查后重新保存");
          }
          const conflict = e.errors.find((error) => "PortConflict" in error);
          if (conflict && "PortConflict" in conflict) {
            const { port: conflictPort } = conflict.PortConflict;
            const listener = listeners.find((l) => l.port === conflictPort);
            fields.port = listener
              ? `控制端口 ${conflictPort} 与 Listener「${listener.name}」冲突，请使用其他端口`
              : `控制端口 ${conflictPort} 与已有 Listener 冲突，请使用其他端口`;
          }
          setErrs(fields);
        }
        throw new Error(isValidationError(e) ? "设置未能保存，请检查后重试" : settingsSaveMessage(e));
      }
    });
  };

  return (
    <Stack spacing={1} sx={{ maxWidth: 760 }}>
      <Typography variant="h5">设置</Typography>
      <Tabs
        value={tab}
        onChange={(_, value: TabValue) => setParams({ tab: value }, { replace: true })}
        aria-label="设置分类"
        sx={{ borderBottom: 1, borderColor: "divider", mb: 1.5 }}
      >
        {TABS.map((t) => (
          <Tab
            key={t.value}
            value={t.value}
            label={t.label}
            id={`settings-tab-${t.value}`}
            aria-controls={`settings-panel-${t.value}`}
          />
        ))}
      </Tabs>
      <TabPanel value="general" current={tab}>
        <UiPreferencesPanel
          startupRows={[
            <Row
              key="autostart"
              label="开机自启"
              description="登录 Windows 后自动启动 Routa"
              feedback={<SaveFeedback notice={feedback.notices.autostart} />}
            >
              <Switch
                checked={initial.autostart}
                disabled={pending}
                onChange={(_, value) => void saveImmediate("autostart", { autostart: value })}
                slotProps={{ input: { "aria-label": "开机自启" } }}
              />
            </Row>,
          ]}
        />
      </TabPanel>
      <TabPanel value="network" current={tab}>
        <Section title="测速">
          {[
            <Row
              key="auto"
              label="自动测速"
              description="按间隔对所有出口测延迟"
              feedback={<SaveFeedback notice={feedback.notices.interval} />}
            >
              <IntervalInput
                key={`${initial.delay_test_interval_mins}:${intervalRevision}`}
                value={initial.delay_test_interval_mins}
                onChange={(value) => {
                  if (value !== initial.delay_test_interval_mins) void saveImmediate("interval", { delay_test_interval_mins: value });
                }}
                defaultMins={10}
                onValidityChange={() => feedback.clear("interval")}
                disabled={pending}
                commitOnBlur
              />
            </Row>,
            <Row
              key="url"
              label="测速 URL"
              feedback={<SaveFeedback notice={feedback.notices.url} onRetry={saveUrl} />}
            >
              <TextField
                size="small"
                value={url}
                disabled={pending}
                onChange={(e) => { setUrl(e.target.value); feedback.clear("url"); }}
                onBlur={saveUrl}
                onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); saveUrl(); } }}
                slotProps={{ htmlInput: { "aria-label": "测速 URL" } }}
                sx={{ width: 320, maxWidth: "100%" }}
              />
            </Row>,
          ]}
        </Section>
        <NodeMonitorSection />
        <Section title="内核" footer={
          <Stack
            role="group"
            aria-label="内核保存操作"
            spacing={0.5}
            sx={{ px: 2, py: 1.5, borderTop: 1, borderColor: "divider", bgcolor: "action.hover" }}
          >
            <Stack direction="row" spacing={1.5} sx={{ alignItems: "center", flexWrap: "wrap", rowGap: 1 }}>
              <Typography variant="body2" color="text.secondary" sx={{ flex: 1 }}>
                {dirty ? "有未保存的修改" : ""}
              </Typography>
              <Button variant="contained" disabled={pending || !dirty} onClick={() => void submit()}>保存并重启</Button>
            </Stack>
            {feedback.notices.core && <SaveFeedback notice={feedback.notices.core} />}
          </Stack>
        }>
          {[
            <Row key="path" label="内置 mihomo"><Typography color="text.secondary">{mihomoVersion.trim()} · 随 Routa 提供</Typography></Row>,
            <Row
              key="port"
              label="控制端口"
              description="修改后 mihomo 会重启"
              below={errs.form.length > 0 && <Alert severity="error">{errs.form.join("；")}</Alert>}
            >
              <TextField
                size="small"
                type="number"
                disabled={pending}
                value={port}
                onChange={(e) => {
                  setPort(e.target.value);
                  setPortLocal(undefined);
                  setErrs((x) => ({ ...x, port: undefined }));
                  feedback.clear("core");
                }}
                error={!!portError}
                helperText={portError}
                slotProps={{ htmlInput: { "aria-label": "控制端口" } }}
                sx={{ width: 140 }}
              />
            </Row>,
            <Row key="log" label="日志">
              <Button variant="outlined" onClick={() => openLogDir.mutate()}>
                打开日志目录
              </Button>
            </Row>,
          ]}
        </Section>
      </TabPanel>
      <TabPanel value="procroute" current={tab}>
        <ProcessRoutingSection />
      </TabPanel>
      <TabPanel value="about" current={tab}>
        <UpdatePanel />
        <Section title="支持">
          {[
            <Row key="diagnostics" label="故障诊断" description="查看内核、监听端口、系统代理和分流组件状态">
              <Button variant="outlined" component={Link} to="/diagnostics" endIcon={<ChevronRight />}>
                打开
              </Button>
            </Row>,
            ...PROJECT_LINKS.map((link) => <ProjectLinkRow key={link} link={link} />),
          ]}
        </Section>
      </TabPanel>
    </Stack>
  );
}

/** Process routing settings that are set once and rarely touched; the page itself keeps the switch and status. */
function ProcessRoutingSection() {
  const model = useModel();
  const setExitPrompt = useSetExitPrompt();
  const status = useProcRouteStatus();
  // Both report their own result (success or 「…失败：原因」) instead of the bare global error.
  const uninstall = useProcRouteUninstall({ errorFeedback: "local" });
  const cleanup = useProcRouteCleanup({ errorFeedback: "local" });
  const notify = useApplyResult();
  const withResult = (run: () => Promise<unknown>, done: string, failed: string) => async () => {
    try {
      await run();
      notify.showMessage(done, "success");
    } catch (e) {
      notify.showError(`${failed}：${errorMessage(e)}`);
      // Rethrown so the confirm dialog stays open for another try.
      throw e;
    }
  };
  const desktop = useDesktopState();
  const savePrefs = useDesktopMutation(desktopApi.preferences);
  const [confirm, setConfirm] = useState<"uninstall" | "cleanup" | null>(null);
  const component = status.data?.component;
  const busy = uninstall.isPending || cleanup.isPending;
  const exitPrompt = model.data?.process_routing.exit_prompt ?? true;
  return (
    <>
      <Section title="退出">
        {[
          <Row
            key="exit-prompt"
            label="退出时提示"
            description="退出 Routa 会让选了阻断的程序断网时，先询问是否停用进程分流"
          >
            <Switch
              checked={exitPrompt}
              disabled={!model.data || setExitPrompt.isPending}
              onChange={(_, value) => setExitPrompt.mutate(value)}
              slotProps={{ input: { "aria-label": "退出时提示" } }}
            />
          </Row>,
        ]}
      </Section>
      <Section title="AI 应用">
        {[
          <Row
            key="ai-reset-prompt"
            label="AI 应用断开连接前提示"
            description="开启、关闭代理或换节点会断开运行中 AI 应用的现有连接，操作前先询问"
          >
            <Switch
              checked={!desktop.data?.preferences.skip_ai_reset_prompt}
              disabled={!desktop.data || savePrefs.isPending}
              onChange={(_, value) =>
                desktop.data && savePrefs.mutate({ ...desktop.data.preferences, skip_ai_reset_prompt: !value })
              }
              slotProps={{ input: { "aria-label": "AI 应用断开连接前提示" } }}
            />
          </Row>,
        ]}
      </Section>
      <Section title="分流组件">
        {[
          <Row
            key="cleanup"
            label="清理残留阻断规则"
            description="删除系统中所有 Routa 留下的阻断规则，被阻断的程序会直接联网。可能需要管理员授权。"
          >
            <Button
              variant="outlined"
              aria-label="清理残留阻断规则"
              disabled={busy}
              onClick={() => setConfirm("cleanup")}
            >
              清理
            </Button>
          </Row>,
          <Row
            key="uninstall"
            label="卸载组件"
            description="停止并删除分流服务。需要管理员授权。"
          >
            <Button
              variant="outlined"
              color="error"
              aria-label="卸载组件"
              disabled={busy || component === "NotInstalled"}
              onClick={() => setConfirm("uninstall")}
            >
              卸载
            </Button>
          </Row>,
        ]}
      </Section>
      {confirm === "uninstall" && (
        <ConfirmDeleteDialog
          title="卸载组件"
          confirmLabel="卸载"
          text="将停止并删除分流服务，并清除它的所有阻断规则。需要管理员授权。"
          onConfirm={withResult(() => uninstall.mutateAsync(), "分流组件已卸载", "卸载失败")}
          onClose={() => setConfirm(null)}
        />
      )}
      {confirm === "cleanup" && (
        <ConfirmDeleteDialog
          title="清理残留阻断规则"
          confirmLabel="清理"
          text="将删除系统中所有 Routa 留下的阻断规则，被阻断的程序会直接联网。服务未运行时需要管理员授权。"
          onConfirm={withResult(() => cleanup.mutateAsync(), "已清理残留阻断规则", "清理失败")}
          onClose={() => setConfirm(null)}
        />
      )}
    </>
  );
}

export function Settings() {
  const model = useModel();
  if (!model.data) return <Typography>加载中…</Typography>;
  return (
    <SettingsForm
      initial={model.data.settings}
      listeners={visibleListeners(model.data)}
    />
  );
}

import { Box, Button, InputAdornment, MenuItem, Select, Stack, Switch, TextField, Typography } from "@mui/material";
import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { settingsSaveMessage, useSettingsSave } from "../api/settings";
import { unwrap, useModel, useStatus } from "../api/client";
import { commands, type MonitorStatus, type MonitorTarget, type NodeMonitorSettings } from "../bindings";
import { customMembersFrom, droppedMessage, failureText, intervalLabel, MONITOR_INTERVALS } from "../lib/monitor";
import { useApplyResult } from "./ApplyResultSnackbar";
import { MemberPicker } from "./MemberPicker";
import { SaveFeedback, useSaveFeedback } from "./SaveFeedback";
import { Row, Section } from "./SettingsSection";

type Key = "monitorEnabled" | "monitorScope" | "monitorMembers" | "monitorInterval" | "monitorThreshold" | "monitorLatency" | "monitorRecovery";

const EMPTY_STATUS: MonitorStatus = {
  last_run: null, next_run: null, probing: false, paused_reason: null, network_down: false, targets: [],
};
const PREVIEW_COUNT = 5;
const LATENCY_ERROR = "请输入 1–60000 的整数";

const hhmm = (secs: number) =>
  new Date(secs * 1000).toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });

type Tone = "idle" | "ok" | "warn" | "down";

function targetState(t: MonitorTarget, threshold: number): { tone: Tone; text: string } {
  if (t.not_applied) return { tone: "idle", text: "出口未生效" };
  if (!t.probed) return { tone: "idle", text: "等待探测" };
  if (t.down) return { tone: "down", text: t.last_failure ? `不可用 · ${failureText(t.last_failure)}` : "不可用" };
  if (t.fails > 0) return { tone: "warn", text: `失败 ${t.fails}/${threshold}` };
  return { tone: "ok", text: `${t.last_ms ?? 0} ms` };
}

const TONE_COLOR: Record<Tone, string> = {
  idle: "text.disabled", ok: "success.main", warn: "warning.main", down: "error.main",
};
const TEXT_COLOR: Record<Tone, string> = {
  idle: "text.secondary", ok: "text.secondary", warn: "warning.main", down: "error.main",
};

function secondaryText(t: MonitorTarget, scope: NodeMonitorSettings["scope"]): string {
  if (scope === "Custom") return t.proxy.split("/")[0];
  const users = t.used_by.join("，");
  return t.node === null ? `自动选择 · ${users}` : users;
}

export function NodeMonitorSection() {
  const model = useModel().data;
  const status = useStatus().data?.monitor ?? EMPTY_STATUS;
  const save = useSettingsSave();
  const qc = useQueryClient();
  const apply = useApplyResult();
  const feedback = useSaveFeedback<Key>();
  const [draft, setDraft] = useState<string | null>(null);
  const [latencyError, setLatencyError] = useState<string>();
  const [showAll, setShowAll] = useState(false);
  const [probeRequested, setProbeRequested] = useState(false);
  if (!model) return null;

  const nm = model.settings.node_monitor;
  const pending = save.isPending || feedback.pending;

  const saveMonitor = (key: Key, patch: Partial<NodeMonitorSettings>) =>
    feedback.run(key, async () => {
      try {
        await save.mutateAsync({ node_monitor: { ...nm, ...patch } });
      } catch (error) {
        throw new Error(settingsSaveMessage(error));
      }
      if (key === "monitorLatency") setDraft(null);
    });

  const changeScope = (scope: NodeMonitorSettings["scope"]) => {
    if (scope === nm.scope) return;
    if (scope === "Custom" && nm.members.length === 0) {
      const { members, dropped } = customMembersFrom(status);
      if (dropped.length) apply.showMessage(droppedMessage(dropped), "info");
      void saveMonitor("monitorScope", { scope, members });
    } else {
      void saveMonitor("monitorScope", { scope });
    }
  };

  const commitLatency = () => {
    if (draft === null || pending) return;
    const text = draft.trim();
    const value = text === "" ? null : /^\d+$/.test(text) ? Number(text) : NaN;
    if (value !== null && !(value >= 1 && value <= 60000)) return setLatencyError(LATENCY_ERROR);
    setLatencyError(undefined);
    if (value === nm.latency_limit_ms) return setDraft(null);
    void saveMonitor("monitorLatency", { latency_limit_ms: value });
  };

  const probeNow = async () => {
    setProbeRequested(true);
    try {
      unwrap(await commands.monitorProbeNow());
    } catch {
      apply.showMessage("探测未能启动，请重试", "error");
    } finally {
      setProbeRequested(false);
      void qc.invalidateQueries({ queryKey: ["status"] });
    }
  };

  const paused = status.paused_reason === "CoreNotRunning";
  const targets = status.targets;
  const downCount = targets.filter((t) => t.down).length;
  // Unprobed, not-yet-applied and currently failing (below threshold) targets are not "normal".
  const okCount = targets.filter((t) => t.probed && !t.down && t.fails === 0 && !t.not_applied).length;
  const listed = showAll ? targets : targets.slice(0, PREVIEW_COUNT);
  const probing = status.probing || probeRequested;

  const notice = (key: Key) => <SaveFeedback notice={feedback.notices[key]} />;
  const rows = [
    <Row key="enabled" label="节点监测" description="定时探测节点，不可用时发送系统通知" feedback={notice("monitorEnabled")}>
      <Switch
        checked={nm.enabled}
        disabled={pending}
        onChange={(_, v) => void saveMonitor("monitorEnabled", { enabled: v })}
        slotProps={{ input: { "aria-label": "节点监测" } }}
      />
    </Row>,
    nm.enabled && (
      <Row
        key="scope"
        label="监测范围"
        description={nm.scope === "InUse"
          ? `代理和进程分流当前使用的节点，随配置自动变化 · ${targets.length} 个`
          : `${targets.length} 个节点`}
        feedback={notice("monitorScope")}
        below={nm.scope === "Custom" && (
          <Box>
            <MemberPicker
              label="选择节点或整组"
              value={nm.members}
              onChange={(members) => void saveMonitor("monitorMembers", { members })}
            />
            {feedback.notices.monitorMembers && <SaveFeedback notice={feedback.notices.monitorMembers} />}
          </Box>
        )}
      >
        <Select
          size="small"
          aria-label="监测范围"
          value={nm.scope}
          disabled={pending}
          sx={{ width: 180 }}
          onChange={(e) => changeScope(e.target.value as NodeMonitorSettings["scope"])}
        >
          <MenuItem value="InUse">正在使用的节点</MenuItem>
          <MenuItem value="Custom">指定节点</MenuItem>
        </Select>
      </Row>
    ),
    nm.enabled && (
      <Row key="interval" label="探测间隔" feedback={notice("monitorInterval")}>
        <Select
          size="small"
          aria-label="探测间隔"
          value={nm.interval_secs}
          disabled={pending}
          sx={{ width: 180 }}
          onChange={(e) => void saveMonitor("monitorInterval", { interval_secs: Number(e.target.value) })}
        >
          {MONITOR_INTERVALS.map((v) => <MenuItem key={v} value={v}>{intervalLabel(v)}</MenuItem>)}
        </Select>
      </Row>
    ),
    nm.enabled && (
      <Row key="threshold" label="判定为不可用" description="连续失败达到次数才通知，避免偶发抖动误报" feedback={notice("monitorThreshold")}>
        <Select
          size="small"
          aria-label="判定为不可用"
          value={nm.fail_threshold}
          disabled={pending}
          sx={{ width: 180 }}
          onChange={(e) => void saveMonitor("monitorThreshold", { fail_threshold: Number(e.target.value) })}
        >
          {[1, 2, 3, 5].map((v) => (
            <MenuItem key={v} value={v}>{v === 1 ? "失败 1 次即通知" : `连续失败 ${v} 次`}</MenuItem>
          ))}
        </Select>
      </Row>
    ),
    nm.enabled && (
      <Row key="latency" label="延迟上限" description="超过上限也算异常；留空只检查能否连通" feedback={notice("monitorLatency")}>
        <TextField
          size="small"
          type="number"
          placeholder="不限"
          value={draft ?? (nm.latency_limit_ms === null ? "" : String(nm.latency_limit_ms))}
          disabled={pending}
          onChange={(e) => { setDraft(e.target.value); setLatencyError(undefined); feedback.clear("monitorLatency"); }}
          onBlur={commitLatency}
          onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); commitLatency(); } }}
          error={!!latencyError}
          helperText={latencyError}
          slotProps={{
            input: { endAdornment: <InputAdornment position="end">ms</InputAdornment> },
            htmlInput: { "aria-label": "延迟上限", min: 1 },
          }}
          sx={{ width: 180 }}
        />
      </Row>
    ),
    nm.enabled && (
      <Row key="recovery" label="恢复时通知" description="不可用的节点重新连通后再通知一次" feedback={notice("monitorRecovery")}>
        <Switch
          checked={nm.notify_recovery}
          disabled={pending}
          onChange={(_, v) => void saveMonitor("monitorRecovery", { notify_recovery: v })}
          slotProps={{ input: { "aria-label": "恢复时通知" } }}
        />
      </Row>
    ),
  ];

  const footer = nm.enabled && (
    <Stack spacing={1} sx={{ px: 2, py: 1.5, borderTop: 1, borderColor: "divider", bgcolor: "action.hover" }}>
      <Stack direction="row" spacing={1.5} sx={{ alignItems: "center" }}>
        <Typography variant="body2" color="text.secondary" sx={{ flex: 1 }}>
          {paused ? "内核未运行，监测已暂停" : status.last_run === null ? "尚未探测" : (
            <>
              上次探测 {hhmm(status.last_run)}
              {status.next_run !== null && ` · 下次 ${hhmm(status.next_run)}`}
              {` · ${okCount} 正常`}
              {downCount > 0 && <Box component="span" sx={{ color: "error.main" }}> · {downCount} 不可用</Box>}
            </>
          )}
        </Typography>
        <Button
          variant="outlined"
          size="small"
          disabled={paused || probing || targets.length === 0}
          onClick={() => void probeNow()}
        >
          {probing ? "探测中…" : "立即探测"}
        </Button>
      </Stack>
      {!paused && status.network_down && (
        <Typography variant="body2" color="error">网络可能不可用，节点状态暂不更新</Typography>
      )}
      {targets.length > 0 && (
        <Box component="ul" aria-label="监测中的节点" sx={{ m: 0, p: 0, listStyle: "none" }}>
          {listed.map((t) => {
            const state = targetState(t, nm.fail_threshold);
            return (
              <Stack component="li" key={t.proxy} direction="row" spacing={1} sx={{ alignItems: "center", py: 0.25 }}>
                <Box sx={{ width: 6, height: 6, borderRadius: "50%", flexShrink: 0, bgcolor: TONE_COLOR[state.tone] }} />
                <Typography variant="body2" noWrap sx={{ minWidth: 0 }}>{t.label}</Typography>
                <Typography variant="caption" color="text.secondary" noWrap sx={{ flex: 1 }}>
                  {secondaryText(t, nm.scope)}
                </Typography>
                <Typography variant="caption" sx={{ color: TEXT_COLOR[state.tone] }}>{state.text}</Typography>
              </Stack>
            );
          })}
          {targets.length > PREVIEW_COUNT && (
            <Button size="small" onClick={() => setShowAll((x) => !x)} sx={{ mt: 0.5, px: 0.5 }}>
              {showAll ? "收起" : `查看全部 ${targets.length} 个`}
            </Button>
          )}
        </Box>
      )}
    </Stack>
  );

  return <Section title="节点监测" footer={footer || undefined}>{rows}</Section>;
}

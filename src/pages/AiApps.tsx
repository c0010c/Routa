import { useState, type ReactNode } from "react";
import { Box, Stack, Switch, Typography } from "@mui/material";
import { alpha } from "@mui/material/styles";
import { useQueryClient } from "@tanstack/react-query";
import type { AiAppConflict, AiAppView, NodeRef } from "../bindings";
import { errorMessage, isValidationError, useModel, validationText } from "../api/client";
import { desktopApi, useDesktopMutation, useDesktopState } from "../api/desktop";
import { useSaveProcessRoutingSettings } from "../api/procroute";
import { useAiAppConflicts, useAiApps, useDisableAiApp, useEnableAiApp, useLaunchAiApp } from "../api/aiApps";
import { AiAppCard } from "../components/AiAppCard";
import { ConflictDialog, ResetDialog } from "../components/AiAppDialogs";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { NodePickerDialog } from "../components/NodePickerDialog";

/**
 * Runs the confirmations in order (pick node -> conflicts -> disconnect warning -> backend call);
 * cancelling any step does nothing. Backend errors are shown through the shared error feedback.
 */
function useAiAppFlow() {
  const qc = useQueryClient();
  const apply = useApplyResult();
  const prefs = useDesktopState().data?.preferences;
  const savePrefs = useDesktopMutation(desktopApi.preferences);
  const enable = useEnableAiApp();
  const disable = useDisableAiApp();
  const launchApp = useLaunchAiApp();
  const conflicts = useAiAppConflicts();
  const changeRuleNode = useDesktopMutation((a: [string, NodeRef]) => desktopApi.changeNode(a[0], a[1]));
  // Nodes chosen for apps that have no rule yet; used when the app is first enabled.
  const [picked, setPicked] = useState<Record<string, NodeRef>>({});
  const [dialog, setDialog] = useState<ReactNode>(null);

  const ask = <T,>(render: (done: (v: T) => void) => ReactNode) =>
    new Promise<T>((resolve) =>
      setDialog(
        render((v) => {
          setDialog(null);
          resolve(v);
        }),
      ),
    );
  const nodeOf = (app: AiAppView) => app.node ?? picked[app.id] ?? null;
  const fail = (e: unknown) => apply.showError(isValidationError(e) ? validationText(e.errors) : errorMessage(e));
  /** Runs a flow; a thrown backend error is reported and ends the flow. */
  const guarded = async (run: () => Promise<void>) => {
    try {
      await run();
    } catch (e) {
      fail(e);
    } finally {
      void qc.invalidateQueries({ queryKey: ["aiApps"] });
    }
  };

  const pickNode = (app: AiAppView) =>
    ask<NodeRef | null>((done) => (
      <NodePickerDialog title={`选择节点 · ${app.name}`} value={null} onClose={() => done(null)} onPick={done} />
    ));
  const confirmReset = async (app: AiAppView) => {
    if (prefs?.skip_ai_reset_prompt) return true;
    const answer = await ask<{ dontAsk: boolean } | null>((done) => (
      <ResetDialog appName={app.name} onCancel={() => done(null)} onConfirm={(dontAsk) => done({ dontAsk })} />
    ));
    if (!answer) return false;
    if (answer.dontAsk && prefs) {
      try {
        await savePrefs.mutateAsync({ ...prefs, skip_ai_reset_prompt: true });
      } catch (e) {
        fail(e);
      }
    }
    return true;
  };
  const confirmConflicts = (found: AiAppConflict[]) =>
    ask<boolean>((done) => <ConflictDialog conflicts={found} onCancel={() => done(false)} onConfirm={() => done(true)} />);

  /** True once the rule is enabled; false when the user cancelled a step. */
  const enableFlow = async (app: AiAppView) => {
    const node = nodeOf(app) ?? (await pickNode(app));
    if (!node) return false;
    setPicked((p) => ({ ...p, [app.id]: node }));
    const found = await conflicts.mutateAsync(app.id);
    if (found.length > 0 && !(await confirmConflicts(found))) return false;
    if (app.running && !(await confirmReset(app))) return false;
    await enable.mutateAsync(app.id, node);
    return true;
  };

  return {
    nodeOf,
    dialogs: dialog,
    toggle: (app: AiAppView, on: boolean) =>
      guarded(async () => {
        if (on) {
          await enableFlow(app);
        } else if (!app.running || (await confirmReset(app))) {
          await disable.mutateAsync(app.id);
        }
      }),
    launch: (app: AiAppView) =>
      guarded(async () => {
        if (!app.enabled && !(await enableFlow(app))) return;
        await launchApp.mutateAsync(app.id);
      }),
    changeNode: async (app: AiAppView, node: NodeRef) => {
      const ruleId = app.rule_id;
      if (!ruleId) return setPicked((p) => ({ ...p, [app.id]: node }));
      await guarded(async () => {
        if (app.enabled && app.running && !(await confirmReset(app))) return;
        await changeRuleNode.mutateAsync([ruleId, node]);
      });
    },
  };
}

export function AiAppsPage() {
  const model = useModel().data;
  const apps = useAiApps().data;
  const settings = useSaveProcessRoutingSettings();
  const flow = useAiAppFlow();
  if (!model) return null;
  const on = model.process_routing.enabled;

  return (
    <Box>
      <Stack direction="row" sx={{ alignItems: "center", justifyContent: "space-between", mb: 2 }}>
        <Typography variant="h5">AI 应用</Typography>
        <Switch
          checked={on}
          onChange={(e) => settings.mutate(e.target.checked)}
          slotProps={{ input: { "aria-label": "进程分流总开关" } }}
          sx={on ? undefined : { borderRadius: 4, boxShadow: (t) => `0 0 0 3px ${alpha(t.palette.primary.main, 0.25)}` }}
        />
      </Stack>
      <Box sx={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(180px, 1fr))", gap: 2 }}>
        {(apps ?? []).map((app) => (
          <AiAppCard
            key={app.id}
            app={{ ...app, node: flow.nodeOf(app) }}
            model={model}
            paused={!on}
            onToggle={(v) => void flow.toggle(app, v)}
            onLaunch={() => void flow.launch(app)}
            onPickNode={(n) => void flow.changeNode(app, n)}
          />
        ))}
      </Box>
      {flow.dialogs}
    </Box>
  );
}

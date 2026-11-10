import { useState } from "react";
import { Box, Stack, Switch, Typography } from "@mui/material";
import { alpha } from "@mui/material/styles";
import { useQueryClient } from "@tanstack/react-query";
import type { AiAppView, NodeRef } from "../bindings";
import { useModel } from "../api/client";
import { desktopApi, useDesktopMutation } from "../api/desktop";
import { useSaveProcessRoutingSettings } from "../api/procroute";
import { useAiApps, useDisableAiApp, useEnableAiApp, useLaunchAiApp } from "../api/aiApps";
import { AiAppCard } from "../components/AiAppCard";
import { NodePickerDialog } from "../components/NodePickerDialog";

export function AiAppsPage() {
  const qc = useQueryClient();
  const model = useModel().data;
  const apps = useAiApps().data;
  const settings = useSaveProcessRoutingSettings();
  const enable = useEnableAiApp();
  const disable = useDisableAiApp();
  const launch = useLaunchAiApp();
  const changeNode = useDesktopMutation((a: [string, NodeRef]) => desktopApi.changeNode(a[0], a[1]));
  // Nodes chosen for apps that have no rule yet; used when the app is first enabled.
  const [picked, setPicked] = useState<Record<string, NodeRef>>({});
  // App waiting for a node before it can be launched.
  const [launching, setLaunching] = useState<AiAppView | null>(null);
  if (!model) return null;
  const on = model.process_routing.enabled;
  const refresh = () => void qc.invalidateQueries({ queryKey: ["aiApps"] });

  // The handlers below are deliberately small: Task 8 swaps them for the dialog flow.
  const onToggle = async (app: AiAppView, next: boolean) => {
    try {
      if (!next) await disable.mutateAsync(app.id);
      else {
        const node = app.node ?? picked[app.id];
        if (node) await enable.mutateAsync(app.id, node);
      }
    } catch {
      // reported by the shared error feedback
    }
    refresh();
  };
  const launchNow = async (app: AiAppView, node?: NodeRef) => {
    try {
      if (!app.enabled && node) await enable.mutateAsync(app.id, node);
      await launch.mutateAsync(app.id);
    } catch {
      // reported by the shared error feedback
    }
    refresh();
  };
  const onLaunch = async (app: AiAppView) => {
    const node = app.node ?? picked[app.id];
    if (!app.enabled && !node) return setLaunching(app);
    await launchNow(app, node);
  };
  const onPickNode = async (app: AiAppView, node: NodeRef) => {
    if (!app.rule_id) return setPicked((p) => ({ ...p, [app.id]: node }));
    try {
      await changeNode.mutateAsync([app.rule_id, node]);
    } catch {
      // reported by the shared error feedback
    }
    refresh();
  };

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
            app={{ ...app, node: app.node ?? picked[app.id] ?? null }}
            model={model}
            paused={!on}
            onToggle={(v) => void onToggle(app, v)}
            onLaunch={() => void onLaunch(app)}
            onPickNode={(n) => void onPickNode(app, n)}
          />
        ))}
      </Box>
      {launching && (
        <NodePickerDialog
          title={`选择节点 · ${launching.name}`}
          value={null}
          onClose={() => setLaunching(null)}
          onPick={(node) => {
            const app = launching;
            setLaunching(null);
            setPicked((p) => ({ ...p, [app.id]: node }));
            void launchNow(app, node);
          }}
        />
      )}
    </Box>
  );
}

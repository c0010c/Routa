import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  NodeRef,
  DesktopState,
  UiPreferences,
  CaptureMode,
  ConnectionInfo,
} from "../bindings";
export type {
  DesktopState,
  UiPreferences,
  InterfaceMode,
  CaptureMode,
} from "../bindings";

export const desktopApi = {
  state: () => invoke<DesktopState>("desktop_state"),
  preferences: (input: UiPreferences) =>
    invoke<UiPreferences>("save_ui_preferences", { input }),
  bind: (id: string | null) =>
    invoke<UiPreferences>("bind_default_listener", { id }),
  select: (node: NodeRef) => invoke<void>("select_simple_node", { node }),
  connect: (mode: CaptureMode | null) =>
    invoke<DesktopState>("set_desktop_connection", { mode }),
  systemListener: (id: string) =>
    invoke<DesktopState>("set_system_proxy_listener", { id }),
  app: (path: string, selected: boolean) =>
    invoke<void>("save_simple_app", { path, selected }),
  port: (port: number) => invoke<void>("set_simple_port", { port }),
  changeListener: (id: string, listener: string) =>
    invoke<void>("change_process_listener", { id, listener }),
  connections: () => invoke<ConnectionInfo[]>("get_connection_details"),
  logs: () => invoke<string>("read_core_log"),
};
export function useDesktopState() {
  return useQuery({
    queryKey: ["desktop"],
    queryFn: desktopApi.state,
    refetchInterval: 2000,
  });
}
export function useDesktopMutation<A, T>(run: (arg: A) => Promise<T>) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSettled: async () => {
      await Promise.all(
        ["desktop", "model", "status", "procroute"].map((key) =>
          qc.invalidateQueries({ queryKey: [key] }),
        ),
      );
    },
  });
}

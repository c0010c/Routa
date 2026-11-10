import { useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type DelayResult } from "../bindings";
import { make } from "./client";

export function useAiApps() {
  return useQuery({
    queryKey: ["aiApps"],
    queryFn: () => commands.aiAppsList(),
    refetchInterval: 3000,
  });
}

// make() refreshes model/status/procroute on success; the card list refreshes by its own poll.
export const useEnableAiApp = make(() => commands.enableAiApp);
export const useDisableAiApp = make(() => commands.disableAiApp);
export const useLaunchAiApp = make(() => commands.launchAiApp);
export const useAiAppConflicts = make(() => commands.aiAppConflicts);

/** Delay of an AI app's exit path, measured on demand and cached for a minute. */
export function useAiAppDelay(appId: string, proxyName: string | undefined) {
  const qc = useQueryClient();
  const key = ["aiDelay", appId, proxyName];
  const q = useQuery({
    queryKey: key,
    enabled: !!proxyName,
    staleTime: 60_000,
    retry: false,
    queryFn: async (): Promise<DelayResult> => {
      const r = await commands.testAiAppDelay(appId, proxyName!);
      if (r.status === "error") throw new Error(r.error);
      return r.data;
    },
  });
  return {
    result: q.data,
    pending: q.isFetching,
    measure: () => void qc.invalidateQueries({ queryKey: key }),
  };
}

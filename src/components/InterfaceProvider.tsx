import { createContext, useContext, type ReactNode } from "react";
import {
  desktopApi,
  useDesktopMutation,
  useDesktopState,
  type DesktopState,
  type InterfaceMode,
  type UiPreferences,
} from "../api/desktop";
import { useSaveFeedback } from "./SaveFeedback";

interface InterfaceContextValue {
  state: DesktopState | undefined;
  loading: boolean;
  error: unknown;
  saving: boolean;
  chooseMode: (mode: InterfaceMode) => Promise<void>;
  retry: () => void;
  preferenceFeedback: ReturnType<typeof useSaveFeedback<keyof UiPreferences>>;
  updatePreference: (next: Partial<UiPreferences>, key: keyof UiPreferences) => Promise<void>;
}
const Context = createContext<InterfaceContextValue | null>(null);
export function useInterface() {
  return useContext(Context);
}
export function InterfaceProvider({ children }: { children: ReactNode }) {
  const query = useDesktopState();
  const save = useDesktopMutation(desktopApi.preferences);
  // The provider survives a mode switch, so its local success feedback does too.
  const preferenceFeedback = useSaveFeedback<keyof UiPreferences>();
  return (
    <Context.Provider
      value={{
        state: query.data,
        loading: query.isLoading,
        error: query.error,
        saving: save.isPending,
        preferenceFeedback,
        updatePreference: async (next, key) => {
          if (!query.data) return;
          await preferenceFeedback.run(key, async () => {
            try {
              await save.mutateAsync({ ...query.data.preferences, ...next });
            } catch {
              throw new Error("未能保存，请重试");
            }
          });
        },
        chooseMode: async (mode) => {
          if (!query.data) throw new Error("界面设置尚未加载");
          await save.mutateAsync({ ...query.data.preferences, mode });
        },
        retry: () => {
          void query.refetch();
        },
      }}
    >
      {children}
    </Context.Provider>
  );
}

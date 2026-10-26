import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useSyncExternalStore } from "react";
import {
  commands,
  type ApplyOutcome,
  type Result,
  type ValidationError,
} from "../bindings";
import { fieldErrors } from "../lib/validation";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { getTraffic, subscribeTraffic } from "./trafficStore";

export interface ApiValidationError {
  kind: "validation";
  errors: ValidationError[];
}

export function isValidationError(e: unknown): e is ApiValidationError {
  return typeof e === "object" && e !== null && (e as ApiValidationError).kind === "validation";
}

/** Unwraps a bindings Result: validation failures throw `ApiValidationError`. */
export function unwrap<T>(r: Result<T, ValidationError[] | string | null>): T {
  if (r.status === "ok") return r.data;
  if (Array.isArray(r.error)) {
    throw { kind: "validation", errors: r.error } satisfies ApiValidationError;
  }
  throw new Error(typeof r.error === "string" ? r.error : "操作失败");
}

export function useModel() {
  return useQuery({ queryKey: ["model"], queryFn: () => commands.getModel() });
}

export function useStatus() {
  return useQuery({ queryKey: ["status"], queryFn: () => commands.getStatus() });
}

export function useTraffic() {
  return useSyncExternalStore(subscribeTraffic, getTraffic);
}

function useApiMutation<A extends unknown[], T>(
  run: (...a: A) => Promise<Result<T, ValidationError[] | string | null>>,
) {
  const qc = useQueryClient();
  const apply = useApplyResult();
  return useMutation({
    mutationFn: async (a: A) => unwrap(await run(...a)),
    onError: (e) => {
      // Validation errors are shown in-form by the caller; everything else must be visible.
      if (!isValidationError(e)) apply.showError(errorMessage(e));
    },
    onSuccess: (data) => {
      if (typeof data === "object" && data !== null && isApplyOutcome(data)) apply.show(data);
      void qc.invalidateQueries({ queryKey: ["model"] });
      void qc.invalidateQueries({ queryKey: ["status"] });
    },
  });
}

function errorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  const t = String(e ?? "");
  return t === "" ? "操作失败" : t;
}

function validationText(errors: ValidationError[]): string {
  const f = fieldErrors(errors);
  return [...(f.name ? [f.name] : []), ...(f.port ? [f.port] : []), ...f.form].join("\n");
}

function isApplyOutcome(d: object): d is ApplyOutcome {
  return "Reloaded" in d || "FailedKeptPrevious" in d || "FailedClosed" in d || "FailedCoreStopped" in d || "NotRunning" in d;
}

type Fn<A extends unknown[], T> = (...a: A) => Promise<Result<T, ValidationError[] | string | null>>;

// Each hook's mutate takes a single argument (or an args tuple for multi-arg commands).
function make<A extends unknown[], T>(get: () => Fn<A, T>) {
  return () => {
    const m = useApiMutation<A, T>((...a) => get()(...a));
    const apply = useApplyResult();
    return {
      ...m,
      // Fire-and-forget callers have no form to put validation errors in: show them as a fallback.
      // mutateAsync callers handle validation errors themselves.
      mutate: (...a: A) =>
        m.mutate(a, {
          onError: (e) => {
            if (isValidationError(e)) apply.showError(validationText(e.errors));
          },
        }),
      mutateAsync: (...a: A) => m.mutateAsync(a),
    };
  };
}

export const useSaveListener = make(() => commands.saveListener);
export const useDeleteListener = make(() => commands.deleteListener);
export const useSaveExit = make(() => commands.saveExit);
export const useDeleteExit = make(() => commands.deleteExit);
export const useSaveSubscription = make(() => commands.saveSubscription);
export const useDeleteSubscription = make(() => commands.deleteSubscription);
export const useRefreshSubscription = make(() => commands.refreshSubscription);
export const useSelectGroupMember = make(() => commands.selectGroupMember);
export const useSaveSettings = make(() => commands.saveSettings);
export const useTestDelays = make(() => commands.testDelays);
export const useOpenLogDir = make(() => commands.openLogDir);
export const useResetTraffic = make(() => commands.resetTraffic);

export function useCoreControl() {
  const start = make(() => commands.coreStart)();
  const stop = make(() => commands.coreStop)();
  const restart = make(() => commands.coreRestart)();
  return {
    start,
    stop,
    restart,
    isPending: start.isPending || stop.isPending || restart.isPending,
  };
}

import { Alert, Button, Snackbar } from "@mui/material";
import { useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQueryClient } from "@tanstack/react-query";
import type { ApplyOutcome, NodeRef, Subscription, SubscriptionPreview, ValidationError } from "../bindings";
import { isValidationError, validationText, errorMessage } from "./client";

export type SubscriptionNotice = (text: string, severity?: "success" | "error" | "info", action?: { label: string; run: () => void }) => void;

export function useSubscriptionFeedback() {
  const [notice, setNotice] = useState<{ text: string; severity: "success" | "error" | "info"; action?: { label: string; run: () => void }; key: number } | null>(null);
  const notify: SubscriptionNotice = useCallback((text, severity = "success", action) => setNotice({ text, severity, action, key: Date.now() }), []);
  const element = <Snackbar key={notice?.key} open={!!notice} anchorOrigin={{ vertical: "bottom", horizontal: "right" }} autoHideDuration={notice?.severity === "error" ? null : 6500} onClose={(_, reason) => reason !== "clickaway" && setNotice(null)}>
    <Alert severity={notice?.severity} variant="outlined" onClose={() => setNotice(null)} sx={{ bgcolor: "background.paper", color: "text.primary", boxShadow: 4, maxWidth: 580, alignItems: "center", "& .MuiAlert-message": { overflowWrap: "anywhere" } }} action={<>
      {notice?.action && <Button size="small" onClick={() => { const action = notice.action; setNotice(null); action?.run(); }}>{notice.action.label}</Button>}
      <Button size="small" color="inherit" aria-label="关闭结果提示" sx={{ minWidth: 24 }} onClick={() => setNotice(null)}>×</Button>
    </>}>{notice?.text}</Alert>
  </Snackbar>;
  return { notify, element };
}

async function command<T>(name: string, args: Record<string, unknown>): Promise<T> {
  try { return await invoke<T>(name, args); }
  catch (e) { if (Array.isArray(e)) throw { kind: "validation", errors: e as ValidationError[] }; throw e; }
}
export function subscriptionError(error: unknown) { return isValidationError(error) ? validationText(error.errors) : errorMessage(error); }

export function useSubscriptionCommands() {
  const qc = useQueryClient();
  const invalidate = useCallback(async () => {
    await Promise.all(["model", "status", "procroute", "desktop", "aiApps"].map(key => qc.invalidateQueries({ queryKey: [key] })));
  }, [qc]);
  return {
    invalidate,
    preview: async (s: Subscription) => {
      try { return await command<SubscriptionPreview>("preview_subscription", { s }); }
      finally { await invalidate(); }
    },
    commit: async (preview: SubscriptionPreview, replacement: NodeRef | null) => {
      try { return await command<ApplyOutcome>("commit_subscription_preview", { preview, replacement }); }
      finally { await invalidate(); }
    },
    remove: async (id: string, replacement: NodeRef | null) => {
      try { return await command<{ token: string; outcome: ApplyOutcome }>("delete_subscription_replacing", { id, replacement }); }
      finally { await invalidate(); }
    },
    restore: async (token: string) => {
      try { return await command<ApplyOutcome>("restore_subscription", { token }); }
      finally { await invalidate(); }
    },
    retry: async (id: string) => {
      try { return await command<ApplyOutcome>("retry_subscription_apply", { id }); }
      finally { await invalidate(); }
    },
  };
}

import { Alert, Snackbar } from "@mui/material";
import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";
import type { ApplyOutcome } from "../bindings";
import { applyOutcomeMessages } from "../lib/format";

export function ApplyResultSnackbar({
  outcome,
  onClose,
}: {
  outcome: ApplyOutcome | null;
  onClose?: () => void;
}) {
  const lines = outcome ? applyOutcomeMessages(outcome) : [];
  const failed = outcome ? !("Reloaded" in outcome) : false;
  return (
    <Snackbar
      open={lines.length > 0}
      onClose={(_, reason) => reason !== "clickaway" && onClose?.()}
      anchorOrigin={{ vertical: "bottom", horizontal: "center" }}
    >
      <Alert severity={failed ? "error" : "warning"} variant="filled" onClose={onClose}>
        {lines.map((l) => (
          <div key={l}>{l}</div>
        ))}
      </Alert>
    </Snackbar>
  );
}

interface ApplyResultApi {
  show: (outcome: ApplyOutcome) => void;
  showError: (message: string) => void;
  /** A plain result message; `error` styles it as a failure. */
  showMessage: (message: string, severity: "success" | "error") => void;
}

const ApplyResultContext = createContext<ApplyResultApi>({ show: () => {}, showError: () => {}, showMessage: () => {} });

export function useApplyResult(): ApplyResultApi {
  return useContext(ApplyResultContext);
}

export function ApplyResultProvider({ children }: { children: ReactNode }) {
  const [notice, setNotice] = useState<
    | { kind: "apply"; outcome: ApplyOutcome }
    | { kind: "message"; text: string; severity: "success" | "error" }
    | null
  >(null);
  const show = useCallback((outcome: ApplyOutcome) => {
    setNotice(applyOutcomeMessages(outcome).length > 0 ? { kind: "apply", outcome } : null);
  }, []);
  const showMessage = useCallback((text: string, severity: "success" | "error") => setNotice({ kind: "message", text, severity }), []);
  const showError = useCallback((text: string) => setNotice({ kind: "message", text, severity: "error" }), []);
  const api = useMemo(() => ({ show, showError, showMessage }), [show, showError, showMessage]);
  return (
    <ApplyResultContext.Provider value={api}>
      {children}
      {notice?.kind === "apply" ? (
        <ApplyResultSnackbar outcome={notice.outcome} onClose={() => setNotice(null)} />
      ) : (
        <Snackbar
          open={notice?.kind === "message"}
          // Successes fade on their own; errors stay until dismissed.
          autoHideDuration={notice?.severity === "success" ? 4000 : null}
          onClose={(_, reason) => reason !== "clickaway" && setNotice(null)}
          anchorOrigin={{ vertical: "bottom", horizontal: "center" }}
        >
          <Alert severity={notice?.severity ?? "error"} variant="filled" sx={{ whiteSpace: "pre-line" }} onClose={() => setNotice(null)}>
            {notice?.text}
          </Alert>
        </Snackbar>
      )}
    </ApplyResultContext.Provider>
  );
}

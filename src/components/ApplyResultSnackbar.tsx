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
}

const ApplyResultContext = createContext<ApplyResultApi>({ show: () => {}, showError: () => {} });

export function useApplyResult(): ApplyResultApi {
  return useContext(ApplyResultContext);
}

export function ApplyResultProvider({ children }: { children: ReactNode }) {
  const [outcome, setOutcome] = useState<ApplyOutcome | null>(null);
  const [error, setError] = useState<string | null>(null);
  const show = useCallback((o: ApplyOutcome) => setOutcome(o), []);
  const showError = useCallback((m: string) => setError(m), []);
  const api = useMemo(() => ({ show, showError }), [show, showError]);
  return (
    <ApplyResultContext.Provider value={api}>
      {children}
      <ApplyResultSnackbar outcome={outcome} onClose={() => setOutcome(null)} />
      <Snackbar
        open={error !== null}
        onClose={(_, reason) => reason !== "clickaway" && setError(null)}
        anchorOrigin={{ vertical: "bottom", horizontal: "center" }}
      >
        <Alert severity="error" variant="filled" sx={{ whiteSpace: "pre-line" }} onClose={() => setError(null)}>
          {error}
        </Alert>
      </Snackbar>
    </ApplyResultContext.Provider>
  );
}

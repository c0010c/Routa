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
}

const ApplyResultContext = createContext<ApplyResultApi>({ show: () => {} });

export function useApplyResult(): ApplyResultApi {
  return useContext(ApplyResultContext);
}

export function ApplyResultProvider({ children }: { children: ReactNode }) {
  const [outcome, setOutcome] = useState<ApplyOutcome | null>(null);
  const show = useCallback((o: ApplyOutcome) => setOutcome(o), []);
  const api = useMemo(() => ({ show }), [show]);
  return (
    <ApplyResultContext.Provider value={api}>
      {children}
      <ApplyResultSnackbar outcome={outcome} onClose={() => setOutcome(null)} />
    </ApplyResultContext.Provider>
  );
}

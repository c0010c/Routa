import { Chip } from "@mui/material";
import type { ExitStatus, ListenerStatus } from "../bindings";
import { exitStatusLabel, listenerStatusLabel } from "../lib/format";

export function ListenerStatusChip({
  status,
  isPortFailed,
  notEffective,
}: {
  status: ListenerStatus;
  isPortFailed: boolean;
  /** The listener is not in effect (see `listenerNotEffective`). */
  notEffective: boolean;
}) {
  const l = listenerStatusLabel(status, isPortFailed, notEffective);
  return <Chip size="small" label={l.text} color={l.color} />;
}

export function ExitStatusChip({ status }: { status: ExitStatus }) {
  const l = exitStatusLabel(status);
  return <Chip size="small" label={l.text} color={l.color} />;
}

import { Box } from "@mui/material";

/**
 * Reserves the width of `reserve` behind `children` (shared grid cell, hidden ::before),
 * so a label that changes (刷新 → 刷新中…, normal → bold) doesn't resize its button.
 * The reserved text is CSS-only: not in textContent and, via the `/ ""` alt text, not announced.
 */
export function ReservedWidth({ reserve, bold, children }: { reserve: string; bold?: boolean; children: string }) {
  return (
    <Box
      component="span"
      data-reserve={reserve}
      sx={{
        display: "inline-grid",
        justifyItems: "center",
        "& > span, &::before": { gridArea: "1 / 1" },
        "&::before": { content: 'attr(data-reserve) / ""', visibility: "hidden", fontWeight: bold ? 600 : undefined },
      }}
    >
      <span>{children}</span>
    </Box>
  );
}

/** Button text that switches to a busy wording without resizing the button. */
export function BusyLabel({ busy, idle, pending }: { busy: boolean; idle: string; pending: string }) {
  return <ReservedWidth reserve={busy ? idle : pending}>{busy ? pending : idle}</ReservedWidth>;
}

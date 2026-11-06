import { Box, Paper, Typography } from "@mui/material";
import type { ReactNode } from "react";

/** What a list shows instead of an empty table: icon, one-line title, explanation, next step. */
export function EmptyState({
  icon,
  title,
  description,
  action,
}: {
  icon: ReactNode;
  title: string;
  description?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <Paper sx={{ py: 6, px: 3, textAlign: "center" }}>
      <Box sx={{ color: "text.disabled", "& svg": { fontSize: 48 }, mb: 1.5 }}>{icon}</Box>
      <Typography sx={{ fontWeight: 600 }}>{title}</Typography>
      {description && (
        <Typography variant="body2" sx={{ color: "text.secondary", mt: 0.5 }}>
          {description}
        </Typography>
      )}
      {action && <Box sx={{ mt: 2 }}>{action}</Box>}
    </Paper>
  );
}

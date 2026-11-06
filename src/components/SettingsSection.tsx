import { Box, Card, Divider, Stack, Typography } from "@mui/material";
import { Fragment, type ReactNode } from "react";

/**
 * One Win11-style settings row: label and explanation on the left, feedback and control on the right.
 * `below` holds secondary controls or messages that need the full width.
 */
export function Row({
  label,
  description,
  feedback,
  below,
  children,
}: {
  label: ReactNode;
  description?: ReactNode;
  feedback?: ReactNode;
  below?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <Box sx={{ px: 2, py: 1.25 }}>
      <Stack
        direction="row"
        sx={{ alignItems: "center", columnGap: 2, rowGap: 1, flexWrap: "wrap", minHeight: 40 }}
      >
        <Box sx={{ flex: "1 1 200px", minWidth: 0 }}>
          <Typography>{label}</Typography>
          {description && (
            <Typography variant="body2" sx={{ color: "text.secondary" }}>
              {description}
            </Typography>
          )}
        </Box>
        <Stack
          direction="row"
          spacing={1.5}
          sx={{ alignItems: "center", justifyContent: "flex-end", flexWrap: "wrap", rowGap: 1, ml: "auto" }}
        >
          {feedback}
          {children}
        </Stack>
      </Stack>
      {below && <Box sx={{ mt: 1 }}>{below}</Box>}
    </Box>
  );
}

/** A titled card of rows separated by dividers; `footer` sits under the rows inside the card. */
export function Section({
  title,
  action,
  children,
  footer,
}: {
  title: string;
  action?: ReactNode;
  children: ReactNode[];
  footer?: ReactNode;
}) {
  const rows = children.filter(Boolean);
  return (
    <Stack component="section" aria-label={title} spacing={1}>
      <Stack direction="row" spacing={0.5} sx={{ alignItems: "center" }}>
        <Typography variant="body2" sx={{ color: "text.secondary", fontWeight: 600 }}>
          {title}
        </Typography>
        {action}
      </Stack>
      <Card>
        {rows.map((c, i) => (
          <Fragment key={i}>
            {i > 0 && <Divider />}
            {c}
          </Fragment>
        ))}
        {footer}
      </Card>
    </Stack>
  );
}

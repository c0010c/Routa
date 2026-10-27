import { Typography } from "@mui/material";
import { delayColor } from "../lib/format";

/** A delay result coloured by band; `undefined` = never tested, `null` = timed out. */
export function DelayText({ ms }: { ms: number | null | undefined }) {
  if (ms === undefined) {
    return (
      <Typography variant="body2" component="span" sx={{ color: "text.secondary" }}>
        —
      </Typography>
    );
  }
  return (
    <Typography variant="body2" component="span" color={delayColor(ms)} sx={{ fontWeight: 600 }}>
      {ms === null ? "超时" : `${ms} ms`}
    </Typography>
  );
}

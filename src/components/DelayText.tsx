import { Tooltip, Typography } from "@mui/material";
import type { DelayResult } from "../bindings";
import { delayColor, delayFailure } from "../lib/format";

/**
 * A delay result coloured by band; `undefined` = never tested. A failure
 * shows its reason, with the explanation (plus `hint`) on hover.
 */
export function DelayText({ result, hint }: { result: DelayResult | undefined; hint?: string }) {
  if (result === undefined) {
    return (
      <Typography variant="body2" component="span" sx={{ color: "text.secondary" }}>
        —
      </Typography>
    );
  }
  const { ms } = result;
  if (ms !== null) {
    return (
      <Typography variant="body2" component="span" color={delayColor(ms)} sx={{ fontWeight: 600 }}>
        {ms} ms
      </Typography>
    );
  }
  const f = delayFailure(result.error);
  return (
    <Tooltip title={hint ? `${f.detail}。${hint}` : f.detail}>
      <Typography variant="body2" component="span" color="error" sx={{ fontWeight: 600, cursor: "help" }}>
        {f.text}
      </Typography>
    </Tooltip>
  );
}

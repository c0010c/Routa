import { Alert, Box, Button, FormControl, FormControlLabel, FormLabel, Radio, RadioGroup, TextField, Typography } from "@mui/material";
import { useState } from "react";
import { exitCandidates, isAutoExitNode, peerFor, useTailnetPeers } from "../lib/tailnet";

export const EXIT_NODE_HELP = "auto:any 在当前内核下无法自动选中出口节点，请改选一个出口节点";
const MANUAL = "manual";
const NONE = "none";

/**
 * Picks a Tailscale instance's `exit-node`: one of the approved exit nodes the
 * system client sees, none (tailnet devices only), or typed by hand. `""` is none.
 */
export function ExitNodeField({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const { peers, error, loading, refetch } = useTailnetPeers();
  const candidates = exitCandidates(peers);
  const [manualPicked, setManualPicked] = useState(false);
  const matched = peerFor(candidates, value);
  const choice = value.trim() === "" && !manualPicked ? NONE : matched && !manualPicked ? matched.ips[0] : MANUAL;

  const pick = (c: string) => {
    setManualPicked(c === MANUAL);
    if (c === NONE) onChange("");
    else if (c !== MANUAL) onChange(c);
  };

  return (
    <FormControl component="fieldset">
      <FormLabel component="legend" sx={{ mb: 0.5 }}>
        出口节点
      </FormLabel>
      {loading && (
        <Typography variant="body2" color="text.secondary">
          正在读取 tailnet 中的设备…
        </Typography>
      )}
      <RadioGroup value={choice} onChange={(e) => pick(e.target.value)}>
        {candidates.map((p) => (
          <FormControlLabel
            key={p.ips[0]}
            value={p.ips[0]}
            control={<Radio size="small" />}
            label={
              <Box component="span" sx={{ display: "inline-flex", gap: 1, alignItems: "baseline" }}>
                <span>{p.host_name}</span>
                <Typography component="span" variant="body2" sx={{ fontFamily: "monospace", color: "text.secondary" }}>
                  {p.ips[0]}
                </Typography>
                <Typography component="span" variant="caption" color={p.online ? "success.main" : "text.secondary"}>
                  {p.online ? "在线" : "离线"}
                </Typography>
              </Box>
            }
          />
        ))}
        <FormControlLabel value={NONE} control={<Radio size="small" />} label="不使用出口节点（只访问 tailnet 内设备）" />
        <FormControlLabel value={MANUAL} control={<Radio size="small" />} label="手动填写 IP 或机器名" />
      </RadioGroup>
      {choice === MANUAL && (
        <TextField
          size="small"
          label="exit-node"
          placeholder="100.64.0.1"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          error={isAutoExitNode(value)}
          helperText={isAutoExitNode(value) ? EXIT_NODE_HELP : "填写 tailnet 中出口节点的 IP 或机器名"}
          sx={{ mt: 1 }}
        />
      )}
      {!loading && candidates.length === 0 && (
        <Alert
          severity="info"
          sx={{ mt: 1 }}
          action={
            <Button size="small" onClick={() => void refetch()}>
              刷新
            </Button>
          }
        >
          {error
            ? `无法列出 tailnet 中的设备：${error}。可以手动填写出口节点。`
            : "tailnet 中还没有可用的出口节点：请在出口设备上开启「作为出口节点」，并在 Tailscale 管理后台批准。"}
        </Alert>
      )}
    </FormControl>
  );
}

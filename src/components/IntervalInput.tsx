import { FormControlLabel, Stack, Switch, TextField } from "@mui/material";
import { useState } from "react";

const MIN = 1;
const MAX = 525600;

const UNITS = [
  { label: "天", mins: 1440 },
  { label: "小时", mins: 60 },
  { label: "分钟", mins: 1 },
] as const;
type Unit = (typeof UNITS)[number]["label"];

const factor = (u: Unit) => UNITS.find((x) => x.label === u)!.mins;

/** Largest unit that divides the value evenly. */
function split(mins: number): { unit: Unit; amount: number } {
  const u = UNITS.find((x) => mins % x.mins === 0) ?? UNITS[2];
  return { unit: u.label, amount: mins / u.mins };
}

interface Props {
  /** Interval in minutes; `null` means disabled. */
  value: number | null;
  onChange: (mins: number | null) => void;
  /** Minutes used when the switch is turned on without a previous value. */
  defaultMins: number;
  label?: string;
  /** Called whenever validity changes; an invalid entry is never emitted via onChange. */
  onValidityChange?: (valid: boolean) => void;
}

/** Enable switch + number + unit (分钟/小时/天); always emits whole minutes within 1..=525600. */
export function IntervalInput({ value, onChange, defaultMins, label = "启用", onValidityChange }: Props) {
  const init = split(value ?? defaultMins);
  const [enabled, setEnabled] = useState(value !== null);
  const [unit, setUnit] = useState<Unit>(init.unit);
  const [text, setText] = useState(String(init.amount));
  const [error, setError] = useState<string>();

  const commit = (t: string, u: Unit) => {
    const n = Number(t);
    const mins = n * factor(u);
    if (t.trim() === "" || !Number.isInteger(n) || !Number.isInteger(mins) || mins < MIN || mins > MAX) {
      setError("超出范围");
      onValidityChange?.(false);
      return;
    }
    setError(undefined);
    onValidityChange?.(true);
    onChange(mins);
  };

  const toggle = (on: boolean) => {
    setEnabled(on);
    if (!on) {
      setError(undefined);
      onValidityChange?.(true);
      onChange(null);
    } else {
      commit(text, unit);
    }
  };

  return (
    <Stack direction="row" spacing={1} sx={{ alignItems: "flex-start" }}>
      <FormControlLabel
        sx={{ mt: 0.5 }}
        control={<Switch checked={enabled} onChange={(e) => toggle(e.target.checked)} />}
        label={label}
      />
      <TextField
        size="small"
        type="number"
        label="间隔"
        value={text}
        disabled={!enabled}
        error={!!error}
        helperText={error}
        slotProps={{ htmlInput: { min: 1 } }}
        onChange={(e) => {
          setText(e.target.value);
          commit(e.target.value, unit);
        }}
        sx={{ width: 140 }}
      />
      <TextField
        select
        size="small"
        label="单位"
        value={unit}
        disabled={!enabled}
        slotProps={{ select: { native: true } }}
        onChange={(e) => {
          const u = e.target.value as Unit;
          setUnit(u);
          commit(text, u);
        }}
        sx={{ width: 110 }}
      >
        {UNITS.map((u) => (
          <option key={u.label} value={u.label}>
            {u.label}
          </option>
        ))}
      </TextField>
    </Stack>
  );
}

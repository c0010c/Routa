import { Box, Chip, FormControl, InputLabel, MenuItem, Select } from "@mui/material";
import { useStatus } from "../api/client";
import type { InterfaceGuid } from "../bindings";
import { isInterfaceUsable } from "../lib/interfaces";

interface Props {
  value: InterfaceGuid | null;
  onChange: (v: InterfaceGuid | null) => void;
  disabled?: boolean;
  label?: string;
}

export function InterfacePicker({ value, onChange, disabled, label = "网卡" }: Props) {
  const status = useStatus();
  const interfaces = status.data?.interfaces ?? [];
  const removed = value !== null && !interfaces.some((i) => i.guid === value);
  return (
    <FormControl size="small" fullWidth>
      {/* shrink + displayEmpty: "系统默认" is the empty value and must still show. */}
      <InputLabel id="iface-label" shrink>
        {label}
      </InputLabel>
      <Select
        disabled={disabled}
        labelId="iface-label"
        label={label}
        displayEmpty
        notched
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value === "" ? null : e.target.value)}
      >
        <MenuItem value="">系统默认</MenuItem>
        {removed && <MenuItem value={value}>（已移除的网卡）</MenuItem>}
        {interfaces.map((i) => (
          <MenuItem key={i.guid} value={i.guid}>
            <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
              {i.friendly_name}
              {i.ipv4.length > 0 && <span>（{i.ipv4.join(", ")}）</span>}
              {!isInterfaceUsable(i) && <Chip size="small" color="error" label="不可用" />}
            </Box>
          </MenuItem>
        ))}
      </Select>
    </FormControl>
  );
}

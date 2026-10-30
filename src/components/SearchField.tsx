import Search from "@mui/icons-material/Search";
import { InputAdornment, TextField, type SxProps, type Theme } from "@mui/material";

/** A shared, vertically centered search control for pages and node pickers. */
export function SearchField({ label, value, onChange, sx }: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  sx?: SxProps<Theme>;
}) {
  return <TextField
    size="small"
    placeholder={label}
    value={value}
    onChange={(event) => onChange(event.target.value)}
    slotProps={{
      htmlInput: { "aria-label": label },
      input: { startAdornment: <InputAdornment position="start"><Search sx={{ fontSize: 18 }} /></InputAdornment> },
    }}
    sx={sx}
  />;
}

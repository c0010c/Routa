import MoreHoriz from "@mui/icons-material/MoreHoriz";
import { IconButton, Menu, MenuItem } from "@mui/material";
import { useState } from "react";

export interface RowAction {
  label: string;
  onClick: () => void;
  danger?: boolean;
}

/** A row's secondary actions behind a ··· button. Item names read "<action> <row label>". */
export function RowActions({ label, items }: { label: string; items: RowAction[] }) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  return (
    <>
      <IconButton size="small" aria-label={`操作 ${label}`} onClick={(e) => setAnchor(e.currentTarget)}>
        <MoreHoriz fontSize="small" />
      </IconButton>
      <Menu anchorEl={anchor} open={!!anchor} onClose={() => setAnchor(null)}>
        {items.map((it) => (
          <MenuItem
            key={it.label}
            aria-label={`${it.label} ${label}`}
            sx={it.danger ? { color: "error.main" } : undefined}
            onClick={() => {
              setAnchor(null);
              it.onClick();
            }}
          >
            {it.label}
          </MenuItem>
        ))}
      </Menu>
    </>
  );
}

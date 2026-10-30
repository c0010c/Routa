import CheckCircle from "@mui/icons-material/CheckCircle";
import MoreHoriz from "@mui/icons-material/MoreHoriz";
import { alpha, Box, ButtonBase, Chip, IconButton, Menu, MenuItem, Paper, Typography } from "@mui/material";
import { useId, useState, type KeyboardEventHandler, type ReactNode } from "react";
import type { DelayResult } from "../bindings";
import { DelayText } from "./DelayText";
import type { RowAction } from "./RowActions";

export function NodeCard({
  name, source, protocol, delay, status, actions, selected = false, current = false,
  onSelect, onSelectionKeyDown, selectionTabIndex, disabled = false, pending = false,
  running = true, gridKey,
}: {
  name: string; source: string; protocol: string; delay?: DelayResult; status?: ReactNode;
  actions: RowAction[]; selected?: boolean; current?: boolean; onSelect?: () => void;
  onSelectionKeyDown?: KeyboardEventHandler<HTMLButtonElement>; selectionTabIndex?: number;
  disabled?: boolean; pending?: boolean; running?: boolean; testTarget?: string; testUrl?: string; gridKey?: string;
}) {
  const menuId = useId();
  const [menu, setMenu] = useState<{ anchor?: HTMLElement; position?: { top: number; left: number } } | null>(null);
  const hasMark = selected || current || !!status;
  return <>
    <Paper component="article" aria-label={name} data-grid-key={gridKey} tabIndex={-1}
      onContextMenu={(e) => {
        if (!actions.length || disabled) return;
        e.preventDefault(); e.stopPropagation(); e.currentTarget.focus();
        setMenu({ position: { top: e.clientY, left: e.clientX } });
      }}
      onKeyDown={(e) => {
        if (actions.length && !disabled && (e.key === "ContextMenu" || (e.shiftKey && e.key === "F10"))) {
          e.preventDefault(); e.stopPropagation();
          const r = e.currentTarget.getBoundingClientRect();
          setMenu({ position: { top: r.top + 20, left: r.left + 20 } });
        }
      }}
      sx={(theme) => {
        const tint = (opacity: number) => theme.vars ? `rgba(${theme.vars.palette.primary.mainChannel} / ${opacity})` : alpha(theme.palette.primary.main, opacity);
        return {
          height: 64, minWidth: 0, boxSizing: "border-box", position: "relative", overflow: "hidden", borderRadius: "7px",
          bgcolor: selected ? tint(0.10) : "background.paper", borderColor: selected ? tint(0.30) : "divider",
          transition: "background-color 120ms ease, border-color 120ms ease",
          "&:hover": { bgcolor: selected ? tint(0.13) : "action.hover", borderColor: selected ? tint(0.35) : "action.disabled" },
          "&::before": selected ? { content: '\"\"', position: "absolute", left: 0, top: 8, bottom: 8, width: 3, bgcolor: "primary.main", borderRadius: "0 2px 2px 0", pointerEvents: "none" } : {},
          "&:focus-visible, &:has(:focus-visible)": { outline: "2px solid", outlineColor: "primary.main", outlineOffset: 2 },
          "@media (prefers-reduced-motion: reduce)": { transition: "none" },
        };
      }}>
        <ButtonBase component={onSelect ? "button" : "div"}
          role={onSelect ? "radio" : undefined} aria-label={onSelect ? `${name} · ${source}` : undefined}
          aria-checked={onSelect ? selected : undefined} aria-busy={pending || undefined}
          tabIndex={onSelect ? selectionTabIndex ?? 0 : 0} disabled={disabled}
          onClick={onSelect} onKeyDown={onSelectionKeyDown} disableRipple
          sx={{ width: "100%", height: "100%", p: "9px 11px", display: "grid", boxSizing: "border-box",
            gridTemplateColumns: "minmax(0,1fr) 60px", gridTemplateRows: "20px 18px", gap: "6px",
            alignContent: "start", textAlign: "left", cursor: onSelect ? "pointer" : "default" }}>
          <Typography noWrap variant="body2" sx={{ minWidth: 0, fontSize: 14, fontWeight: 600, lineHeight: "20px", gridColumn: 1, gridRow: 1 }}>{name}</Typography>
          <Box sx={{ gridColumn: 2, gridRow: 1, height: 20, display: "flex", justifyContent: "flex-end", minWidth: 0 }}>
            <DelayText result={delay} variant="card" emptyText="未测试" tooltip={false} pending={pending} running={running} />
          </Box>
          <Box sx={{ gridColumn: "1 / 3", gridRow: 2, display: "flex", gap: "7px", alignItems: "center", minWidth: 0, height: 18, pr: hasMark ? "84px" : actions.length ? "27px" : 0 }}>
            <Chip label={protocol} size="small" sx={{ height: 18, fontSize: 12, maxWidth: "60%", bgcolor: "action.hover", borderColor: "transparent", "& .MuiChip-label": { px: "5px" } }} />
            <Typography noWrap title={source} variant="caption" color="text.secondary" sx={{ minWidth: 0, flex: 1, fontSize: 12, lineHeight: "18px" }}>{source}</Typography>
          </Box>
          {hasMark && <Box component="span" aria-label={current ? "当前使用的节点" : selected ? "已选择，等待确认" : undefined} sx={{ position: "absolute", right: actions.length ? 36 : 11, bottom: 9, height: 18, display: "flex", alignItems: "center", gap: "4px", fontSize: 11, color: selected ? "primary.main" : "text.secondary", maxWidth: 58, overflow: "hidden", "& .MuiChip-root": { height: 18, fontSize: 11, border: 0, maxWidth: "100%", "& .MuiChip-label": { px: 0 } } }}>
            {selected ? <CheckCircle sx={{ fontSize: 12 }} /> : current ? <Box component="span" sx={{ width: 4, height: 4, borderRadius: "50%", bgcolor: "text.secondary", flexShrink: 0 }} /> : null}
            {current ? "当前" : selected ? "已选择" : status}
          </Box>}
        </ButtonBase>
      {actions.length > 0 && <IconButton size="small" aria-label={`操作 ${name}`} aria-haspopup="menu" aria-controls={menu ? menuId : undefined} aria-expanded={!!menu} disabled={disabled}
        onClick={(e) => { e.stopPropagation(); setMenu({ anchor: e.currentTarget }); }}
        sx={{ position: "absolute", right: 6, bottom: 4, width: 26, height: 26, p: 0, color: "text.secondary" }}><MoreHoriz sx={{ fontSize: 16 }} /></IconButton>}
    </Paper>
    <Menu id={menuId} anchorReference={menu?.position ? "anchorPosition" : "anchorEl"} anchorPosition={menu?.position} anchorEl={menu?.anchor ?? null} open={!!menu} onClose={() => setMenu(null)} slotProps={{ list: { "aria-label": `${name} 节点操作` } }}>
      {actions.map((action) => <MenuItem key={action.label} aria-label={`${action.label} ${name}`} disabled={action.disabled} sx={action.danger ? { color: "error.main" } : undefined} onClick={() => { setMenu(null); action.onClick(); }}>{action.label}</MenuItem>)}
    </Menu>
  </>;
}

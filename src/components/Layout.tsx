import { Box, Drawer, List, ListItemButton, ListItemText } from "@mui/material";
import type { ReactNode } from "react";
import { NavLink } from "react-router-dom";

const DRAWER_WIDTH = 200;

export const pages = [
  { path: "/", label: "概览" },
  { path: "/listeners", label: "Listener" },
  { path: "/exits", label: "出口" },
  { path: "/nodes", label: "节点" },
  { path: "/tailscale", label: "Tailscale" },
  { path: "/settings", label: "设置" },
];

export function Layout({ children }: { children: ReactNode }) {
  return (
    <Box sx={{ display: "flex" }}>
      <Drawer
        variant="permanent"
        sx={{
          width: DRAWER_WIDTH,
          flexShrink: 0,
          "& .MuiDrawer-paper": { width: DRAWER_WIDTH, boxSizing: "border-box" },
        }}
      >
        <List component="nav" aria-label="主导航">
          {pages.map((p) => (
            <ListItemButton key={p.path} component={NavLink} to={p.path} end={p.path === "/"}>
              <ListItemText primary={p.label} />
            </ListItemButton>
          ))}
        </List>
      </Drawer>
      <Box component="main" sx={{ flexGrow: 1, p: 3, minWidth: 0 }}>
        {children}
      </Box>
    </Box>
  );
}

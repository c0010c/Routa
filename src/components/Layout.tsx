import DashboardOutlined from "@mui/icons-material/DashboardOutlined";
import HubOutlined from "@mui/icons-material/HubOutlined";
import InputOutlined from "@mui/icons-material/InputOutlined";
import LogoutOutlined from "@mui/icons-material/LogoutOutlined";
import SettingsOutlined from "@mui/icons-material/SettingsOutlined";
import VpnLockOutlined from "@mui/icons-material/VpnLockOutlined";
import { Box, Drawer, List, ListItemButton, ListItemIcon, ListItemText } from "@mui/material";
import type { ReactNode } from "react";
import { NavLink } from "react-router-dom";

const DRAWER_WIDTH = 200;

export const pages = [
  { path: "/", label: "概览", icon: <DashboardOutlined /> },
  { path: "/listeners", label: "监听器", icon: <InputOutlined /> },
  { path: "/exits", label: "出口", icon: <LogoutOutlined /> },
  { path: "/nodes", label: "节点", icon: <HubOutlined /> },
  { path: "/tailscale", label: "Tailscale", icon: <VpnLockOutlined /> },
  { path: "/settings", label: "设置", icon: <SettingsOutlined /> },
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
        <List component="nav" aria-label="主导航" sx={{ px: 1 }}>
          {pages.map((p) => (
            <ListItemButton
              key={p.path}
              component={NavLink}
              to={p.path}
              end={p.path === "/"}
              sx={{
                borderRadius: 1,
                mb: 0.5,
                // NavLink marks the current page with `.active`.
                "&.active": { bgcolor: "action.selected", color: "primary.main", fontWeight: 600 },
                "&.active .MuiListItemIcon-root": { color: "primary.main" },
              }}
            >
              <ListItemIcon sx={{ minWidth: 36 }}>{p.icon}</ListItemIcon>
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

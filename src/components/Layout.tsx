import AppsOutlined from "@mui/icons-material/AppsOutlined";
import AltRouteOutlined from "@mui/icons-material/AltRouteOutlined";
import DashboardOutlined from "@mui/icons-material/DashboardOutlined";
import HubOutlined from "@mui/icons-material/HubOutlined";
import InputOutlined from "@mui/icons-material/InputOutlined";
import RouteOutlined from "@mui/icons-material/RouteOutlined";
import SettingsOutlined from "@mui/icons-material/SettingsOutlined";
import VpnLockOutlined from "@mui/icons-material/VpnLockOutlined";
import { Box, Drawer, List, ListItemButton, ListItemIcon, ListItemText, Stack, Typography } from "@mui/material";
import type { ReactNode } from "react";
import { Link, useLocation } from "react-router-dom";
import pkg from "../../package.json";

const DRAWER_WIDTH = 200;

export const pages = [
  { path: "/", label: "概览", icon: <DashboardOutlined /> },
  { path: "/listeners", label: "监听器", icon: <InputOutlined /> },
  { path: "/process-routing", label: "进程分流", icon: <AppsOutlined /> },
  { path: "/exits", label: "出口", icon: <AltRouteOutlined /> },
  { path: "/nodes", label: "节点", icon: <HubOutlined /> },
  { path: "/tailscale", label: "Tailscale", icon: <VpnLockOutlined /> },
  { path: "/settings", label: "设置", icon: <SettingsOutlined /> },
];

function isCurrent(path: string, current: string): boolean {
  return path === "/" ? current === "/" : current === path || current.startsWith(`${path}/`);
}

export function Layout({ children }: { children: ReactNode }) {
  const { pathname } = useLocation();
  return (
    <Box sx={{ display: "flex", minHeight: "100vh", bgcolor: "background.default" }}>
      <Drawer
        variant="permanent"
        sx={{
          width: DRAWER_WIDTH,
          flexShrink: 0,
          "& .MuiDrawer-paper": {
            width: DRAWER_WIDTH,
            boxSizing: "border-box",
            border: "none",
            bgcolor: "background.default",
          },
        }}
      >
        <Stack direction="row" spacing={1.25} sx={{ alignItems: "center", px: 2.5, pt: 2, pb: 1.5 }}>
          <Box
            sx={{
              width: 24,
              height: 24,
              borderRadius: 1,
              bgcolor: "primary.main",
              color: "primary.contrastText",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
          >
            <RouteOutlined sx={{ fontSize: 16 }} />
          </Box>
          <Typography sx={{ fontWeight: 600 }}>Routa</Typography>
        </Stack>
        <List component="nav" aria-label="主导航" sx={{ px: 1 }}>
          {pages.map((p) => (
            <ListItemButton
              key={p.path}
              component={Link}
              to={p.path}
              selected={isCurrent(p.path, pathname)}
              sx={{ mb: 0.5 }}
            >
              <ListItemIcon sx={{ minWidth: 36, color: "inherit" }}>{p.icon}</ListItemIcon>
              <ListItemText primary={p.label} />
            </ListItemButton>
          ))}
        </List>
        <Typography variant="caption" sx={{ mt: "auto", px: 2.5, pb: 2, color: "text.secondary" }}>
          v{pkg.version}
        </Typography>
      </Drawer>
      <Box component="main" sx={{ flexGrow: 1, p: 3, minWidth: 0 }}>
        {children}
      </Box>
    </Box>
  );
}

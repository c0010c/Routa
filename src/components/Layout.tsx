import AltRouteOutlined from "@mui/icons-material/AltRouteOutlined";
import SpaceDashboardOutlined from "@mui/icons-material/SpaceDashboardOutlined";
import DnsOutlined from "@mui/icons-material/DnsOutlined";
import SwapHorizOutlined from "@mui/icons-material/SwapHorizOutlined";
import LanguageOutlined from "@mui/icons-material/LanguageOutlined";
import SettingsOutlined from "@mui/icons-material/SettingsOutlined";
import {
  Box,
  Drawer,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Stack,
  Typography,
} from "@mui/material";
import type { ReactNode } from "react";
import { Link, useLocation } from "react-router-dom";
import { BrandLockup } from "./BrandLockup";
import { UpdateNavButton } from "./UpdateNavButton";
import { UpdateNotice } from "./UpdateNotice";
import pkg from "../../package.json";

const DRAWER_WIDTH = 184;

export const pages = [
  { path: "/", label: "概览", icon: <SpaceDashboardOutlined /> },
  { path: "/listeners", label: "代理", icon: <SwapHorizOutlined /> },
  { path: "/process-routing", label: "进程分流", icon: <AltRouteOutlined /> },
  { path: "/subscriptions", label: "订阅", icon: <LanguageOutlined /> },
  { path: "/nodes", label: "节点", icon: <DnsOutlined /> },
  { path: "/settings", label: "设置", icon: <SettingsOutlined /> },
];

function isCurrent(path: string, current: string): boolean {
  return path === "/"
    ? current === "/"
    : current === path || current.startsWith(`${path}/`);
}

export function Layout({ children }: { children: ReactNode }) {
  const { pathname } = useLocation();
  return (
    <Box
      sx={{
        display: "flex",
        minHeight: "100vh",
        bgcolor: "background.default",
      }}
    >
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
        <Stack
          direction="row"
          sx={{ alignItems: "center", px: 2.5, pt: 2, pb: 1.5 }}
        >
          <BrandLockup height={30} />
        </Stack>
        <List
          component="nav"
          aria-label="主导航"
          sx={{ px: 1, flex: 1, display: "flex", flexDirection: "column" }}
        >
          {pages.map((p) => {
            const isSettings = p.path === "/settings";
            const item = (
              <ListItemButton
                key={p.path}
                component={Link}
                to={p.path}
                selected={isCurrent(p.path, pathname)}
                sx={{
                  mb: 0.5,
                  flexGrow: 0,
                  minHeight: 42,
                  mt: isSettings ? "auto" : 0,
                }}
              >
                <ListItemIcon sx={{ minWidth: 36, color: "inherit" }}>
                  {p.icon}
                </ListItemIcon>
                <ListItemText primary={p.label} />
              </ListItemButton>
            );
            if (!isSettings) return item;
            // The update button is a sibling, not a child: a <button> inside the
            // row's <a> would be invalid and its clicks would also navigate.
            return (
              <Box key={p.path} sx={{ position: "relative", mt: "auto", display: "flex", flexDirection: "column" }}>
                {item}
                <Box sx={{ position: "absolute", right: 10, top: 21, transform: "translateY(-50%)" }}>
                  <UpdateNavButton />
                </Box>
              </Box>
            );
          })}
        </List>
        <Typography
          variant="caption"
          sx={{ px: 2.5, pb: 2, color: "text.secondary" }}
        >
          v{pkg.version}
        </Typography>
      </Drawer>
      <Box component="main" sx={{ flexGrow: 1, p: 3, minWidth: 0 }}>
        {children}
      </Box>
      <UpdateNotice />
    </Box>
  );
}

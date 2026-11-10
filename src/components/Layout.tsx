import AltRouteOutlined from "@mui/icons-material/AltRouteOutlined";
import SmartToyOutlined from "@mui/icons-material/SmartToyOutlined";
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
import { useEffect, useRef, type ReactNode } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { BrandLockup } from "./BrandLockup";
import { UpdateNavButton } from "./UpdateNavButton";
import { UpdateNotice } from "./UpdateNotice";
import { clearMonitorFocus, useMonitorFocus } from "../api/monitorFocus";
import pkg from "../../package.json";

const DRAWER_WIDTH = 184;

export const pages = [
  { path: "/", label: "概览", icon: <SpaceDashboardOutlined /> },
  { path: "/listeners", label: "代理", icon: <SwapHorizOutlined /> },
  { path: "/ai-apps", label: "AI 应用", icon: <SmartToyOutlined /> },
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

/**
 * Routes a toast click to its page. A node focus is left in the store for the node page to
 * highlight (it may mount only after this navigation); the other targets have nothing to consume it.
 */
export function useMonitorFocusNavigation() {
  const focus = useMonitorFocus();
  const navigate = useNavigate();
  // `navigate` changes with the location, so remember the focus already routed: a node focus
  // stays set until the node page consumes it and must not navigate again in the meantime.
  const handled = useRef<typeof focus>(null);
  const { pathname: current } = useLocation();
  const pathname = useRef(current);
  useEffect(() => { pathname.current = current; }, [current]);
  useEffect(() => {
    if (focus === handled.current) return;
    handled.current = focus;
    if (!focus) return;
    const go = (to: string, path: string) => navigate(to, { replace: pathname.current === path });
    if (typeof focus === "object") return void go("/nodes", "/nodes");
    if (focus === "Listeners") go("/listeners", "/listeners");
    else go("/settings?tab=network", "/settings");
    clearMonitorFocus();
  }, [focus, navigate]);
}

export function Layout({ children }: { children: ReactNode }) {
  const { pathname } = useLocation();
  useMonitorFocusNavigation();
  return (
    <Box
      sx={{
        display: "flex",
        // Fixed to the window so only <main> scrolls: the scrollbar stays inside
        // the content column instead of running down the window edge.
        height: "100vh",
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
      <Box
        component="main"
        sx={{ flexGrow: 1, p: 3, minWidth: 0, overflowY: "auto", scrollbarGutter: "stable" }}
      >
        {children}
      </Box>
      <UpdateNotice />
    </Box>
  );
}

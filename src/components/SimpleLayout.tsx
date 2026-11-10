import { Box, Button, Divider, Stack } from "@mui/material";
import DnsOutlined from "@mui/icons-material/DnsOutlined";
import SettingsOutlined from "@mui/icons-material/SettingsOutlined";
import PowerSettingsNewOutlined from "@mui/icons-material/PowerSettingsNewOutlined";
import { Link, useLocation } from "react-router-dom";
import type { ReactNode } from "react";
import { ReservedWidth } from "./BusyLabel";
import { useMonitorFocusNavigation } from "./Layout";
import { UpdateNotice } from "./UpdateNotice";

const NAV = [
  { to: "/", label: "连接", icon: <PowerSettingsNewOutlined />, match: (p: string) => p === "/" },
  { to: "/nodes", label: "订阅与节点", icon: <DnsOutlined />, match: (p: string) => p === "/nodes" || p === "/subscriptions" },
  { to: "/settings", label: "设置", icon: <SettingsOutlined />, match: (p: string) => p === "/settings" || p === "/diagnostics" },
];

export function SimpleLayout({ children }: { children: ReactNode }) {
  const { pathname } = useLocation();
  useMonitorFocusNavigation();
  return (
    <Stack
      sx={{
        minHeight: "100vh",
        maxWidth: 960,
        mx: "auto",
        p: { xs: 2, sm: 3.5 },
      }}
    >
      <Box component="main" sx={{ flex: 1, minWidth: 0 }}>
        {children}
      </Box>
      <UpdateNotice />
      <Divider sx={{ mt: 3 }} />
      <Stack
        component="nav"
        aria-label="简洁导航"
        direction="row"
        sx={{ pt: 1.5, justifyContent: "space-between" }}
      >
        {NAV.map((item) => {
          const current = item.match(pathname);
          return (
            <Button
              key={item.to}
              component={Link}
              to={item.to}
              startIcon={item.icon}
              aria-current={current ? "page" : undefined}
              color={current ? "primary" : "inherit"}
              sx={{ fontWeight: current ? 600 : undefined }}
            >
              {/* Reserving the bold width keeps switching pages from nudging the other items. */}
              <ReservedWidth reserve={item.label} bold>
                {item.label}
              </ReservedWidth>
            </Button>
          );
        })}
      </Stack>
    </Stack>
  );
}

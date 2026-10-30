import { Box, Button, Divider, Stack } from "@mui/material";
import DnsOutlined from "@mui/icons-material/DnsOutlined";
import SettingsOutlined from "@mui/icons-material/SettingsOutlined";
import { Link } from "react-router-dom";
import type { ReactNode } from "react";

export function SimpleLayout({ children }: { children: ReactNode }) {
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
      <Divider sx={{ mt: 3 }} />
      <Stack
        component="nav"
        aria-label="简洁导航"
        direction="row"
        sx={{ pt: 1.5, justifyContent: "space-between" }}
      >
        <Button
          component={Link}
          to="/nodes"
          startIcon={<DnsOutlined />}
          color="inherit"
        >
          订阅与节点
        </Button>
        <Button
          component={Link}
          to="/settings"
          startIcon={<SettingsOutlined />}
          color="inherit"
        >
          设置
        </Button>
      </Stack>
    </Stack>
  );
}

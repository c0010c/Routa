import {
  Alert,
  Button,
  CircularProgress,
  CssBaseline,
  Stack,
  ThemeProvider,
  Typography,
} from "@mui/material";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useRuntimeEvents } from "./api/events";
import { ApplyResultProvider } from "./components/ApplyResultSnackbar";
import { Layout } from "./components/Layout";

import { Listeners } from "./pages/Listeners";
import { FullOverview } from "./pages/FullOverview";
import { ProcessRoutingPage } from "./pages/ProcessRouting";
import { Settings } from "./pages/Settings";
import { ResourceCenter } from "./pages/ResourceCenter";
import { Welcome } from "./pages/Welcome";
import { SimpleConnection } from "./pages/SimpleConnection";
import { SimpleSettings } from "./pages/SimpleSettings";
import { Diagnostics } from "./pages/Diagnostics";
import {
  InterfaceProvider,
  useInterface,
} from "./components/InterfaceProvider";
import { SimpleLayout } from "./components/SimpleLayout";
import { errorMessage } from "./api/client";
import { theme } from "./theme";

function Events() {
  useRuntimeEvents();
  return null;
}

function ApplicationShell() {
  const ui = useInterface()!;
  const mode = ui.state?.preferences.mode;
  const captureMode = ui.state?.preferences.capture_mode;
  useEffect(() => {
    if (!isTauri() || mode === undefined) return;
    const window = getCurrentWindow();
    void (async () => {
      await window.setMinSize(
        new LogicalSize(mode === "Full" ? 900 : 480, 640),
      );
      await window.setSize(
        new LogicalSize(
          mode === "Full" ? 1100 : captureMode === "Apps" ? 920 : 560,
          mode === "Full" ? 800 : 720,
        ),
      );
    })().catch(() => {});
  }, [mode, captureMode]);
  if (!ui.state)
    return (
      <Stack spacing={2} sx={{ p: 5, alignItems: "center" }}>
        {ui.error ? (
          <Alert
            severity="error"
            action={<Button onClick={ui.retry}>重试</Button>}
          >
            {errorMessage(ui.error)}
          </Alert>
        ) : (
          <>
            <CircularProgress />
            <Typography>正在读取界面设置…</Typography>
          </>
        )}
      </Stack>
    );
  if (mode === null) return <Welcome />;
  if (mode === "Simple")
    return (
      <SimpleLayout>
        <Routes>
          <Route path="/" element={<SimpleConnection />} />
          <Route path="/nodes" element={<ResourceCenter />} />
          <Route path="/subscriptions" element={<ResourceCenter subscriptionsOnly />} />
          <Route path="/settings" element={<SimpleSettings />} />
          <Route path="/diagnostics" element={<Diagnostics />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </SimpleLayout>
    );
  return (
    <Layout>
      <Routes>
        <Route path="/" element={<FullOverview />} />
        <Route path="/listeners" element={<Listeners />} />
        <Route path="/process-routing" element={<ProcessRoutingPage />} />
        <Route path="/exits" element={<Navigate to="/listeners" replace />} />
        <Route path="/subscriptions" element={<ResourceCenter subscriptionsOnly />} />
        <Route path="/nodes" element={<ResourceCenter />} />
        <Route
          path="/tailscale"
          element={<Navigate to="/nodes?tab=tailscale" replace />}
        />
        <Route path="/settings" element={<Settings />} />
        <Route path="/diagnostics" element={<Diagnostics />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Layout>
  );
}

export default function App() {
  // Keep the client with its query observers when Fast Refresh re-evaluates this module.
  const [queryClient] = useState(() => new QueryClient());

  return (
    <ThemeProvider
      theme={theme}
      defaultMode="light"
      modeStorageKey="routa-color-mode"
    >
      <QueryClientProvider client={queryClient}>
        <ApplyResultProvider>
          <Events />
          <InterfaceProvider>
            <HashRouter>
              <CssBaseline enableColorScheme />
              <ApplicationShell />
            </HashRouter>
          </InterfaceProvider>
        </ApplyResultProvider>
      </QueryClientProvider>
    </ThemeProvider>
  );
}

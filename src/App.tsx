import { CssBaseline, ThemeProvider } from "@mui/material";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useState } from "react";
import { HashRouter, Route, Routes } from "react-router-dom";
import { useRuntimeEvents } from "./api/events";
import { ApplyResultProvider } from "./components/ApplyResultSnackbar";
import { Layout } from "./components/Layout";
import { Exits } from "./pages/Exits";
import { Listeners } from "./pages/Listeners";
import { Nodes } from "./pages/Nodes";
import { Overview } from "./pages/Overview";
import { ProcessRoutingPage } from "./pages/ProcessRouting";
import { Settings } from "./pages/Settings";
import { Tailscale } from "./pages/Tailscale";
import { theme } from "./theme";

function Events() {
  useRuntimeEvents();
  return null;
}

export default function App() {
  // Keep the client with its query observers when Fast Refresh re-evaluates this module.
  const [queryClient] = useState(() => new QueryClient());

  return (
    <ThemeProvider theme={theme}>
      <QueryClientProvider client={queryClient}>
        <ApplyResultProvider>
          <Events />
          <HashRouter>
            <CssBaseline enableColorScheme />
            <Layout>
              <Routes>
                <Route path="/" element={<Overview />} />
                <Route path="/listeners" element={<Listeners />} />
                <Route path="/process-routing" element={<ProcessRoutingPage />} />
                <Route path="/exits" element={<Exits />} />
                <Route path="/nodes" element={<Nodes />} />
                <Route path="/tailscale" element={<Tailscale />} />
                <Route path="/settings" element={<Settings />} />
              </Routes>
            </Layout>
          </HashRouter>
        </ApplyResultProvider>
      </QueryClientProvider>
    </ThemeProvider>
  );
}

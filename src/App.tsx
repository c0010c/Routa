import { CssBaseline } from "@mui/material";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HashRouter, Route, Routes } from "react-router-dom";
import { useRuntimeEvents } from "./api/events";
import { ApplyResultProvider } from "./components/ApplyResultSnackbar";
import { Layout } from "./components/Layout";
import { Exits } from "./pages/Exits";
import { Listeners } from "./pages/Listeners";
import { Nodes } from "./pages/Nodes";
import { Overview } from "./pages/Overview";
import { Settings } from "./pages/Settings";
import { Tailscale } from "./pages/Tailscale";

const queryClient = new QueryClient();

function Events() {
  useRuntimeEvents();
  return null;
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ApplyResultProvider>
        <Events />
        <HashRouter>
          <CssBaseline />
          <Layout>
            <Routes>
              <Route path="/" element={<Overview />} />
              <Route path="/listeners" element={<Listeners />} />
              <Route path="/exits" element={<Exits />} />
              <Route path="/nodes" element={<Nodes />} />
              <Route path="/tailscale" element={<Tailscale />} />
              <Route path="/settings" element={<Settings />} />
            </Routes>
          </Layout>
        </HashRouter>
      </ApplyResultProvider>
    </QueryClientProvider>
  );
}

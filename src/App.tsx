import { CssBaseline, Typography } from "@mui/material";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HashRouter, Route, Routes } from "react-router-dom";
import { useRuntimeEvents } from "./api/events";
import { ApplyResultProvider } from "./components/ApplyResultSnackbar";
import { Layout, pages } from "./components/Layout";
import { Overview } from "./pages/Overview";

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
              {pages
                .filter((p) => p.path !== "/")
                .map((p) => (
                  <Route
                    key={p.path}
                    path={p.path}
                    element={<Typography variant="h5">{p.label}</Typography>}
                  />
                ))}
            </Routes>
          </Layout>
        </HashRouter>
      </ApplyResultProvider>
    </QueryClientProvider>
  );
}

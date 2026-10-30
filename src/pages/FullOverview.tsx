import { useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Alert,
  Box,
  Button,
  Stack,
  Tab,
  Tabs,
  Typography,
} from "@mui/material";
import { RuntimeSummary } from "./RuntimeSummary";
import { desktopApi } from "../api/desktop";
import { errorMessage } from "../api/client";
import { createConnectionSampler } from "../lib/connections";
import { ConnectionsPanel } from "./ConnectionsPanel";
import { LogsPanel } from "./LogsPanel";
export function FullOverview() {
  const [tab, setTab] = useState("summary");
  const [grouped, setGrouped] = useState(true);
  const [logsPaused, setLogsPaused] = useState(false);
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const sampler = useRef(createConnectionSampler());
  const connections = useQuery({
    queryKey: ["connections", "live"],
    queryFn: async () => {
      try {
        return sampler.current.sample(await desktopApi.connections(), performance.now());
      } catch (error) {
        sampler.current.reset();
        throw error;
      }
    },
    retry: false,
    enabled: tab === "connections",
    refetchInterval: tab === "connections" ? 2000 : false,
  });
  const logs = useQuery({
    queryKey: ["core-log"],
    queryFn: desktopApi.logs,
    enabled: tab === "logs" && !logsPaused,
    retry: false,
    refetchInterval: tab === "logs" && !logsPaused ? 3000 : false,
  });
  return (
    <Stack spacing={2}>
      <Typography variant="h5">概览</Typography>
      <Tabs aria-label="概览内容" value={tab} onChange={(_, v) => { sampler.current.reset(); setTab(v); }}>
        <Tab id="overview-summary-tab" aria-controls="overview-summary" value="summary" label="运行概况" />
        <Tab id="overview-connections-tab" aria-controls="overview-connections" value="connections" label="连接" />
        <Tab id="overview-logs-tab" aria-controls="overview-logs" value="logs" label="日志" />
      </Tabs>
      <Box role="tabpanel" id={`overview-${tab}`} aria-labelledby={`overview-${tab}-tab`} sx={{
        "--overview-content-height": "clamp(240px, calc(100dvh - 216px), 480px)",
        minHeight: "calc(var(--overview-content-height) + 56px)",
      }}>
      {tab === "summary" ? (
        <RuntimeSummary />
      ) : tab === "connections" ? (
        <>
          <ConnectionsPanel rows={connections.data ?? []} loading={connections.isLoading}
            feedback={connections.error && <Alert severity="error" action={<Button onClick={() => void connections.refetch()} disabled={connections.isFetching}>重试</Button>}>{errorMessage(connections.error)}</Alert>}
            stale={connections.isError} grouped={grouped} onGroupedChange={setGrouped}
            collapsed={collapsed} onCollapse={(key) => setCollapsed((previous) => {
              const next = new Set(previous);
              if (next.has(key)) next.delete(key); else next.add(key);
              return next;
            })} />
        </>
      ) : (
        <LogsPanel text={logs.data ?? ""} loading={logs.isLoading} fetching={logs.isFetching}
          error={logs.error ? errorMessage(logs.error) : undefined}
          paused={logsPaused} onPausedChange={setLogsPaused} onRefresh={() => void logs.refetch()} />
      )}
      </Box>
    </Stack>
  );
}

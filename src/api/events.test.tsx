import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen } from "@testing-library/react";
import type { TrafficTick } from "../bindings";

const handlers = vi.hoisted(() => new Map<string, (event: { payload: unknown }) => void>());
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
    handlers.set(name, handler);
    return () => handlers.delete(name);
  }),
}));

import { useRuntimeEvents } from "./events";
import { useTraffic, useTrafficHistory } from "./client";
import { resetTrafficStore } from "./trafficStore";

function Probe() {
  useRuntimeEvents();
  const traffic = useTraffic();
  const history = useTrafficHistory();
  return <output data-testid="traffic">{JSON.stringify({ traffic, history })}</output>;
}

function emit(name: string, payload: TrafficTick) {
  act(() => handlers.get(name)!({ payload }));
}

function snapshot() {
  return JSON.parse(screen.getByTestId("traffic").textContent!);
}

describe("traffic event synchronization", () => {
  beforeEach(() => { handlers.clear(); resetTrafficStore(); });

  it("applies clear/stop corrections immediately and resumes history only on real samples", async () => {
    render(<QueryClientProvider client={new QueryClient()}><Probe /></QueryClientProvider>);
    const running: TrafficTick = {
      total_rate: { up: 5, down: 7 }, per_listener_rate: { l1: { up: 5, down: 7 } },
      totals: { total: { up: 100, down: 200 }, per_listener: { l1: { up: 100, down: 200 } } },
    };
    emit("traffic", running);
    const cleared = { ...running, totals: { total: { up: 0, down: 0 }, per_listener: {} } };
    emit("traffic-snapshot", cleared);
    expect(snapshot()).toEqual({ traffic: cleared, history: { up: [5], down: [7] } });
    const stopped = { ...cleared, total_rate: { up: 0, down: 0 }, per_listener_rate: {} };
    emit("traffic-snapshot", stopped);
    expect(snapshot()).toEqual({ traffic: stopped, history: { up: [5], down: [7] } });
    emit("traffic", { ...cleared, total_rate: { up: 1, down: 2 } });
    expect(snapshot().history).toEqual({ up: [5, 1], down: [7, 2] });
    // Let pending registrations settle before React runs effect cleanup.
    await act(async () => {});
  });
});

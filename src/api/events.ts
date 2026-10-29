import { useQueryClient } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect } from "react";
import type { Flow, ProcRouteStatus, RuntimeStatus, TrafficTick } from "../bindings";
import { setFlows } from "./procroute";
import { setTraffic } from "./trafficStore";

/** Subscribes to Rust-side events. Call once, from App. */
export function useRuntimeEvents(): void {
  const qc = useQueryClient();
  useEffect(() => {
    let disposed = false;
    const unlistens: UnlistenFn[] = [];
    const reg = (p: Promise<UnlistenFn>) =>
      p
        .then((u) => (disposed ? u() : unlistens.push(u)))
        .catch((e) => console.error("event subscription failed", e));

    reg(
      listen<RuntimeStatus>("runtime-status", (e) => {
        qc.setQueryData(["status"], e.payload);
        // Background subscription updates change the model without a model event.
        void qc.invalidateQueries({ queryKey: ["model"] });
      }),
    );
    reg(listen<TrafficTick>("traffic", (e) => setTraffic(e.payload)));
    reg(listen<TrafficTick>("traffic-snapshot", (e) => setTraffic(e.payload, false)));
    reg(listen<ProcRouteStatus>("procroute-status", (e) => qc.setQueryData(["procroute"], e.payload)));
    reg(listen<Flow[]>("procroute-flows", (e) => setFlows(e.payload)));

    return () => {
      disposed = true;
      unlistens.forEach((u) => u());
    };
  }, [qc]);
}

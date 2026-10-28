import { useQuery } from "@tanstack/react-query";
import { useSyncExternalStore } from "react";
import { commands, type Flow } from "../bindings";
import { make } from "./client";

export function useProcRouteStatus() {
  return useQuery({ queryKey: ["procroute"], queryFn: () => commands.procrouteStatus() });
}

export function useRunningProcesses() {
  return useQuery({ queryKey: ["runningProcesses"], queryFn: () => commands.listRunningProcesses() });
}

export const useSaveProcessRule = make(() => commands.saveProcessRule);
export const useDeleteProcessRule = make(() => commands.deleteProcessRule);
export const useSaveProcessRoutingSettings = make(() => commands.saveProcessRoutingSettings);
// 这三个会弹 UAC；被拒绝时返回的错误文本（如"已取消"）由通用的错误提示显示。
export const useProcRouteInstall = make(() => commands.procrouteInstall);
export const useProcRouteUninstall = make(() => commands.procrouteUninstall);
export const useProcRouteCleanup = make(() => commands.procrouteCleanup);

// 服务每秒推送的连接列表（"procroute-flows" 事件），同 trafficStore 的做法。
const NO_FLOWS: Flow[] = [];
let flows: Flow[] = NO_FLOWS;
const subs = new Set<() => void>();

export function setFlows(f: Flow[]): void {
  flows = f;
  subs.forEach((s) => s());
}

export function getFlows(): Flow[] {
  return flows;
}

export function subscribeFlows(f: () => void): () => void {
  subs.add(f);
  return () => {
    subs.delete(f);
  };
}

export function useFlows(): Flow[] {
  return useSyncExternalStore(subscribeFlows, getFlows);
}

/** Test helper: forget all flows. */
export function resetFlowsStore(): void {
  flows = NO_FLOWS;
}

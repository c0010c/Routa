import { useQuery } from "@tanstack/react-query";
import { commands, type TailnetPeer } from "../bindings";

/**
 * Devices of the system Tailscale client's tailnet. `error` says why there is
 * no list (no client installed, not logged in, ...); the UI then falls back to
 * typing the exit node by hand.
 */
export function useTailnetPeers(enabled = true) {
  const q = useQuery({
    queryKey: ["tailnet"],
    queryFn: async () => {
      const r = await commands.tailnetPeers();
      return r.status === "ok" ? { peers: r.data, error: null } : { peers: null, error: r.error };
    },
    enabled,
    staleTime: 15_000,
  });
  return { peers: q.data?.peers ?? null, error: q.data?.error ?? null, loading: q.isLoading, refetch: q.refetch };
}

const same = (a: string, b: string) => a.trim().toLowerCase() === b.trim().toLowerCase();

/**
 * The instance itself among the system client's peers (by hostname; an online
 * machine wins over a stale offline one). `null` when the system client is on
 * another tailnet or has not seen the instance yet.
 */
export function findSelf(peers: TailnetPeer[] | null, hostname: string): TailnetPeer | null {
  const hits = (peers ?? []).filter((p) => same(p.host_name, hostname));
  return hits.find((p) => p.online) ?? hits[0] ?? null;
}

/** Approved exit nodes, online ones first. */
export function exitCandidates(peers: TailnetPeer[] | null): TailnetPeer[] {
  return (peers ?? []).filter((p) => p.exit_node_option && p.ips.length > 0).sort((a, b) => Number(b.online) - Number(a.online));
}

/** The peer an `exit-node` value (IP, hostname or MagicDNS name) points at. */
export function peerFor(peers: TailnetPeer[] | null, value: string): TailnetPeer | null {
  const v = value.trim();
  if (v === "") return null;
  return (peers ?? []).find((p) => p.ips.includes(v) || same(p.host_name, v) || same(p.dns_name, v) || same(p.dns_name.split(".")[0], v)) ?? null;
}

/** Display text of an `exit-node` value: `名称 · IP` when the device is known. */
export function exitNodeText(peers: TailnetPeer[] | null, value: string): string {
  if (value.trim() === "") return "不使用（只访问 tailnet 内设备）";
  const p = peerFor(peers, value);
  return p ? `${p.host_name} · ${p.ips[0] ?? value}` : value;
}

/** `auto:*` values that mihomo v1.19.33 never resolves to an exit node. */
export const isAutoExitNode = (v: string) => v.trim().toLowerCase().startsWith("auto:");

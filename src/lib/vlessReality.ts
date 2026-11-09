import type { JsonValue, ManualNode } from "../bindings";

type Raw = Partial<{ [key in string]: JsonValue }>;

const isObj = (v: JsonValue | undefined): v is Raw => typeof v === "object" && v !== null && !Array.isArray(v);

/** A mihomo proxy the VLESS + REALITY form can show without losing meaning. */
export function isRealityVision(raw: Raw | undefined): boolean {
  if (!raw || raw.type !== "vless" || !isObj(raw["reality-opts"])) return false;
  return raw.network === undefined || raw.network === "tcp";
}

/**
 * `client-fingerprint` values mihomo knows (component/tls/utls.go, v1.19.33). Anything else
 * only logs "wrong clientFingerprint" and silently drops uTLS. The first nine are the common ones.
 */
export const MIHOMO_FINGERPRINTS = [
  "chrome",
  "firefox",
  "safari",
  "ios",
  "android",
  "edge",
  "360",
  "qq",
  "random",
  "chrome120",
  "firefox120",
  "safari16",
  "chrome_psk",
  "chrome_psk_shuffle",
  "chrome_padding_psk_shuffle",
  "chrome_pq",
  "chrome_pq_psk",
  "randomized",
];

export const isSupportedFingerprint = (fp: string) => MIHOMO_FINGERPRINTS.includes(fp);

export function publicKeyError(s: string): string | undefined {
  const v = s.trim();
  if (v === "" || /^[A-Za-z0-9_-]{43}$/.test(v)) return undefined;
  return "应为 43 位字符（xray x25519 输出的 Public key）";
}

export function shortIdError(s: string): string | undefined {
  const v = s.trim();
  if (v === "" || (/^[0-9a-fA-F]+$/.test(v) && v.length % 2 === 0 && v.length <= 16)) return undefined;
  return "只能是十六进制，长度为偶数且不超过 16 位";
}

/** `host:port` or `[v6]:port` pasted into the server field. */
export function splitHostPort(s: string): { host: string; port: string } | null {
  const m = /^(?:\[([0-9a-fA-F:.]+)\]|([^\s:/[\]]+)):(\d{1,5})$/.exec(s.trim());
  return m ? { host: m[1] ?? m[2], port: m[3] } : null;
}

/** Another manual node with the same server, port and uuid. */
export function findDuplicateNode(
  nodes: ManualNode[],
  selfId: string,
  server: string,
  port: number,
  uuid: string,
): ManualNode | undefined {
  return nodes.find((n) => {
    if (n.id === selfId || !("Proxy" in n.kind) || !isObj(n.kind.Proxy.raw)) return false;
    const r = n.kind.Proxy.raw;
    return r.server === server && Number(r.port) === port && r.uuid === uuid;
  });
}

import type { NetInterface } from "../bindings";

/** Usable = up and holding at least one non-link-local address (same rule as the backend). */
export function isInterfaceUsable(i: NetInterface): boolean {
  const v4 = i.ipv4.some((a) => !a.startsWith("169.254."));
  const v6 = i.ipv6.some((a) => !a.toLowerCase().startsWith("fe80"));
  return i.is_up && (v4 || v6);
}

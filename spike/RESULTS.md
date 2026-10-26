# mihomo behavior spike (S1-S5)

Version: `Mihomo Meta v1.19.33 windows amd64 with go1.26.10 Wed Oct 21 16:40:02 UTC 2026` (official MetaCubeX/mihomo release v1.19.33, asset `mihomo-windows-amd64-v1.19.33.zip`; zip sha256 `6ab2180875f9f6fff8b19386d760daec257f66b45f72b3afc650634272fa837a` = the digest published on the GitHub release page, verified in the fix round by re-downloading the zip; the exe extracted from that verified zip has sha256 `1cde41cfffd3de0aedad63c0a8515e1ce5eea507e8133aed0f637d316c93800a`, identical to the exe used in all tests). Binary in `spike/bin/` (gitignored).
Host: Windows 11, no proxy nodes. Default route NIC = `vEthernet (haos)` (172.20.10.8). The machine also runs the user's own Clash Verge (`verge-mihomo.exe`); it was not touched.
Setup: all listeners/controller on 127.0.0.1, controller 19097 + secret, listeners 19101-19121, exits are `type: direct` proxies referenced by `listeners[].proxy`. Configs: `s1.yaml` (S1-S4), `s5a.yaml`, `s5b.yaml` (S5). `slowsrv.py` = local endless slow HTTP stream used as the long-lived connection. Test traffic: `curl -x http://127.0.0.1:<port> https://www.gstatic.com/generate_204`. All started processes were stopped afterwards.

Adapters on this host (Get-NetAdapter):

| Name | Description | Status | IPv4 |
|---|---|---|---|
| vEthernet (haos) | Hyper-V Virtual Ethernet Adapter #3 | Up (default route) | 172.20.10.8 |
| WLAN 2 | Intel Wi-Fi 6 AX200 | Up / Connected | none |
| 蓝牙网络连接 | Bluetooth PAN | Disconnected | 169.254.184.29 (link-local) |
| 以太网 3 | Intel I225-V | Not Present | - |
| Tailscale | Tailscale Tunnel | Up | 100.113.29.21 |

## S1 - which `interface-name` value works on Windows

Config: `{name: d-friendly, type: direct, interface-name: "vEthernet (haos)"}` etc. (see s1.yaml). Observed by curl through each listener + mihomo debug log.

| interface-name value | result |
|---|---|
| `vEthernet (haos)` (friendly name = Get-NetAdapter `Name`) | **works**, HTTP 204 |
| `{F9785699-...}` (InterfaceGuid) | `dial ... error: interface not found` |
| `Hyper-V Virtual Ethernet Adapter #3` (InterfaceDescription) | `interface not found` |
| `VETHERNET (HAOS)` (different case) | `interface not found` (match is case-sensitive) |

Evidence that the binding is really applied (not just default route): `interface-name: Tailscale` (Up, but no internet route) fails with `connectex: A socket operation was attempted to an unreachable network`, while the same destination succeeds via the default route.

**Conclusion S1:** use the exact Windows friendly name (`Name` / `InterfaceAlias`, case-sensitive; Chinese names fine as UTF-8 YAML). GUID and description do NOT work. If we store the GUID for robustness, resolve GUID -> current friendly name before writing YAML.

Extra finding: `WLAN 2` (Up and Connected) is also `interface not found`. Observed: `Get-NetIPAddress` (IPv4 and IPv6) returns **no addresses of any family** for WLAN 2 (re-checked in the fix round). The disconnected Bluetooth adapter, which has an IPv6 fe80:: and an IPv4 169.254.x address, was found by mihomo. **Working hypothesis (unproven):** mihomo only resolves adapters that have at least one IP address. An adapter with only IPv6 or only IPv4 was not tested (every adapter that was found had both families). Any NIC-usable filter based on 'has an IP' is a hypothesis; the safe signal is a real dial result / mihomo log.

## S2 - missing / disconnected interface (verified by proxy only)

Cannot unplug NICs or capture packets; tested with non-existent / disconnected names. No packet capture was done; "no fallback" is inferred from the dial error and from the request failing while the default route works, not from sniffing the default NIC.

| case | listener result | log |
|---|---|---|
| name does not exist (`NoSuchNIC`) | fails after ~3s (mihomo retries ~10 dials); curl `SSL_ERROR_SYSCALL` (tunnel closed) | `[TCP] dial d-bad ... error: interface not found` |
| adapter Not Present (`以太网 3`) | same | `interface not found` |
| adapter present, disconnected, link-local IP (`蓝牙网络连接`) | fails | `connectex: ... unreachable network` (bound, no route) |
| adapter up but no IP (`WLAN 2`) | fails | `interface not found` |

**Conclusion S2:** dial **fails**; it does NOT fall back to the default NIC (all 4 cases failed while default-route requests succeeded). Plan assumption holds. The client sees a closed tunnel after ~2-3s, not an HTTP error code; the reason is only visible in mihomo's log. Physical unplug test is deferred to the final acceptance task.

## S3 - `metadata.inboundName`

`GET /connections` while curls ran through `l-none` (19101) and `l-friendly` (19102):

```
"inboundIP":"127.0.0.1","inboundPort":"19102","inboundName":"l-friendly", ... "specialProxy":"d-friendly"  chains:["d-friendly"] rule:""
"inboundIP":"127.0.0.1","inboundPort":"19101","inboundName":"l-none",     ... "specialProxy":"d-none"      chains:["d-none"]
```

**Conclusion S3:** `metadata.inboundName` == listener `name`; `inboundPort` also present. Plan assumption holds. With `listeners[].proxy` set, `rule` is empty and `chains` holds the exit proxy/group name.

## S4 - `DELETE /connections/{id}`

Long-lived connection: `curl -x http://127.0.0.1:19101 http://127.0.0.1:19200/` (endless slow stream). DELETE at t=0; curl exited ~0.1s later:

```
DELETE http=204 t=0.0019
curl: transfer closed with 99999400 bytes remaining to read
```

The connection disappeared from `/connections`; other connections were unaffected.

**Conclusion S4:** works, immediate (204, client sees closure within ~100ms). Plan assumption holds. (Only plain HTTP via `direct` tested.)

## S5 - `PUT /configs?force=true` with `{"path":"<abs path>"}`

- Path must be inside mihomo's home dir (`-d`) or SAFE_PATHS, else **HTTP 400** `{"message":"path is not subpath of home directory or SAFE_PATHS: ... allowed paths: [...]"}`. Forward-slash Windows paths accepted.
- Add listener (`l-new` 19110): works (204), listening immediately.
- Remove listener (`l-guid` 19103): works, port closed.
- Change port (`l-none` 19101 -> 19111): works; old port closed, new serves. Only changed/new listeners were restarted (log shows only `l-new`, `l-none` re-listening); unchanged listeners kept running.
- Existing long connections through reloaded listeners survived the reload (still in `/connections` and streaming 40s later). Stale connections must be closed explicitly with DELETE.
- **Port occupied** (another process listening on 19120; config adds `l-occ` on 19120 and `l-ok` on 19121): **`PUT` returns `204` with NO error.** The failure is only in mihomo's log: `level=error msg="Listener l-occ listen err: listen tcp 127.0.0.1:19120: bind: Only one usage of each socket address (protocol/network address/port) is normally permitted."` The rest of the config was applied (l-ok listening, removals done); only `l-occ` is silently missing. There is no API to list listeners (`GET /listeners`, `/configs/listeners` -> 404; `GET /configs` has no listener list).
- Retry: after the occupying process exited, re-PUTting the same config started `l-occ` successfully.

**Conclusion S5:** add/remove/port-change apply via hot reload. **Plan assumption "returns an error when port occupied" is FALSE.**

## Port-conflict detection: what is authoritative (fix round)

Tested with an occupier process (python socket bound to 127.0.0.1:19120) and a PUT adding `l-occ` on 19120:
- **TCP connect cannot detect it.** Connecting to 19120 succeeds because the foreign occupier accepts, so a connect check gives a false positive in exactly the failing case.
- **No REST state for listeners.** `GET /listeners` and `/listener` return 404; `GET /configs` has no listener list. v1.19.33 does not expose listener state over the API.
- **Authoritative signal 1: the mihomo error line** `Listener <name> listen err: ...`, on stdout/stderr and via the REST log stream `GET /logs?level=error` (verified: received `{"type":"error","payload":"Listener l-occ listen err: listen tcp 127.0.0.1:19120: bind: Only one usage of each socket address ..."}`). The stream is live, not history, so it must be opened before the PUT.
- **Authoritative signal 2 (reliable post-check): `Get-NetTCPConnection -State Listen -LocalPort <p>`.** `OwningProcess` was mihomo's PID for listeners that started (19121, 19101, 19102, 19097) and the occupier's PID (not mihomo) for 19120. Comparing OwningProcess to our mihomo child PID (in Rust: GetExtendedTcpTable) is a reliable per-listener check that does not depend on log parsing.
- A pre-bind check by the tool is only a hint (race window between check and PUT); the post-check (OwningProcess and/or the log line) decides.

## S5 extra: changing a listener's port to an occupied one

`s5c.yaml` moved `l-none` from 19111 to the occupied 19120. PUT returned 204; the old port 19111 was closed (no process listens) and the new port was not bound (log: `Listener l-none listen err: ... bind`). **The old listener does NOT stay up; the listener is simply gone** until a later reload succeeds. There is no rollback.

## Impact on plan

1. **S5 contradicts the plan:** PUT /configs does not report port conflicts, and a failed port move drops the old listener as well. The tool must pre-check ports (hint only) and then verify each listener after PUT via `OwningProcess == mihomo PID` on its port (primary; TCP connect is NOT valid) plus the `Listener <name> listen err` log signal (stdout, or `/logs?level=error` opened before the PUT). A missing listener is an apply failure. Affects **Task 9** (MihomoApi.reload must not treat 204 as success; needs log capture and the port-owner check), **Task 10** (apply pipeline fail-closed: verify per listener, report which one failed, consider re-PUT of the last good config), **Task 13** (runtime orchestration: error surfacing; re-PUT after the port is freed recovers) and **Task 18** (acceptance: port-occupied scenario).
2. **S1 refines the plan:** `interface-name` must be the exact, case-sensitive friendly name; GUID and description do not work. Affects **Task 6** (NIC enumeration: `mihomo_name` = friendly name; `NetInterface.is_usable` must not rely on 'has an IP' alone, which is only an unproven hypothesis) and **Task 8** (config generator writes `interface-name` from the friendly name, resolving from a stored GUID if one is kept).
3. The generated config path must be inside mihomo's home dir (or SAFE_PATHS) or PUT returns 400. Affects **Tasks 8, 9, 10** (where the file is written and passed to `PUT /configs`).
4. Stale connections survive reloads; after removing/rebinding a listener or exit, close affected connections via `DELETE /connections/{id}` (filter by `metadata.inboundName`). Affects **Tasks 10, 13**.
5. A failed NIC is only visible as a closed tunnel plus a log line; 'NIC missing' health should be derived by the tool from the adapter list (**Tasks 6, 13**), not from proxy responses. Physical verification belongs to **Task 18**.
6. No contradiction: S2 (missing interface -> dial fails, no fallback; proxy-verified), S3 (inboundName == listener name), S4 (DELETE immediate).

Not verified: packet-level confirmation on a second NIC, physical cable unplug (deferred to final acceptance); UDP; real proxy nodes / proxy groups as listener exit; `interface-name` on non-direct proxy types; binding a friendly name to a non-default NIC that has internet access (only the default NIC and non-routable NICs were exercised); IPv6-only adapters.

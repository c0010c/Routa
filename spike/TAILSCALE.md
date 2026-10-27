# Tailscale spike (phase A: no login performed)

Binary: `spike/bin/mihomo-windows-amd64.exe` v1.19.33 (tsnet reports `IPNVersion 1.102.3-mihomo-v1.19.33`). Work dir `spike/work` (gitignored). Controller `127.0.0.1:19197` secret `spk-ts-1`, listeners 19201/19202. Configs: `ts1.yaml` (one node), `ts2.yaml` (two nodes). Every node uses `ephemeral: true`; no login, no auth-key was used. The control plane `controlplane.tailscale.com` IS reachable from this network (DNS via the LAN resolver, TCP 443 and 80 connect to 192.200.0.x in ~0.2s; IPv6 is unreachable on this host and fails fast, harmless).

## Key finding: the instance starts lazily

mihomo starts a tailscale node only on its **first dial** (`DialContext` -> `ensureStarted`). Loading the config, `mihomo -t`, `GET /proxies`, and `GET /proxies/<name>` do nothing: with no traffic there is no `[Tailscale]` log line, no `state-dir`, and no auth URL (verified: 40s idle run, log has no Tailscale lines, `spike/work/tailscale` not created). Triggers that worked: a request through a listener that uses the node (`curl -x http://127.0.0.1:19201 ...`) and `GET /proxies/ts-a/delay?url=...&timeout=8000`. The first dial then blocks until the backend leaves `NoState` and fails with `context deadline exceeded` (~5s, the listener dial timeout; delay test returned `{"message":"Timeout"}`), but the instance keeps running and the login URL is logged afterwards anyway.

## T1: how login works / auth URL in the log

Info-level run (curl through listener as trigger), verbatim:

```
time="2026-10-27T12:02:02.596774800+08:00" level=info msg="[Tailscale](ts-a) tsnet running state path C:\\E\\...\\spike\\work\\tailscale\\t1\\tailscaled.state"
time="2026-10-27T12:02:02.603611600+08:00" level=info msg="[Tailscale](ts-a) tsnet starting with hostname \"mihomo-spike-a\", varRoot \"C:\\\\E\\\\...\\\\spike\\\\work\\\\tailscale\\\\t1\""
time="2026-10-27T12:02:02.619244000+08:00" level=info msg="[Tailscale](ts-a) LocalBackend state is NeedsLogin; running StartLoginInteractive..."
time="2026-10-27T12:02:07.554157900+08:00" level=warning msg="[TCP] dial ts-a 127.0.0.1:1918 --> www.gstatic.com:443 error: context deadline exceeded"
time="2026-10-27T12:02:09.246573600+08:00" level=info msg="[Tailscale](ts-a) To start this tsnet server, restart with TS_AUTHKEY set, or go to: https://login.tailscale.com/a/148c30e001e974"
```

- The auth URL **is written to the info log** (it is the tsnet `UserLogf` line). Timing: 6-11s after the first dial (7s, 11s, 3-4s in other runs), i.e. the log line is not immediate; a tool must wait/poll for it.
- The wording "restart with TS_AUTHKEY set, or go to:" is tsnet's stock text; ignore the first half. mihomo itself does not expose the URL through the API.
- URL format: `https://login.tailscale.com/a/<12-14 hex chars>` (seen `148c30e001e974`, `98191d6015a37`, `792a63012f01`, `1e72955c018e69`, `7c44d52015757`). It is new for every run (fresh registration each start while not logged in).
- Regex (per instance, anchor on the node name): `\[Tailscale\]\((?P<name>[^)]+)\) To start this tsnet server, restart with TS_AUTHKEY set, or go to: (?P<url>https://\S+)`. A looser fallback: `https://login\.tailscale\.com/a/[0-9a-f]+`. The node name in parentheses is the mihomo proxy name, which is how multiple instances are told apart.
- State-change lines at info: only `LocalBackend state is NeedsLogin; running StartLoginInteractive...`. The transition to Running was not observed (needs login, phase B). The first line `tsnet running state path` is the "started" marker.
- Debug level (`log-level: debug`) also shows the URL, in extra lines: `control: AuthURL is https://login.tailscale.com/a/98191d6015a37`, `Received auth URL: https://login.tailsc...` (truncated, do not use that one), `Switching ipn state NoState -> NeedsLogin (WantRunning=true, nm=false)`, `control: RegisterReq: got response; nodeKeyExpired=false, machineAuthorized=false; authURL=true`, and the same info line as above. Debug is very noisy (WireGuard, DNS, magicsock); the info line is sufficient and the tool should not require debug.
- Auth URL for the user to open in phase B: **URLs are per run; phase B will start a fresh run.** Sample from this run (will be stale, do not use): `https://login.tailscale.com/a/7c44d52015757`.
- Side effects of a running node: creates `<state-dir>/tailscaled.state` and `tailscaled.log.conf` (relative `state-dir` resolves against the mihomo work dir `-d`; paths outside it are rejected by `IsSafePath`). Default `state-dir` is `tailscale` when empty.
- `mihomo -t -f ts1.yaml`: 0.3s, exit 0, "test is successful"; no state dir created, no network contact (lazy start), so config test is safe and cheap on tailscale configs.
- Auth-key path not tried (needs a key from the user).

## T3: login state through the API

`GET /proxies/ts-a` with `Authorization: Bearer spk-ts-1`, while waiting for login (before, during and after the dial timeout), returns only the generic fields:

```
{"alive":true,"dialer-proxy":"","extra":{},"history":[],"id":"...","interface":"","mptcp":false,"name":"ts-a","provider-name":"","routing-mark":0,"smux":false,"tfo":false,"type":"Tailscale","udp":false,"uot":false,"xudp":false}
```

Nothing indicates login state (`type` is `Tailscale`; `alive` is true before any test). After a failed delay test: `alive:false`, `history:[{"delay":0}]` - which just means "not reachable", not "needs login". `GET /proxies` shows the node under `ts-a` and in GLOBAL `all`, no extra fields. Conclusion: login state is NOT available through the REST API in v1.19.33; the tool must derive it from the log (auth-URL line present = needs login; no direct "logged in" line found yet, phase B) or from a successful delay test.

## T4: two instances

`ts2.yaml` (ts-a: hostname mihomo-spike-a, state-dir tailscale/t1; ts-b: mihomo-spike-b, tailscale/t2), one curl through each listener. Both started independently (their own `tsnet running state path`, own varRoot, own `NeedsLogin`), both created their own state dir, and both logged their own auth URL:

```
[Tailscale](ts-b) To start this tsnet server, restart with TS_AUTHKEY set, or go to: https://login.tailscale.com/a/792a63012f01
[Tailscale](ts-a) To start this tsnet server, restart with TS_AUTHKEY set, or go to: https://login.tailscale.com/a/1e72955c018e69
```

So two instances coexist at the pre-login stage (separate URLs, no port/state clash; each picks its own random UDP port). Whether both work after login (including two different devices on one tailnet) is untested.

## T6: interface binding (`interface-name`), pre-login control-plane connections

Same one-node config, curl through the listener to trigger startup, debug log + `netstat -ano` for the mihomo PID. This host: default-route NIC `vEthernet (haos)` = 192.168.50.188 (192.168.50.x now, not the 172.20.10.8 listed in RESULTS.md); `Tailscale` adapter = 100.113.29.21, Up, no internet route.

| node `interface-name` | control plane (TCP 443/80 to 192.200.0.x) | netstat (local address of the control conn) |
|---|---|---|
| `Tailscale` | **every dial fails**: v4 `connectex: A socket operation was attempted to an unreachable network`, v6 `The requested address is not valid in its context`; 0 successful dials, no auth URL, no connection ever ESTABLISHED | none (only LISTENING 19197/19201 and unbound UDP) |
| `"vEthernet (haos)"` | succeeds (2 ok dials), auth URL logged | `TCP 192.168.50.188:53534 -> 192.200.0.101:80 ESTABLISHED` |
| none | succeeds (2 ok dials), auth URL logged | `TCP 192.168.50.188:10887 -> 192.200.0.113:80 ESTABLISHED` |

Conclusions:
- Binding is applied to the **control-plane TCP** (tsnet `SystemDialer` goes through mihomo's dialer): the Tailscale NIC (no internet route) breaks it, the real NIC works, and the established connection's local address is the bound NIC's IP. Note `haos` and "none" are indistinguishable by local address here because the default route already is that NIC; the `Tailscale` failure is the proof that the option is honoured.
- With `Tailscale` binding the instance also never reaches a login URL, so a wrongly chosen NIC makes the node permanently stuck at the dial timeout; the symptom is only visible in the log (`SystemDialer ... unreachable network` at debug level, `context deadline exceeded` at warning level).
- UDP (WireGuard/DERP/magicsock) sockets come from `SystemPacketListener`, which also uses the mihomo dialer's `ListenPacket`; netstat shows them as unbound wildcards (`UDP 0.0.0.0:<port>`, `[::]:<port>`) with interface binding applied via socket options that netstat does not display. **Not proven** for the data plane since nothing was logged in: no WireGuard/DERP traffic exists before login. Needs phase B.

## Pending phase B: T2, T5 (need an authorized login)

- T2: after a login, restart mihomo with the same `state-dir`; does it come up Running without a new URL?
- T5: with a live long connection through the listener, `PUT /configs?force=true` changing only an unrelated listener port; does the connection survive, and how long does the tailscale re-handshake take? (Also check whether a reload re-creates the tsnet server and whether `ephemeral: true` nodes vanish from the tailnet on stop.)
- Also: the "logged in" log/status signal, the auth-key path, and data-plane interface binding (UDP) after login.

# ProxyBridge 上游源码

- 上游仓库：https://github.com/InterceptSuite/ProxyBridge
- 标签：`v4.3.0_Windows`
- 基线提交：`8cb9dceeed582fcd7c2baed8d20eec34934732fc`
- 来源目录：`Windows/src/`、`Windows/gui/res/pbver.inc`、`Windows/gui/res/resource.h`（原样拷贝，后两者仅因 `src/core.rc` 通过 `../gui/res/pbver.inc` 引用版本资源而需要）与仓库根目录 `LICENSE`

## 本目录补丁清单

后续任务对 `src/` 的修改在此追加（每项一行：文件 - 目的）。

- `src/pb_internal.h`、`src/ProxyBridge.c`、`src/ProxyBridge.h`、`src/pb_relay.c` - 中转端口可配置：`LOCAL_UDP_RELAY_PORT` 的全部使用点改为全局 `g_local_udp_relay_port`（TCP 已是上游的 `g_local_relay_port`），默认仍为 34010/34011；新增导出 `ProxyBridge_SetRelayPorts(tcp, udp)`，仅在 `ProxyBridge_Start` 前可调用，两端口须非 0 且不相等。
- `src/pb_rules.c` - `match_process_pattern` 非通配分支改为 UTF-8 → UTF-16 后 `CompareStringOrdinal(..., TRUE)`，完整 Unicode 不区分大小写（上游 `_stricmp` 仅折叠 ASCII）；通配分支 `wildcard_match` 未改，仍只折叠 ASCII。
- `src/pb_rules.c`、`src/pb_internal.h`、`src/ProxyBridge.c` - BLOCK 放行回环：新增 `pb_is_loopback_v4`（127.0.0.0/8）/`pb_is_loopback_v6`（`::1`、`::ffff:127.x.x.x`），在 IPv4/IPv6 的 TCP/UDP 共四处规则判定后，若动作为 BLOCK 且目标为回环则改按 DIRECT 处理。
- `tests/pbtest.h`（上游 `Windows/tests/pbtest.h` 原样拷贝）、`tests/test_routa_patches.c` - 上述补丁的单元测试，由 `scripts/test-proxybridge.ps1`（`pnpm test-proxybridge`）编译运行。

## 回环阻断核实

阅读上游 `ProxyBridge.c`：WinDivert 过滤器含 `loopback`，会捕获本机回环流量；BLOCK 分支直接丢包、无任何回环判断（上游只对 PROXY 动作在回环目标上改为 DIRECT）。因此上游对回环目标的 BLOCK 会阻断本地服务，需要上述补丁 3。

运行时实测（对 `curl.exe` 加 BLOCK 规则后访问 `http://127.0.0.1:<本地服务>`）需要管理员权限加载 WinDivert，尚未执行，待用户验收时实测确认。

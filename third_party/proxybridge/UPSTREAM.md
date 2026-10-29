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
- `src/pb_proxy.c`、`src/pb_rules.c` - 代理配置严格匹配：仅 ID 0 保留“第一份可用配置”的约定，具体 ID 不存在时拒绝转发；IPv4/IPv6 规则判定中缺失或无效的代理配置返回 Block，不隐式转为 Direct。
- `src/pb_conntrack.c`、`src/pb_internal.h`、`src/pb_rules.c`、`src/pb_proxy.c`、`src/pb_relay.c` - UDP 规则版本：规则增删改、启停、重排和代理配置变化后，旧版本 UDP 会话及 Direct 缓存不再参与转发；版本取自规则判定时刻，避免修改前的判断在修改后写入而复活。采用全局版本，变化后其他 UDP 会话也在下一包重新判定；不重置已有 TCP 会话。
- `src/pb_proxy.c`、`src/pb_internal.h`、`src/pb_relay.c`、`src/ProxyBridge.c` - 配置修改与 UDP 中转使用读写锁同步，避免删除配置时数组压缩让旧指针变成另一份配置；`select()` 等待期间释放锁，修改后丢弃旧 socket 集合；TCP 握手、规则校验和日志读取配置快照。快照采用非阻塞读锁，配置忙时按不可用处理，防止捕包线程等待中转网络 I/O；DNS 解析在写锁外进行。
- `tests/test_udp_policy.c`、`scripts/test-proxybridge.ps1` - 回归覆盖具体 ID 失效且另一代理仍存在、IPv4/IPv6 缺失配置不直连、UDP 会话及 Direct 缓存失效、延迟写入旧判断、切换代理目标、TCP 会话保留、配置删除与读者的并发同步和配置忙时保守处理；不加载驱动。

## 审查问题处理决定（2026-10-29，第一批）

编号对应审查聊天 `Review code changes` 中的全仓库审查结果。以下决定经用户确认；后续升级上游时应同时核对这些策略。

1. **失效代理配置回退到其他 Listener：修复。** 上游 `find_proxy_config()` 在具体 ID 不存在时回退到第一份配置；Routa 要求 Listener 停用、删除或未生效时阻断，因此改为严格匹配，并使旧 UDP 判断失效。仅底层 API 的 ID 0 保留默认配置含义，Routa 为代理规则使用具体 ID。缺失配置不能在调用方再变成 Direct。沿用现有回环例外。
2. **所有 `x.x.x.255` 被当作广播并绕过代理：保留现状。** 保持上游 `is_broadcast_or_multicast()` 的现有判断，不引入网卡前缀检测。已知影响：末段 `.255` 的正常 IPv4 单播也可能被当成广播，从 Proxy 改为 Direct，绕过指定出口；该保留决定不代表误判已经修复。
3. **UDP 中转入口来源校验：保持 ProxyBridge 上游行为。** 保留全网卡监听及按来源端口查找会话，不新增 UDP 来源地址校验，不把 TCP 的来源校验移植到 UDP。已知影响：其他主机能到达中转端口且使用匹配的来源端口时，仍存在借用已有会话注入数据的风险。问题 1 对旧版本会话的拒绝不改变这里的来源识别策略。

参考：[ProxyBridge 多代理回退说明](https://github.com/InterceptSuite/ProxyBridge/blob/8cb9dceeed582fcd7c2baed8d20eec34934732fc/Windows/README.md#multiple-proxy-configurations)、[广播识别源码](https://github.com/InterceptSuite/ProxyBridge/blob/8cb9dceeed582fcd7c2baed8d20eec34934732fc/Windows/src/pb_rules.c)、[UDP/TCP 中转源码](https://github.com/InterceptSuite/ProxyBridge/blob/8cb9dceeed582fcd7c2baed8d20eec34934732fc/Windows/src/pb_relay.c)、[Proxifier 显式故障转移设置](https://www.proxifier.com/docs/win-v4/redundancy.html)。Proxifier Windows v4 文档描述 TCP 代理，仅借鉴其显式回退策略，不据此推断其 UDP 实现。

驱动验收：保持 UDP 持续发送，分别切换 Proxy→Block、Proxy→Direct、Proxy A→Proxy B，并在 A 被删除而 B 存在时确认不借用 B；覆盖 IPv4/IPv6。无驱动单元测试不能替代这些实测。

验证记录（2026-10-29）：`scripts/test-proxybridge.ps1` 通过，`test_routa_patches` 19 项检查、`test_udp_policy` 246 项检查。测试使用 MSVC 编译全部 C 源码，不加载 WinDivert 驱动；上述真实网络切换场景尚未实测。

## 回环阻断核实

阅读上游 `ProxyBridge.c`：WinDivert 过滤器含 `loopback`，会捕获本机回环流量；BLOCK 分支直接丢包、无任何回环判断（上游只对 PROXY 动作在回环目标上改为 DIRECT）。因此上游对回环目标的 BLOCK 会阻断本地服务，需要上述补丁 3。

运行时实测（对 `curl.exe` 加 BLOCK 规则后访问 `http://127.0.0.1:<本地服务>`）需要管理员权限加载 WinDivert，尚未执行，待用户验收时实测确认。

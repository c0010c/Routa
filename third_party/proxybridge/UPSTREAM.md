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
- `src/pb_proxy.c`、`src/pb_internal.h`、`src/pb_relay.c`、`src/ProxyBridge.c` - 第一批修复引入配置读写同步，防止数组压缩把旧指针变成另一份配置；第二批 UDP 改用会话私有的配置副本，不再跨网络 I/O 持有全局配置锁。TCP 握手、规则校验和日志同样读取快照；快照采用非阻塞读锁，配置忙时按不可用处理，DNS 解析在写锁外进行。
- `tests/test_udp_policy.c`、`scripts/test-proxybridge.ps1` - 回归覆盖具体 ID 失效且另一代理仍存在、IPv4/IPv6 缺失配置不直连、UDP 会话及 Direct 缓存失效、延迟写入旧判断、切换代理目标、TCP 会话保留、配置删除与读者的并发同步和配置忙时保守处理；不加载驱动。
- `src/pb_udp.c`、`src/pb_relay.c`、`src/pb_conntrack.c`、`src/pb_internal.h`、`src/ProxyBridge.c`、`src/pb_socks5.c` - UDP 每会话独立 SOCKS5 关联、唯一会话标识及回包归属检查；用 WSAPoll 代替受 fd_set 大小限制的多路等待；上游端点使用独立短锁索引，避免自身 UDP 被再次拦截。失效或空闲超过 20 秒的关联释放，控制连接断开后下一包按需重建；最多 1024 个关联，满额时丢弃新请求，不挪用其他会话。
- `src/ProxyBridge.c`、`src/pb_udp.c`、`src-tauri/procsvc/src/lazy.rs` - UDP 中转随引擎启动，不依赖已有 SOCKS5 配置；IPv4/IPv6 均绑定成功才报告启动成功，端口独占，失败时清理；停止和失败路径等待旧线程退出后才释放内存、端口，支持再次启动。
- `tests/test_udp_relay.c`、`scripts/test-proxybridge.ps1`、`scripts/build-proxybridge.ps1` - 新增 UDP 模块到正式构建和测试构建；使用本机模拟 SOCKS5 服务、实际 TCP/UDP socket 验证同配置/不同配置、IPv4/IPv6、共用上游 UDP 端口、乱序回包、已识别会话重建后的迟到回包、规则版本失效、70 路会话、重连和启停；不加载驱动、不访问外网。

## 审查问题处理决定（2026-10-29，第一批）

编号对应审查聊天 `Review code changes` 中的全仓库审查结果。以下决定经用户确认；后续升级上游时应同时核对这些策略。

1. **失效代理配置回退到其他 Listener：修复。** 上游 `find_proxy_config()` 在具体 ID 不存在时回退到第一份配置；Routa 要求 Listener 停用、删除或未生效时阻断，因此改为严格匹配，并使旧 UDP 判断失效。仅底层 API 的 ID 0 保留默认配置含义，Routa 为代理规则使用具体 ID。缺失配置不能在调用方再变成 Direct。沿用现有回环例外。
2. **所有 `x.x.x.255` 被当作广播并绕过代理：保留现状。** 保持上游 `is_broadcast_or_multicast()` 的现有判断，不引入网卡前缀检测。已知影响：末段 `.255` 的正常 IPv4 单播也可能被当成广播，从 Proxy 改为 Direct，绕过指定出口；该保留决定不代表误判已经修复。
3. **UDP 中转入口来源校验：保持 ProxyBridge 上游行为。** 保留全网卡监听及按来源端口查找会话，不新增 UDP 来源地址校验，不把 TCP 的来源校验移植到 UDP。已知影响：其他主机能到达中转端口且使用匹配的来源端口时，仍存在借用已有会话注入数据的风险。问题 1 对旧版本会话的拒绝不改变这里的来源识别策略。

参考：[ProxyBridge 多代理回退说明](https://github.com/InterceptSuite/ProxyBridge/blob/8cb9dceeed582fcd7c2baed8d20eec34934732fc/Windows/README.md#multiple-proxy-configurations)、[广播识别源码](https://github.com/InterceptSuite/ProxyBridge/blob/8cb9dceeed582fcd7c2baed8d20eec34934732fc/Windows/src/pb_rules.c)、[UDP/TCP 中转源码](https://github.com/InterceptSuite/ProxyBridge/blob/8cb9dceeed582fcd7c2baed8d20eec34934732fc/Windows/src/pb_relay.c)、[Proxifier 显式故障转移设置](https://www.proxifier.com/docs/win-v4/redundancy.html)。Proxifier Windows v4 文档描述 TCP 代理，仅借鉴其显式回退策略，不据此推断其 UDP 实现。

驱动验收：保持 UDP 持续发送，分别切换 Proxy→Block、Proxy→Direct、Proxy A→Proxy B，并在 A 被删除而 B 存在时确认不借用 B；覆盖 IPv4/IPv6。无驱动单元测试不能替代这些实测。

验证记录（2026-10-29）：`scripts/test-proxybridge.ps1` 通过，`test_routa_patches` 19 项检查、`test_udp_policy` 246 项检查。测试使用 MSVC 编译全部 C 源码，不加载 WinDivert 驱动；上述真实网络切换场景尚未实测。

## 审查问题处理决定（2026-10-29，第二批）

4. **UDP 回包送错程序：按完整方案修复。** 每个已跟踪的本地 UDP 会话持有独立 SOCKS5 控制连接和 UDP socket，回包直接归属该会话，不再全局按远端地址和“最近活跃”猜测。即使多个会话使用同一配置、SOCKS5 服务返回同一 UDP 端口，也分别处理。连接表新建/替换条目、目标变化时分配新的会话 ID，结合第一批规则版本检查拒绝旧关联回包。沿用上游按本地端口和地址族识别连接的范围；未新增驱动 socket 生命周期事件，因此未被连接表识别的快速端口复用仍受原有空闲重判机制约束。
5. **服务恢复后漏启 UDP：修复。** 引擎启动时始终准备 UDP，中转端口绑定失败直接报告失败；配置随后从 Block/Direct 恢复为 Proxy 时无需重启引擎。没有代理流量时不建立 SOCKS5 关联，只保留本地监听和等待线程。要求 IPv4、IPv6 本地监听均可绑定；系统禁用 IPv6 或相应端口占用时启动失败。

6. **节点切换失败却报告应用成功：已按简化方案修复（用户确认）。** `src-tauri/src/core/apply.rs` 的策略组选择只要失败，就停止后续选择并进入现有失败关闭流程：重载空配置，清理新旧 Listener 的连接；空配置也不能应用时停止内核。错误信息包含失败的策略组、目标成员和原因。新配置已重载，不能伪称保留旧配置；只有全部选择成功后才记录新的已应用计划。不改配置生成，不新增选择重试或读回验证；现有运行时对应用失败的恢复机制保留。

验收重点：两个程序交错访问同一 UDP 服务、同配置和不同配置、IPv4/IPv6、服务以 Block/Direct 规则启动后恢复 Proxy、规则变化时的迟到回包、反复启动停止。下面的本机 socket 测试不替代 WinDivert 驱动、真实游戏/语音、VPN 网卡和服务崩溃恢复验收。

验证记录（2026-10-29）：`scripts/test-proxybridge.ps1` 通过，原有补丁 19 项、UDP 策略 246 项、本机 SOCKS5/UDP 集成测试 425 项，共 690 项检查；正式启动路径的端口冲突测试在加载驱动前失败退出。未替换正在运行的服务或其 DLL。

问题 6 补充验证记录（2026-10-29）：`cargo test --manifest-path src-tauri/Cargo.toml --lib core:: -- --nocapture` 通过 49 项、按原设置跳过 3 项（真实端口归属和真实 mihomo 二进制测试）；`cargo test --manifest-path src-tauri/Cargo.toml --lib runtime::` 通过 35 项。新增 4 项覆盖部分策略组切换成功后失败并停止后续选择、首次应用切换超时、空配置失败后停核、连接清理失败上报 `uncut`；已有成功选择和运行时失败恢复、未清理连接触发重启测试也通过。测试命令仅在当前进程设置 `TAURI_CONFIG={"bundle":{"externalBin":[],"resources":[]}}`，绕过运行中二进制的复制占用；未修改正式 Tauri 配置，未停止或替换运行中的应用。以上为模拟 API 的回归验证，未向运行中的 mihomo 注入节点切换故障。

问题 6 验证（2026-10-29）：应用流程 24 项测试通过，其中新增 4 项覆盖部分策略组成功后失败、首次应用的选择超时、空配置也失败时停止内核，以及旧连接无法清理时保留升级处理信号。Rust 主程序全部库测试 292 项通过、5 项按原设置忽略。测试命令临时使用 `TAURI_CONFIG` 跳过被运行中内核占用的打包文件复制，未改项目打包配置。

## 审查问题处理决定（2026-10-29，第三批）

7. **规则防护状态来自期望、并非执行结果：修复。** 服务返回逐条规则的确认动作和原因；客户端只展示与当前设置对应的服务结果。应用期间显示“应用中”，请求被拒绝显示“应用失败”，组件不可用、连接断开或阻断执行也无法确认时显示“状态未知”。代理应用失败但确认强制阻断成功时显示“已阻断”并说明失败原因。断开旧连接失败单独提示，保留已确认的代理动作。服务记录的保守 Block 不作为阻断成功的证据；无法确认的状态跨普通同步保留到操作真正成功。返回数据不含代理认证信息。
8. **部分失败后无法恢复：改为用户手动重试。** 按用户要求，同一份配置的部分失败、整体拒绝或应用请求超时均不自动重发；定时只查询状态，重新连接也不重试这份失败配置。错误提示提供“重试”，每次点击按最新设置尝试一次，成功才清除错误，持续失败继续展示。重试使用独立请求，允许重新尝试上次失败的 IPv4 TCP 断连；普通同步不反复断连。用户改变配置仍会下发新设置；此前成功配置在服务重连后的正常同步保留。
9. **更换代理目标不触发断开旧连接：保留现状（用户确认）。** 切换 Listener / 代理地址、端口或认证信息仍属于 Proxy，不算动作种类切换，即使“断开旧连接”开启，也不主动断开旧连接。保持已有 `proxy_target_change_edits_config_without_reset` 行为及测试；相应更新界面说明、README 和验收清单。

协议变更：主程序与分流服务的管道协议升至版本 2，新增逐条执行结果和手动重试请求。旧服务握手时显示“需要更新”，不混用新旧协议。使用此版本主程序前须配套更新分流组件；本次未安装或替换运行中的组件。

验证记录（2026-10-29）：临时设置 `TAURI_CONFIG={"bundle":{"externalBin":[],"resources":[]}}` 后运行 `cargo test --manifest-path src-tauri/Cargo.toml --workspace --lib --bins --quiet`，主程序 294 项、服务 93 项、共享协议 4 项通过，共 391 项，12 项按原设置忽略。覆盖确认阻断与未知状态、同一失败配置不自动重发、断线重连后仍等待用户重试、使用最新配置、断连失败的独立手动重试、服务端结果回传和旧协议拒绝。没有加载真实驱动注入故障，实际阻断及断连效果仍需人工验收。 前端 `node node_modules/vitest/vitest.mjs run --exclude '**/.claude/**'` 的 26 个测试文件、271 项全部通过，`node node_modules/typescript/bin/tsc --noEmit` 通过；新增覆盖状态未知、应用中、失败显示和必须点击才触发重试。

## 审查问题处理决定（2026-10-29，第四批）

10. **进程分流仅允许 IPv4 入口（用户确认）。** 保持当前 ProxyBridge Windows 底层连接代理服务器的能力，不扩展 IPv6 上游 TCP/UDP 连接。引用 IPv6 或其他不支持的监听地址的分流规则，在保存规则及修改所引用 Listener 时明确报错；旧配置在运行时阻断并显示“仅支持 IPv4 入口”，不把 `::` / `::1` 改写为 `127.0.0.1`，不自动选择其他入口。保留 IPv4 通配监听与现有空值 / localhost 到 IPv4 回环的入口约定。此限制只针对进程分流连接 Listener 的一段，不禁止其他用途的 IPv6 Listener，也不禁止经 IPv4 代理入口访问 IPv6 目标。
11. **更换内核路径后的禁止分流程序复查：不处理（用户确认）。** 用户决定产品不允许手动更换内核，因此本项不增加设置变更时的禁止程序校验，也不自动修改已有分流规则。本批只记录该产品决策，尚未移除现有自定义内核路径界面或后端能力。
12. **控制端口冲突提示：修复。** 设置页将后端端口校验错误展示在控制端口字段，优先显示本地格式错误；冲突提示说明与已有 Listener 重复，可识别时展示名称。编辑端口后清除旧字段错误，由用户手动再次保存；不自动改端口或重试。

验证记录（2026-10-29）：临时设置 `TAURI_CONFIG={"bundle":{"externalBin":[],"resources":[]}}` 后，主程序 `cargo test --manifest-path src-tauri/Cargo.toml --lib --quiet` 为 295 项通过、5 项按原设置忽略；覆盖分流引用 IPv6 入口的校验拒绝、旧配置阻断及原因、非分流用途的 IPv6 Listener 不受限制。前端 Settings、ProcessRuleDialog、ListenerDialog、validation 共 4 个文件、34 项测试通过，TypeScript 检查通过；覆盖端口冲突字段提示、停用 Listener 名称、编辑后清错与手动再次提交、IPv6 入口错误定位。接口错误类型通过 Specta 导出同步；临时导出源码已移除。未修改 ProxyBridge C 代码，未重跑 C 测试，未加载驱动或替换运行中的应用 / 服务；真实驱动场景仍按 ACCEPTANCE.md 人工验收。全部改动未提交。

## 审查问题处理决定（2026-10-29，第五批）

13. **设置保存成功提示与应用结果不一致：修复。** 设置页读取应用结果，仅在无附带警告的成功应用后显示“设置已保存并生效”；应用失败或部分监听器未启动时保留对应提示，不再根据字段变化推断“正在重启”。公共提示使用单一状态，避免成功和错误提示重叠。配置已保存但未生效时仍更新表单保存状态，不回滚保存结果；保存失败及配置被拒绝时保留未保存状态。面向用户只说明结果、影响和可操作的下一步，不展示底层错误原文；详细应用错误保留在运行日志中。未新增自动重试。
14. **停止 / 清零后流量缓存未同步：修复。** 后端在停止内核（包括崩溃清理、重启切换）时发布速率为零、累计量保留的快照；清零时发布累计量归零、当前速率保留的快照。前端同步独立流量缓存，快照不追加历史采样点；停止时保留曲线并暂停更新，清零不清除曲线，恢复采样后继续记录。流量请求带内部代次，停止、切换内核或清零之前发出的旧请求即使稍后返回，也不能覆盖新快照或改变统计基线。未新增自动重试。

验证记录（2026-10-29）：主程序 `cargo test --manifest-path src-tauri/Cargo.toml --lib --quiet` 为 299 项通过、5 项按原设置忽略，其中运行时 39 项通过，新增 4 项覆盖停止归零速率并保留累计量、运行中清零并保留速率与统计基线、停止后清零及旧内核响应丢弃、崩溃停核同步快照。测试进程临时使用 `TAURI_CONFIG={"bundle":{"externalBin":[],"resources":[]}}`，未改正式打包配置。前端 `node node_modules/vitest/vitest.mjs run --exclude '**/.claude/**'` 的 27 个文件、286 项全部通过，TypeScript 检查及 `git diff --check` 通过；覆盖各应用结果的简短提示、不叠加成功提示、不自动重试、流量快照即时更新且不改变历史曲线。未进行真实运行中的应用验收，未加载驱动、替换应用或服务；全部改动未提交。

## 重连退出策略与 UDP 握手修复（2026-10-29）

- 主程序与服务协议升至版本 3，增加独立的 `Attach` 请求。每次连接在同步前登记管理进程；登记不应用规则、不保存期望配置、不重试失败操作，也不清除客户端原有错误。正常退出、再次断连和 Windows 注销继续按已接受配置处理，保留失败配置须手动重试的决定。
- `src/pb_udp.c` 的 SOCKS5 UDP 握手改为 `WSAPoll` 驱动的非阻塞状态机，支持部分读写、无认证及用户名密码认证、IPv4 中转端点和 `0.0.0.0` 地址回复。配置发布前已解析 IPv4 地址，轮询线程不执行 DNS 或阻塞握手。握手共用 8 秒绝对截止时间；每会话最多暂存 4 个数据包，全局待发队列含分配开销最多 4 MiB。超额丢弃，不回落直连；失效、超时、认证失败和停止均释放待发数据及 socket。
- 本机 socket 回归新增 6 路沉默握手与正常流量并行、握手超时、响应分段及认证拒绝、会话和策略失效、握手期间停止与重新启动。实际 WinDivert 驱动、VPN 网卡、游戏/语音仍需按验收清单检查。
- 验证：临时使用 `TAURI_CONFIG={"bundle":{"externalBin":[],"resources":[]}}` 跳过被运行程序占用的打包文件复制，运行 `cargo test --workspace --lib --bins --quiet`：主程序 300 项、服务 95 项、协议 5 项通过，共 400 项，12 项按原设置忽略。`npm run test-proxybridge`：补丁 19 项、UDP 策略 246 项、UDP socket 集成 600 项，共 865 项通过；包含首包保留、每会话 4 包上限、IPv4 通配中转地址回复。`npm run typecheck` 和 `git diff --check` 通过。未替换运行中的应用、服务或 DLL，未加载驱动注入故障。

## 回环阻断核实

阅读上游 `ProxyBridge.c`：WinDivert 过滤器含 `loopback`，会捕获本机回环流量；BLOCK 分支直接丢包、无任何回环判断（上游只对 PROXY 动作在回环目标上改为 DIRECT）。因此上游对回环目标的 BLOCK 会阻断本地服务，需要上述补丁 3。

运行时实测（对 `curl.exe` 加 BLOCK 规则后访问 `http://127.0.0.1:<本地服务>`）需要管理员权限加载 WinDivert，尚未执行，待用户验收时实测确认。

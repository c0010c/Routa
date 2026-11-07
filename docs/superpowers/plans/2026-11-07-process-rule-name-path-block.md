# 进程分流：进程名与路径并存、按进程名阻断、取消全局「退出 Routa 后」 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 进程分流规则的每个进程条目带显式类型（进程名 / 路径），同一程序可两者并存、路径优先；进程名条目也能阻断，分流服务自动记录程序位置，服务停止后由 WFP 继续阻断；取消全局「退出 Routa 后」，所有不可用场景按规则自己的「Routa 或组件不可用时」处理。

**Architecture:** 协议 `RuleSpec` 增加 `kind` 和 `guard_paths`，`DesiredState` 去掉 `on_routa_exit`：进程名条目照常下发成 ProxyBridge 规则（按名称拦截），它的已知位置只进 WFP 守护。**位置学习在分流服务里**：注册 ProxyBridge 连接回调，新进程联网时按 PID 取完整路径，立即加入守护、持久化，并经 `GetStatus` 回报 Routa；Routa 存进模型 `ProcessRule.learned`。保存时的运行进程和商店应用更新事件（需要用户会话）由 Routa 补充。代理地址相同（即「节点 + 网卡」相同）的条目共用一个 ProxyBridge 代理配置。托盘与进程分流页新增「停用进程分流（并退出）」，托盘退出时对阻断规则弹一次确认。

**Tech Stack:** Rust（Tauri 2、serde、specta、windows-sys 0.61、windows 0.62 已在依赖树中、windows-service）、ProxyBridge C DLL（FFI）、React 19 + MUI 9 + TanStack Query、vitest。

**Spec:** `docs/superpowers/specs/2026-11-07-process-rule-name-path-block-design.md`（界面以原型 `spike/process-rule-params.tsx` 为准，选项名与文案以 spec 为准）

**Branch:** 当前 worktree 分支 `claude/process-routing-rule-params-a0e6c7`。`docs/` 被全局 gitignore 忽略，提交 spec / 计划时用 `git add -f`。

## Global Constraints

- 协议版本：`PROTO_VERSION` 4 → 5。
- 条目类型保存后以 `kind` 为准，不从内容推断；界面按内容给默认类型（含 `\` `/` `:` 为路径）。
- **不兼容旧数据、不迁移老用户**：读取配置时丢弃旧格式的进程分流规则（`processes` 含字符串，或带 `exe_path` / `match_full_path`），其余配置照常读取，**不能让整份配置被隔离**；旧 `on_routa_exit` 直接忽略。
- 取消全局「退出 Routa 后」：所有不可用场景按规则 `on_fault`；进程名 + 阻断与路径 + 阻断处理相同。界面名「Routa 或组件不可用时」，新规则默认直连。
- 「被其他规则占用的位置」只看**启用**的规则（且全局开关打开）；规则间判重照旧包含停用规则。
- **不设软件自定义上限**：已记录位置、守护路径数量都不设上限；增长靠"文件已不存在的记录自动删除"控制。唯一保留的上限是 ProxyBridge 的 `MAX_PROXY_CONFIGS = 32`（`third_party/proxybridge/src/ProxyBridge.h:16`）。代理配置按代理地址共用：Routa 为每个「节点 + 网卡」组合只建一个隐藏监听端口（`model/hidden.rs`），所以它限制的是「启用规则里不同的『节点 + 网卡』组合数」。
- 学到新位置不弹提示；只在保存让程序新进入"按进程名阻断"时弹一次 info toast（约 10 秒）。
- 卸载分流服务照旧清掉全部 WFP 规则。
- 界面遵循 AGENTS.md：常驻说明最少，说明进 ? 图标；对话框固定高度。
- 文案（逐字使用）：
  - 选项名：`Routa 或组件不可用时`；规则卡片标签：`不可用时阻断`。
  - 选项 ?：`Routa 退出或尚未运行、mihomo 内核启动或被停止、内核或分流组件出故障时生效；节点失效、网卡断开时，始终阻断。\n\n选阻断后，按进程名匹配的程序也会被阻断。程序联网时会自动记录它的位置，分流服务停止后，系统防火墙按这些位置继续阻断；还没联网过的位置除外。\n\n其他规则用路径指定的位置不会被记录，也不会被这条规则阻断。`
  - 进程 ?：`所有位置（按进程名）：同名程序装在哪里都会命中。\n指定位置（按路径）：只命中这个位置的程序。\n同一个程序可以两个都选。\n同名程序：填了路径的规则优先。`
  - toast：`已开启按进程名阻断\n{名称以、连接} 联网时会自动记录位置，分流服务停止后也能拦住；还没联网过的位置除外。`；有跳过位置时追加一行 `{「规则」指定的 x.exe，以、连接} 不受影响。`
  - 对话框跳过提示：`{x.exe 在「规则」指定的位置，以、连接}不受这条规则阻断。`
  - 已记录：`已记录 N 个位置`、`，M 个位置属于其他规则`、`尚未记录位置`、`程序联网后自动记录。在此之前，分流服务停止时拦不住。`、`不记录：这个位置属于规则「X」，不受这条规则阻断`、`未运行`、移除按钮 tooltip `移除这个位置（程序再次联网时会重新记录）`。
  - 超限：`走代理的「节点 + 网卡」组合最多 32 个`。
  - 托盘：`停用进程分流` / `启用进程分流`、`停用进程分流并退出`；进程分流页按钮：`停用并退出`。
  - 退出确认：标题 `退出 Routa？`；正文 `下面这些规则选了阻断，退出后对应程序会断网，直到 Routa 再次运行：{规则名，以、连接}`；按钮 `停用进程分流并退出`、`仍然退出`、`取消`；复选框 `不再提示`。

设计 4.4 的"缩短服务不在的时间"已由现有安装逻辑满足（`procsvc/src/install.rs`：自动启动，故障后 1 / 5 / 30 秒重启，非崩溃退出也重启），本计划不再改动。

## Review Focus

1. **旧配置文件里有进程分流规则**：只丢掉这些规则，订阅、节点等其他配置必须保留，不能整份被隔离 —— Task 5 测试 `old_format_rules_are_dropped_not_quarantined`。
2. **已记录的旧版本文件被删除**（商店应用更新后旧目录被清理）：WFP 不能因文件不存在而安装失败、把整条规则打成失败 —— Task 4 测试 `missing_guard_files_are_skipped_and_forgotten`。
3. **学到之后另一条规则才用路径接管了这个位置**（或那条规则被重新启用）：守护必须立刻让出 —— Task 7 测试 `guard_paths_yield_to_enabled_path_rules`。
4. **用户手动停止内核 / 正常退出 Routa 时，规则选的是直连**：原来全局默认阻断会拦，现在必须按规则直连，不能残留旧的全局行为 —— Task 2 测试 `graceful_and_never_connected_follow_on_fault`、Task 8 测试 `user_stop_and_starting_follow_on_fault`。
5. **编辑一条早已是"进程名 + 阻断"的规则只改了节点**：不能再弹 info toast —— Task 12 测试 `no_info_toast_when_nothing_newly_learning`。

---

## File Structure

| 文件 | 职责 |
|---|---|
| `src-tauri/procsvc-proto/src/lib.rs` | `ProcessKind`、`RuleSpec.kind / guard_paths`、`SvcStatus.learned`、删除 `ExitPolicy` / `DesiredState.on_routa_exit`、版本 5 |
| `src-tauri/procsvc/src/reconcile.rs` | WFP 守护集合从"规则 exe"推广为 `guard_exes(r)` |
| `src-tauri/procsvc/src/fallback.rs` | Routa 不在时一律按规则 `on_fault` |
| `src-tauri/procsvc/src/server.rs` | Apply 校验、`guarded_keys`、学习结果并入生效状态 |
| `src-tauri/procsvc/src/proxypool.rs`（新） | 按代理地址共用 ProxyBridge 代理配置 |
| `src-tauri/procsvc/src/learn.rs`（新） | 服务端学习的纯逻辑 |
| `src-tauri/procsvc/src/pb_ffi.rs` | 连接回调、代理配置共用、路径规则插到最前 |
| `src-tauri/src/model/types.rs` | `ProcessEntry`、`learned`、`exit_prompt`，删除 `match_full_path` / `exe_path` / `on_routa_exit` |
| `src-tauri/src/store.rs` | 读取时丢弃旧格式进程分流规则 |
| `src-tauri/src/model/normalize.rs` | `prune_learned`，删除 `migrate_legacy_process_rules` |
| `src-tauri/src/model/validate.rs` | 按条目类型校验、4.1 判重、代理组合上限 |
| `src-tauri/src/procroute/learn.rs`（新） | 守护路径计算、合并服务回报、保存时预记录、删除已不存在的记录 |
| `src-tauri/src/procroute/store.rs`（新） | 商店应用安装 / 更新事件、同族新版本路径推导 |
| `src-tauri/src/procroute/decide.rs` | 下发 `kind` 与 `guard_paths`；启动中 / 手动停止按 `on_fault` |
| `src-tauri/src/procroute/client.rs` | 合并服务回报的学习结果、商店事件 |
| `src-tauri/src/commands.rs`、`src-tauri/src/lib.rs` | 保存时预记录、移除记录命令、`save_process_routing_settings(enabled)`、退出相关命令 |
| `src-tauri/src/tray.rs` | 托盘「停用 / 启用进程分流」「停用进程分流并退出」、退出确认 |
| `src-tauri/src/desktop.rs` | 简洁版规则改用条目 |
| `src/lib/processRule.ts` | 前端分组、冲突、占用、学习相关纯函数 |
| `src/components/ProcessEntriesField.tsx`（新） | 进程标签 + 多选菜单 |
| `src/components/LearnedLocations.tsx`（新） | 已记录的位置 |
| `src/components/ExitConfirmDialog.tsx`（新） | 退出确认对话框 |
| `src/pages/ProcessRuleDialog.tsx` | 新表单 + toast |
| `src/pages/ProcessRouting.tsx`、`src/pages/SimpleConnection.tsx`、`src/pages/Settings.tsx` | 列表与简洁版改用条目；去掉全局「退出 Routa 后」；「停用并退出」按钮 |
| `src/components/ApplyResultSnackbar.tsx` | `showMessage` 支持 `info` |
| `src/api/procroute.ts` | `useRemoveLearnedPath`、`useDisableRoutingAndQuit` |

---

### Task 1: 协议 v5

**Files:**
- Modify: `src-tauri/procsvc-proto/src/lib.rs`

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)] #[cfg_attr(feature = "specta", derive(specta::Type))] pub enum ProcessKind { Name, Path }`
  - `RuleSpec { id, exe_path, kind: ProcessKind, guard_paths: Vec<String>, action, on_fault, reset_on_change }`（`guard_paths` 带 `#[serde(default)]`）。文档注释：`kind=Name` 时 `exe_path` 是进程名，阻断由 ProxyBridge 按名称执行，`guard_paths` 是已知位置（仅用于 WFP 守护）；`kind=Path` 时 `guard_paths` 必须为空。`on_fault` 现在管所有 Routa / 组件不可用场景。
  - `DesiredState { rules, routa_pid }`（删除 `on_routa_exit`），删除 `ExitPolicy`。
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] pub struct LearnedPath { pub rule_id: String, pub path: String }`（`rule_id` 是 `RuleSpec.id`）
  - `SvcStatus.learned: Vec<LearnedPath>`（`#[serde(default)]`）—— 服务自己学到、且还不在对应 `RuleSpec.guard_paths` 里的位置。
  - `pub const PROTO_VERSION: u32 = 5;`

- [ ] **Step 1: 写失败测试** `rule_spec_round_trips_kind_and_guard_paths`：`kind: Name, guard_paths: [C:\a\x.exe]` 的 `RuleSpec` 经 `encode` / `decode` 相等；缺 `guard_paths` 的 JSON 解出空 `Vec`；`SvcStatus` 缺 `learned` 解出空；`DesiredState` 序列化不含 `on_routa_exit`；`assert_eq!(PROTO_VERSION, 5)`。
- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml -p routa-procsvc-proto` → 编译失败。
- [ ] **Step 3: 实现**。
- [ ] **Step 4:** 同一命令 → PASS（其余 crate 暂时编译失败，Task 2 / 5 修复）。
- [ ] **Step 5: Commit** `feat(procsvc-proto): v5 adds process kind, guard paths, learned paths; drops exit policy`

---

### Task 2: 服务端：守护集合、校验、回落一律按规则

**Files:**
- Modify: `src-tauri/procsvc/src/reconcile.rs`、`src-tauri/procsvc/src/server.rs`、`src-tauri/procsvc/src/paths.rs`、`src-tauri/procsvc/src/fallback.rs`、`src-tauri/procsvc/src/persist.rs`（测试数据）
- Test: 各文件已有测试模块（`server.rs` 的测试构造器 `rule()` 按 exe 是否含 `\` 推断 `kind`）

**Interfaces:**
- Consumes: Task 1。
- Produces:
  - `reconcile::guard_exes(r: &RuleSpec) -> Vec<&str>` —— `on_fault=Block` 时：`Path` → `[exe_path]`，`Name` → `guard_paths`；否则空。`guarded / permitted` 改为"某规则的 `guard_exes` 含此 exe 键"（permitted 还要求 `action != Block`）；`plan` 的 `exes` 集合包含所有规则的 `exe_path` 与 `guard_exes`。`record` 对 `Name` 规则把 `guard_paths` 过滤为已装好阻断的路径。
  - `server::guarded_keys` = 所有规则 `guard_exes` 的 `exe_key` 并集。
  - `paths::matches_process(kind: ProcessKind, pattern: &str, exe: &str) -> bool`（不再从内容推断）。
  - `fallback::fallback(state: &DesiredState, why: RoutaGone) -> Vec<RuleSpec>`：`Graceful` / `NeverConnected` / `Crashed` 一律按每条规则 `on_fault`（`why` 只用于记录中断原因）。

- [ ] **Step 1: 写失败测试**（`reconcile.rs`）：
  - `name_block_guards_learned_paths_not_name`：新 `[Name "a.exe" Block Proxy guard_paths=[C:\x\a.exe]]` → 含 `EnsureBlock{C:\x\a.exe}`、`Permit{C:\x\a.exe}`、`SetRule{exe:"a.exe"}`，无 `exe:"a.exe"` 的 WFP 操作。
  - `adding_guard_path_does_not_touch_pb_rule_or_reset`：同一 Name 规则（`reset_on_change=true`）新增一个 guard path → ops 恰为 `[EnsureBlock{p}, Permit{p}]`。
  - `removing_guard_path_removes_block`：反向 → `[Revoke{p}, RemoveBlock{p}]`。
  - `name_direct_has_no_wfp`：Name + Direct + 非空 guard_paths → 无 WFP 操作。
- [ ] **Step 2: 写失败测试**（`server.rs` Apply 校验表）：Name + Block 被接受（删除原"按进程名匹配的规则不能在故障时阻断"一行）；`kind=Path` 而值是进程名 → `程序路径无效`；`kind=Name` 而值带目录 → `进程名无效`；`kind=Path` 带非空 `guard_paths` → `只有按进程名的规则可以带守护路径`；guard path 不是完整路径 → `守护路径无效`；guard path 与任一 `Path` 规则同键 → `守护路径与规则 {id} 重叠`。`guarded_keys_include_guard_paths`：guard path 在、进程名不在。
- [ ] **Step 3: 写失败测试**（`fallback.rs`，替换原按 `on_routa_exit` 的用例）`graceful_and_never_connected_follow_on_fault`：规则 a `on_fault=Block`、b `on_fault=Direct`（a 为 Name + Proxy）；`Graceful`、`NeverConnected`、`Crashed` 三种都得到 a=`Block`、b=`Direct`；`guard_paths` 原样保留。`server.rs` 中原先断言"正常退出按 `on_routa_exit`"的测试改为按 `on_fault`。
- [ ] **Step 4:** `cargo test --manifest-path src-tauri/Cargo.toml -p routa-procsvc --lib` → 新测试 FAIL。
- [ ] **Step 5: 实现**（`validate` 用 `kind` 选择 `exe_path_problem` / `process_name_problem`）。
- [ ] **Step 6:** 同命令 → PASS（需要管理员的 WFP 集成测试保持原有 `#[ignore]`）。
- [ ] **Step 7: Commit** `feat(procsvc): guard paths for process-name rules; fall back per rule only`

---

### Task 3: 服务端：同一代理地址共用代理配置、路径规则在前

**Files:**
- Create: `src-tauri/procsvc/src/proxypool.rs`（在 `main.rs` 声明）
- Modify: `src-tauri/procsvc/src/pb_ffi.rs`、`src-tauri/procsvc/src/server.rs`

**Interfaces:**
- Produces（`proxypool.rs`，纯逻辑）：`pub struct ProxyPool`，按 `ProxyTarget` 相等去重、引用计数：
  - `pub fn attach(&mut self, rule: &str, target: &ProxyTarget) -> Attach`，`enum Attach { Reuse(u32), Create }`；`pub fn created(&mut self, rule: &str, target: &ProxyTarget, cfg: u32)`
  - `pub fn detach(&mut self, rule: &str) -> Option<u32>` —— 该规则不再引用；返回"引用归零、需要删除的配置 id"。
  - `pub fn cfg_of(&self, rule: &str) -> Option<u32>`
- `pb_ffi.rs`：`set_proxy` / `set_rule` / `remove_rule` 改经 `ProxyPool` 取得配置；目标变化时 `detach` 旧的再 `attach` 新的（不再原地 `edit_proxy_config`，以免影响共用者）。新增 ProxyBridge 规则且 `exe` 为完整路径时调用新加载的 `ProxyBridge_MoveRuleToPosition(id, 1)`（`unsafe extern "C" fn(u32, u32) -> BOOL`），失败返回 `移动规则 {rule} 的 ProxyBridge 规则失败`。
- `server.rs`：`MAX_PROXY_RULES` 改为统计**不同的** `ProxyTarget` 个数（上限仍为 32），文案 `走代理的「节点 + 网卡」组合最多 32 个，收到 {n} 个`。

- [ ] **Step 1: 写失败测试**（`proxypool.rs`）：
  - `rules_share_one_config_per_target`：r1、r2 同目标 → 第一次 `Create`、`created(.., 7)` 后第二次 `Reuse(7)`；`detach(r1)` → `None`；`detach(r2)` → `Some(7)`。
  - `target_change_moves_to_new_config`：r1 目标 A 改 B → `detach` 返回 A 的配置（无其他引用时），`attach` B 为 `Create`。
- [ ] **Step 2: 写失败测试**（`server.rs`）：33 条不同 `exe` 但只有 2 个不同目标的 Proxy 规则 → 接受；33 个不同目标 → 拒绝。
- [ ] **Step 3:** `cargo test --manifest-path src-tauri/Cargo.toml -p routa-procsvc --lib` → FAIL。
- [ ] **Step 4: 实现**。`pb_ffi` 的改动无单元测试（依赖 DLL），Task 14 实机验证。
- [ ] **Step 5:** 同命令 → PASS。
- [ ] **Step 6: Commit** `feat(procsvc): share proxy configs per target; path rules first`

---

### Task 4: 服务端学习位置

**Files:**
- Create: `src-tauri/procsvc/src/learn.rs`（在 `main.rs` 声明）
- Modify: `src-tauri/procsvc/src/pb_ffi.rs`、`src-tauri/procsvc/src/engine.rs`、`src-tauri/procsvc/src/server.rs`

**Interfaces:**
- Consumes: Task 1、2。
- Produces（`learn.rs`，纯逻辑）：
  - `pub fn observe(rules: &[RuleSpec], exe: &str) -> Option<LearnedPath>` —— 找 `kind=Name`、`on_fault=Block`、`exe_path` 等于 `exe` 文件名（不区分大小写）的规则；`exe` 已在它的 `guard_paths`、或与任一 `Path` 规则同键时返回 `None`。
  - `pub fn effective(desired: &DesiredState, learned: &[LearnedPath], exists: &dyn Fn(&str) -> bool) -> DesiredState` —— 把 `learned` 并入对应规则的 `guard_paths`（按 `exe_key` 去重），并删掉所有 `exists` 为假的 guard path。
  - `pub fn forget(learned: &mut Vec<LearnedPath>, desired: &DesiredState, exists: &dyn Fn(&str) -> bool)` —— 删掉已出现在对应规则 `guard_paths` 里的、规则已不存在 / 不再是 Name+Block 的、文件已不存在的。
- `engine.rs`：`Interceptor` 增加 `fn take_new_processes(&mut self) -> Vec<u32> { Vec::new() }`（默认空，测试替身可覆盖）。
- `pb_ffi.rs`：用 `ProxyBridge_SetConnectionCallback` 注册 `extern "C" fn on_connection(name: *const c_char, pid: u32, ..)`，回调只把 `pid` 推入一个 `static` 队列（不得做 I/O、不得阻塞）；`take_new_processes` 取出并清空。
- `server.rs`：`Server` 增加 `learned: Vec<LearnedPath>`；在 `check_routa` 所在的 250 ms tick 里调用新方法 `learn_tick()`：对 `take_new_processes()` 的每个 pid 用 `paths::exe_of_pid` 取路径，`learn::observe` 命中则加入 `learned`，有新增时按 `learn::effective` 重新 `apply` 并 `persist::save`。每次收到 Apply 先 `learn::forget`，再用 `learn::effective` 落地。`GetStatus` 的 `SvcStatus.learned` 返回 `self.learned`。Routa 不在时（回落状态）同样学习。

- [ ] **Step 1: 写失败测试**（`learn.rs`）：
  - `observe_matches_name_block_rules_only`：Name `chatgpt.exe` Block → `C:\N\ChatGPT.exe` 命中；Direct → `None`；已在 guard_paths → `None`；某 `Path` 规则就是 `C:\N\ChatGPT.exe` → `None`。
  - `missing_guard_files_are_skipped_and_forgotten`：`effective` 删掉 `exists` 为假的 guard path（包括 Routa 下发的）；`forget` 删掉文件不存在的 learned。
  - `forget_drops_paths_routa_now_sends`：下发的 guard_paths 已含该路径 → 从 `learned` 删除。
- [ ] **Step 2: 写失败测试**（`server.rs`，用现有测试替身 + 覆盖 `take_new_processes`）`learned_path_is_guarded_and_reported`：Apply 一条 Name+Block 规则 → 替身报告一个 pid（`exe_of_pid` 与文件存在检查经 `Server` 上的可注入函数）→ `learn_tick` 后 guard 替身收到 `ensure_block(p)`，`GetStatus` 的 `learned` 含 `p`；再 Apply 带上该 guard path → `learned` 为空。
- [ ] **Step 3:** `cargo test --manifest-path src-tauri/Cargo.toml -p routa-procsvc --lib` → FAIL。
- [ ] **Step 4: 实现**。
- [ ] **Step 5:** 同命令 → PASS。
- [ ] **Step 6: Commit** `feat(procsvc): learn locations of process-name block rules from connections`

---

### Task 5: 模型：显式条目类型、`learned`、丢弃旧格式规则

**Files:**
- Modify: `src-tauri/src/model/types.rs`、`src-tauri/src/model/normalize.rs`（删除 `migrate_legacy_process_rules`）、`src-tauri/src/store.rs`、`src-tauri/src/model/validate.rs`（编译修复）、`src-tauri/src/desktop.rs`、`src-tauri/src/procroute/decide.rs`（编译修复：`exe_path: p.value.clone(), kind: p.kind, guard_paths: vec![]`；`DesiredState` 不再带 `on_routa_exit`）、`src-tauri/src/runtime/tests.rs` 等构造 `ProcessRule` 的测试

**Interfaces:**
- Consumes: `routa_procsvc_proto::ProcessKind`（在 `model::types` 里 `pub use`）。
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, specta::Type)] pub struct ProcessEntry { pub kind: ProcessKind, pub value: String }`；构造器 `ProcessEntry::name(v)` / `ProcessEntry::path(v)`。
  - `ProcessRule { processes: Vec<ProcessEntry>, learned: Vec<String>, .. }`；删除 `match_full_path`、`exe_path`。
  - `ProcessRouting { enabled, rules, exit_prompt: bool }`（`exit_prompt` 默认 `true`）；`on_routa_exit` 在本任务里**暂时保留**（Task 8 删除）。
  - `store.rs`：`pub fn drop_old_process_rules(raw: &mut serde_json::Value) -> usize` —— 在 `serde_json::from_value` 之前，从 `process_routing.rules` 删掉旧格式规则（`processes` 含字符串元素，或带 `exe_path` / `match_full_path` 键），返回删掉的条数；`load_model` 在反序列化前调用。

- [ ] **Step 1: 写失败测试**：
  - `old_format_rules_are_dropped_not_quarantined`（`store.rs`）：状态文件含一条订阅、一条 `{"processes":["a.exe"],"match_full_path":false}` 规则和一条新格式规则 → `load_model` 成功，订阅保留，规则只剩新格式那条；`{"exe_path":"C:\\a.exe"}` 旧规则同样被丢弃。
  - `entries_round_trip`（`types.rs`）：`[Name "a.exe", Path r"C:\x\a.exe"]` 序列化为对象数组并读回相等。
  - `learned_defaults_empty_and_exit_prompt_defaults_true`。
- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml --lib model:: store::` → 编译失败。
- [ ] **Step 3: 实现** 并修全部编译错误（`desktop.rs` 简洁版：判重改为"任一规则有同文件名的 `Name` 条目或同路径的 `Path` 条目"，新规则 `processes: vec![ProcessEntry::path(path)]`）。
- [ ] **Step 4:** `cargo test --manifest-path src-tauri/Cargo.toml --lib` → PASS。
- [ ] **Step 5: Commit** `feat(model): explicit process entry kind; drop old-format rules on load`

---

### Task 6: 校验：按条目类型、4.1 判重、代理组合上限

**Files:**
- Modify: `src-tauri/src/model/validate.rs`、`src-tauri/src/desktop.rs`（删 `ProcessNameCannotBlock` 文案分支）、`src/lib/validation.ts`、`src/lib/validation.test.ts`

**Interfaces:**
- Produces: 删除 `ValidationError::ProcessNameCannotBlock`；`DuplicateProcessRule { exe_path }` 改为"同一进程名或同一路径重复"；`InvalidExePath` 用于类型与内容不一致（原因：`按进程名匹配时只填写进程名，如 app.exe` / 沿用 `exe_path_problem`）；`TooManyProcessRules { max: 32 }` 改为统计「启用规则里不同的 (node, 生效网卡) 组合数」（生效网卡用 `hidden.rs` 的 `effective_interface`，与隐藏监听器分组一致）；`validate_forbidden_exes`：`Name` 比较文件名，`Path` 比较整条路径。

- [ ] **Step 1: 写失败测试**：
  - `name_and_path_of_same_program_coexist`；`name_vs_other_rule_path_is_not_duplicate`；
  - `name_vs_name_and_same_path_are_duplicates`（跨规则同名、跨规则同路径大小写不同、同规则两个同名）；
  - `disabled_rules_still_count_for_duplicates`；`name_rule_can_block`；`kind_must_match_value`；
  - `limit_counts_node_interface_pairs`：40 条启用规则、每条 2 个条目，只用到 3 个不同节点 → 无错误；33 个不同节点各一条启用规则 → `TooManyProcessRules`；同节点不同网卡算两个。
- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml --lib model::validate` → FAIL。
- [ ] **Step 3: 实现**；前端 `validation.ts` 删除 `ProcessNameCannotBlock` 分支及用例，`TooManyProcessRules` 文案改为 `走代理的「节点 + 网卡」组合最多 32 个`。
- [ ] **Step 4:** 上述 cargo 命令 + `pnpm vitest run src/lib/validation.test.ts` → PASS。
- [ ] **Step 5: Commit** `feat(validate): per-entry kinds, path-wins duplicates, proxy-pair limit`

---

### Task 7: Routa：下发守护路径、合并服务回报、保存时预记录

**Files:**
- Create: `src-tauri/src/procroute/learn.rs`（在 `procroute/mod.rs` 声明）
- Modify: `src-tauri/src/procroute/decide.rs`、`src-tauri/src/procroute/client.rs`、`src-tauri/src/model/normalize.rs`、`src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Task 1 `LearnedPath`、`SvcStatus.learned`；Task 5 模型；`procroute::processes::{RunningProcess, running_processes}`；`decide::rule_of_spec`。
- Produces（`learn.rs`）：
  - `pub fn is_learning(model: &Model, r: &ProcessRule) -> bool` —— 全局开关开、规则启用、`on_fault=Block`、至少一个 `Name` 条目。
  - `pub fn owned_by_others(model: &Model, rule: &ProcessRuleId, path: &str) -> bool` —— 其他**启用**规则有同键 `Path` 条目。
  - `pub fn guard_paths(model: &Model, r: &ProcessRule, name: &str) -> Vec<String>` —— `r.learned` 中文件名等于 `name`、非本规则 `Path` 条目、非 `owned_by_others` 的路径（存在性由服务端过滤）。
  - `pub fn merge_reported(pr: &mut ProcessRouting, reported: &[LearnedPath]) -> bool` —— 按 `rule_of_spec` 找规则，追加未有的路径（按 `exe_key` 去重），返回是否有改动。
  - `pub fn seed(model: &Model, before: Option<&ProcessRule>, rule: &mut ProcessRule, running: &[RunningProcess])` —— 只对本次新进入学习的进程名追加正在运行的同名 exe 路径（排除本规则路径与 `owned_by_others`）。
  - `pub fn drop_missing(pr: &mut ProcessRouting, exists: &dyn Fn(&str) -> bool) -> bool`
  - `pub fn newly_learning(before: Option<&ProcessRule>, after: &ProcessRule) -> Vec<String>`
- Produces（`normalize.rs`）：`pub fn prune_learned(r: &mut ProcessRule)` —— 停用或 `on_fault≠Block` 时清空；否则只保留文件名匹配某个 `Name` 条目、且不等于本规则 `Path` 条目的路径，去重。在 `normalize()` 里对每条规则调用。
- Produces（`decide.rs`）：每个条目一条 `RuleSpec`，`kind` 取条目类型；`guard_paths` 仅对 `Name` 条目且规则 `on_fault=Block` 时为 `learn::guard_paths(..)`。
- `client.rs`：每次取到 `SvcStatus` 后，`learned` 非空则 `rt.mutate_process_routing(|pr| { learn::merge_reported(pr, &s.learned); })`（错误只记日志）；`run` 启动时 `learn::drop_missing`（有改动则保存）。
- 命令：`ops::save_process_rule` 在 `mutate_process_routing` 闭包里先 `learn::seed(..)`（`running` 在闭包外取一次；`save_process_rule_confirmed` 若不经过它也要调用）；新命令 `remove_learned_process_path(id: ProcessRuleId, path: String) -> Result<Option<ApplyOutcome>, Vec<ValidationError>>`，在 `lib.rs` 注册。

- [ ] **Step 1: 写失败测试**（`decide.rs`）：
  - `name_block_rule_sends_kind_and_guard_paths`：`[Name ChatGPT.exe]` Block，`learned=[C:\A\ChatGPT.exe, C:\B\other.exe]` → `kind=Name`、`guard_paths=[C:\A\ChatGPT.exe]`。
  - `guard_paths_yield_to_enabled_path_rules`：另一启用规则有 `Path C:\A\ChatGPT.exe` → 不在 `guard_paths`；那条规则停用 → 又出现。
  - `direct_rule_sends_no_guard_paths`；`path_entry_kind_is_path`。
- [ ] **Step 2: 写失败测试**（`learn.rs`）：`merge_reported_adds_once`（重复回报只加一次，返回 `false`）；`seed_only_for_newly_learning_names`（两个运行中的 `chrome.exe`，被他规则占用的那个不加；`before` 已是同名 Block 时不加）；`drop_missing_removes_deleted_files`。
- [ ] **Step 3: 写失败测试**（`normalize.rs`）`prune_learned_clears_on_direct_disable_and_removed_name`。
- [ ] **Step 4:** `cargo test --manifest-path src-tauri/Cargo.toml --lib procroute model::normalize` → FAIL。
- [ ] **Step 5: 实现**。
- [ ] **Step 6:** `cargo test --manifest-path src-tauri/Cargo.toml --lib` → PASS。
- [ ] **Step 7: Commit** `feat(procroute): send guard paths and keep service-learned locations`

---

### Task 8: 取消全局「退出 Routa 后」（Routa 侧与设置页）

**Files:**
- Modify: `src-tauri/src/model/types.rs`、`src-tauri/src/procroute/decide.rs`、`src-tauri/src/commands.rs`、`src-tauri/src/procroute/client.rs`（测试数据）、`src/pages/Settings.tsx`、`src/pages/ProcessRouting.tsx`、相关测试

**Interfaces:**
- Consumes: Task 1（`ExitPolicy` 已从协议删除）。
- Produces：
  - 删除 `ProcessRouting.on_routa_exit` 与 `model::types` 里对 `ExitPolicy` 的 re-export（旧 JSON 里的该字段被 serde 忽略）。
  - `decide::rule_action`：`CoreState::Starting`（非恢复中）与 `Stopped` + `CoreStopReason::User` 改为 `fault_action(&rule.on_fault)`；删除 `exit_action`。原因文案不变（`内核启动中`、`内核已手动停止`）。
  - `ops::save_process_routing_settings(rt, enabled: bool)` 与同名命令只收 `enabled`。
  - 设置页删除「退出 Routa 后」选项；`ProcessRouting.tsx` 里 `ComponentStatus` / `ComponentAlerts` 去掉 `onExit` 参数，`settings.mutate(enabled)`；页面底部文案 `退出 Routa 后的行为、卸载组件和清理残留阻断规则在「设置 → 进程分流」中。` 改为 `卸载组件和清理残留阻断规则在「设置 → 进程分流」中。`

- [ ] **Step 1: 写失败测试**（`decide.rs`，替换原 `on_routa_exit` 相关用例）`user_stop_and_starting_follow_on_fault`：`on_fault=Direct` 时内核手动停止 → `Direct`，正常启动中 → `Direct`；`on_fault=Block` 时两者 → `Block`；内核恢复中、崩溃保持按 `on_fault`。
- [ ] **Step 2:** `cargo test --manifest-path src-tauri/Cargo.toml --lib procroute::decide` → FAIL。
- [ ] **Step 3: 实现** Rust 部分，重新生成绑定（`cargo run --manifest-path src-tauri/Cargo.toml -- --export-bindings`），再改前端；更新 `commands.rs` 中 `save_process_routing_settings` 的测试与前端 `Settings` / `ProcessRouting` 测试里对 `on_routa_exit` 的引用。
- [ ] **Step 4:** `cargo test --manifest-path src-tauri/Cargo.toml --lib` 与 `pnpm vitest run src/pages/Settings.test.tsx src/pages/ProcessRouting.test.tsx` → PASS。
- [ ] **Step 5: Commit** `feat(procroute): drop the global exit policy; rules decide every unavailable case`

---

### Task 9: 商店应用更新事件

**Files:**
- Create: `src-tauri/src/procroute/store.rs`（在 `procroute/mod.rs` 声明）
- Modify: `src-tauri/Cargo.toml`（`[target."cfg(windows)".dependencies]` 加 `windows = { version = "0.62", features = ["ApplicationModel", "Foundation"] }`，与依赖树中已有的 0.62.2 同版本）、`src-tauri/src/procroute/learn.rs`、`src-tauri/src/procroute/client.rs`

**Interfaces:**
- Produces（`store.rs`）：
  - `pub fn family_key(full_name: &str) -> Option<(String, String)>` —— 包全名 `Name_Version_Arch_ResourceId_PublisherId` → （去掉 `.v<数字和->` 后缀的小写名称，小写发布者 id）。
  - `pub fn siblings(learned: &[String], new_full_name: &str, new_root: &str) -> Vec<String>` —— 对 `learned` 中位于 `\WindowsApps\<包全名>\<相对路径>`、且与新包同 `family_key` 的路径，产出 `<new_root>\<相对路径>`；排除已在 `learned` 的，去重。
  - `pub fn watch(on_package: impl Fn(String /*full name*/, String /*root*/) + Send + 'static) -> windows::core::Result<WatchGuard>` —— `PackageCatalog::OpenForCurrentUser()`，订阅 `PackageInstalling` / `PackageUpdating`，在 `IsComplete()` 为真时用 `Package().Id().FullName()` 与 `Package().InstalledLocation().Path()` 回调；`WatchGuard` 析构时注销。
- Produces（`learn.rs`）：`pub fn from_package(model: &Model, full_name: &str, root: &str, exists: &dyn Fn(&str) -> bool) -> Vec<(ProcessRuleId, String)>`（对每条 `is_learning` 规则取 `siblings`，保留 `exists` 为真且不 `owned_by_others` 的）与 `pub fn add(pr: &mut ProcessRouting, found: &[(ProcessRuleId, String)])`（去重追加）。
- `client.rs`：`run` 启动时 `store::watch`，回调经 `mpsc` 通知循环；循环收到后 `from_package` → 非空则 `mutate_process_routing(|pr| learn::add(..))`。`watch` 失败只记日志。

- [ ] **Step 1: 可行性检查（先做）**：写 `#[test] #[ignore] fn catalog_opens_in_unpackaged_process()`，调用 `store::watch(|_, _| {})` 断言 `Ok`；`cargo test --manifest-path src-tauri/Cargo.toml --lib procroute::store -- --ignored` → PASS。若失败（非打包进程不能订阅），停止本任务并报告，不要换实现。
- [ ] **Step 2: 写失败测试**：
  - `family_key_strips_version_suffix`：`OpenAI.Codex_26.1029.7803.0_x64__2p2nqsd0c76g0` → `("openai.codex", "2p2nqsd0c76g0")`；两个 `OpenAI.CodexPrimaryRuntime.v26-…` 版本同键；`not-a-package` → `None`。
  - `siblings_map_relative_path_into_new_version`：`learned=[C:\Program Files\WindowsApps\OpenAI.Codex_26.1029.7803.0_x64__2p2nqsd0c76g0\app\ChatGPT.exe]`，新包 `OpenAI.Codex_26.1105.8012.0_x64__2p2nqsd0c76g0` 根目录 `C:\Program Files\WindowsApps\OpenAI.Codex_26.1105.8012.0_x64__2p2nqsd0c76g0` → `[…26.1105…\app\ChatGPT.exe]`；非 WindowsApps 路径不产出。
  - `from_package_requires_existing_file`（`learn.rs`）。
- [ ] **Step 3:** `cargo test --manifest-path src-tauri/Cargo.toml --lib procroute::store procroute::learn` → FAIL。
- [ ] **Step 4: 实现**。
- [ ] **Step 5:** 同命令 → PASS。
- [ ] **Step 6: Commit** `feat(procroute): follow store app updates to new install folders`

---

### Task 10: 前端绑定与纯函数

**Files:**
- Modify: `src/bindings.ts`（生成）、`src/lib/processRule.ts`
- Test: `src/lib/processRule.test.ts`

**Interfaces:**
- Consumes: 绑定 `ProcessEntry { kind: ProcessKind; value: string }`、`ProcessKind = "Name" | "Path"`、`ProcessRule.learned: string[]`、命令 `removeLearnedProcessPath(id, path)`。
- Produces（`processRule.ts`）：
  - `fileName(p)`（保留）、`defaultKind(raw): ProcessKind`、`cleanEntry(raw): string`（去引号、`/`→`\`、trim）
  - `interface ProcessGroup { key: string; name: string; hasName: boolean; paths: string[] }`、`groupsOf(entries): ProcessGroup[]`、`chipLabel(g): string`（`a.exe` / `a.exe + N 个位置` / `a.exe · N 个位置`）
  - `ruleLabel(rule)`：名称为空时 `a.exe` / `a.exe 等 N 个` / `未命名规则`（按分组）
  - `entryError(e, entries, others): string | undefined`（文案：`不是完整路径（应以 C:\ 之类的盘符开头）`、`{x} 不是 .exe 程序`、`已经在这条规则里`、`已在规则「X」中按进程名匹配；要单独分流某个位置，请勾选它的路径`、`这个位置已在规则「X」中`；`others` 含停用规则）
  - `pathOwners(name, others, routingEnabled)`（只看启用规则）、`learningNames(rule)`、`skippedOf(rule, others, routingEnabled)`、`newlyLearning(before | undefined, after): string[]`、`blockingRules(rules, routingEnabled): ProcessRule[]`（启用且 `on_fault=Block`）

- [ ] **Step 1: 生成绑定**：`cargo run --manifest-path src-tauri/Cargo.toml -- --export-bindings`，确认出现 `ProcessEntry`、`ProcessKind`、`learned`、`exit_prompt`、`removeLearnedProcessPath`，且没有 `ExitPolicy`。
- [ ] **Step 2: 写失败测试**（值取自原型 mock）：`groupsOf` 合并同名名称与路径；`chipLabel` 三种形态；`entryError` 5 种文案及"名称与他规则路径不冲突"；`pathOwners` 忽略停用规则；`newlyLearning`：Direct→Block 返回名称，已 Block 只改节点返回 `[]`，新增一个 `Name` 返回该名称；`blockingRules` 忽略停用与直连规则。
- [ ] **Step 3:** `pnpm vitest run src/lib/processRule.test.ts` → FAIL。
- [ ] **Step 4: 实现**。
- [ ] **Step 5:** 同命令 → PASS。
- [ ] **Step 6: Commit** `feat(ui): process entry helpers and regenerated bindings`

---

### Task 11: 组件：进程标签字段、已记录的位置

**Files:**
- Create: `src/components/ProcessEntriesField.tsx`、`src/components/LearnedLocations.tsx`
- Test: `src/components/ProcessEntriesField.test.tsx`、`src/components/LearnedLocations.test.tsx`

**Interfaces:**
- Consumes: Task 10；绑定 `RunningProcess`。
- Produces：
  - `ProcessEntriesField(props: { entries: ProcessEntry[]; others: ProcessRule[]; routingEnabled: boolean; running: RunningProcess[]; error?: string; onChange(entries: ProcessEntry[]): void })` —— 行为照原型 `ProcessChips`。
  - `LearnedLocations(props: { rule: ProcessRule; others: ProcessRule[]; routingEnabled: boolean; running: RunningProcess[]; onRemove?(path: string): void })` —— 行为与文案照原型；不在学习时渲染 `null`。

- [ ] **Step 1: 写失败测试**（移植 `.superpowers/proto/smoke.test.tsx` 对应断言）：Field —— 回车生成 `chrome.exe`；勾选正式版路径后 `chrome.exe + 1 个位置`；Canary 项 `aria-disabled` 且说明含「Chrome Canary」；唯一勾选项不可取消；规则内重复报错；`Telegram.exe` 被他规则按名称占用时报错、粘贴其路径可加入。Learned —— 展开显示路径与 `未运行`；`尚未记录位置`；跳过项文案；点移除调用 `onRemove(path)`。
- [ ] **Step 2:** `pnpm vitest run src/components/ProcessEntriesField.test.tsx src/components/LearnedLocations.test.tsx` → FAIL。
- [ ] **Step 3: 实现**（从原型移植，`kind` 用 `"Name" | "Path"`）。
- [ ] **Step 4:** 同命令 → PASS。
- [ ] **Step 5: Commit** `feat(ui): process entries field and learned locations`

---

### Task 12: 规则对话框、规则列表、简洁版、toast

**Files:**
- Modify: `src/pages/ProcessRuleDialog.tsx`、`src/pages/ProcessRouting.tsx`、`src/pages/SimpleConnection.tsx`、`src/components/ApplyResultSnackbar.tsx`、`src/api/procroute.ts`
- Test: `src/pages/ProcessRuleDialog.test.tsx`、`src/pages/ProcessRouting.test.tsx`、`src/pages/SimpleConnection.test.tsx`、`src/components/ProcessNodeSelect.test.tsx`（修测试数据）

**Interfaces:**
- Consumes: Task 10、11。
- Produces：`ApplyResultApi.showMessage(text, severity: "success" | "error" | "info")`（`info` 时 `autoHideDuration` 10000、`white-space: pre-line`）；`useRemoveLearnedPath()` 调 `commands.removeLearnedProcessPath`，成功后失效 model 查询。

- [ ] **Step 1: 写失败测试**（`ProcessRuleDialog.test.tsx`）：
  - 字段顺序：规则名称、进程、节点、网卡、启用、Routa 或组件不可用时、规则变化时断开旧连接；无「高级选项」与「按完整路径匹配」；新建规则该选项默认「直连」。
  - 只有进程名时「阻断」可选。
  - 选阻断且他启用规则有同名路径时显示 `chrome.exe 在「Chrome Canary」指定的位置不受这条规则阻断。`
  - `info_toast_on_newly_learning`：新建 `[Name chrome.exe]` + 阻断保存成功 → `showMessage` 以 `info` 调用，含 `已开启按进程名阻断` 与 `「Chrome Canary」指定的 chrome.exe 不受影响。`
  - `no_info_toast_when_nothing_newly_learning`：已是 `[Name a.exe]` + Block 的规则只改节点 → 不以 `info` 调用。
  - 编辑学习中的规则显示 `LearnedLocations`，移除后保存的 `learned` 不含该路径。
- [ ] **Step 2: 写失败测试**（`ProcessRouting.test.tsx`）：标签用 `chipLabel`；阻断规则卡片显示 `不可用时阻断`；学习中的规则显示 `已记录 N 个位置`，移除调用 `removeLearnedProcessPath`；搜索按条目 `value` 匹配。
- [ ] **Step 3:** `pnpm vitest run src/pages src/components` → FAIL。
- [ ] **Step 4: 实现**（对话框以原型 `RuleDialog` 为准，保留现有 `useSaveProcessRuleConfirmed`、`NodePickerDialog`、`InterfacePicker`、Tailscale 网卡锁定与 `ruleErrors` 映射；`SimpleConnection` 的 `processes[0]` → `processes[0]?.value`）。
- [ ] **Step 5:** `pnpm typecheck && pnpm test` → PASS。
- [ ] **Step 6: Commit** `feat(ui): rule dialog with per-entry kinds, process-name block and learned locations`

---

### Task 13: 停用进程分流（并退出）与退出确认

**Files:**
- Modify: `src-tauri/src/tray.rs`、`src-tauri/src/commands.rs`、`src-tauri/src/lib.rs`、`src/pages/ProcessRouting.tsx`、`src/api/procroute.ts`、`src/App.tsx`（挂载对话框）
- Create: `src/components/ExitConfirmDialog.tsx`
- Test: `src-tauri/src/tray.rs` 测试模块、`src/components/ExitConfirmDialog.test.tsx`、`src/pages/ProcessRouting.test.tsx`

**Interfaces:**
- Consumes: Task 5 `ProcessRouting.exit_prompt`、Task 10 `blockingRules`。
- Produces（Rust）：
  - `tray::exit_needs_confirm(model: &Model) -> bool` —— `process_routing.enabled && exit_prompt` 且存在启用的 `on_fault=Block` 规则。
  - 托盘菜单：进程分流开着且有启用规则时显示 `停用进程分流`（点击 → `mutate_process_routing(|p| p.enabled = false)`）与 `停用进程分流并退出`（先停用、等本轮下发完成，再 `app.exit(0)`）；关着时第一项为 `启用进程分流`，第二项不显示。菜单随 model 变化重建。
  - 托盘 `quit`：`exit_needs_confirm` 为真时不直接退出，而是显示主窗口并 `emit("confirm-exit", 规则名列表)`；否则照旧 `app.exit(0)`。
  - 命令：`quit_app()`（直接退出）、`disable_routing_and_quit()`（停用后退出）、`set_exit_prompt(enabled: bool)`（经 `mutate_process_routing` 写 `exit_prompt`）。
- Produces（前端）：
  - `ExitConfirmDialog` —— 监听 `confirm-exit` 事件；标题、正文、三个按钮与 `不再提示` 复选框按 Global Constraints 文案；勾选后点 `停用进程分流并退出` 或 `仍然退出` 时先 `setExitPrompt(false)`；`取消` 只关闭。
  - `useDisableRoutingAndQuit()`；进程分流页总开关旁加按钮 `停用并退出`（进程分流开着时显示）。

- [ ] **Step 1: 写失败测试**（`tray.rs`）`exit_needs_confirm_only_with_enabled_block_rules`：无规则 → 否；仅直连 / 停用的阻断规则 → 否；启用的阻断规则 → 是；`exit_prompt=false` → 否；全局开关关 → 否。
- [ ] **Step 2: 写失败测试**（`ExitConfirmDialog.test.tsx`）：收到事件后显示规则名；勾选「不再提示」再点「仍然退出」→ 依次调用 `setExitPrompt(false)` 与 `quitApp()`；点「停用进程分流并退出」→ 调用 `disableRoutingAndQuit()`；「取消」不调用任何命令。（`ProcessRouting.test.tsx`）进程分流开着时显示 `停用并退出`，点击调用 `disableRoutingAndQuit()`。
- [ ] **Step 3:** `cargo test --manifest-path src-tauri/Cargo.toml --lib tray` 与 `pnpm vitest run src/components/ExitConfirmDialog.test.tsx src/pages/ProcessRouting.test.tsx` → FAIL。
- [ ] **Step 4: 实现**（命令在 `lib.rs` 注册并重新生成绑定）。
- [ ] **Step 5:** 同命令 → PASS。
- [ ] **Step 6: Commit** `feat: disable process routing (and quit) from tray and page; confirm exit with block rules`

---

### Task 14: 全量验证与实机检查

**Files:** 无新增（只修验证中发现的问题）

- [ ] **Step 1:** `cargo test --locked --manifest-path src-tauri/Cargo.toml --workspace` → PASS。
- [ ] **Step 2:** `pnpm typecheck && pnpm test && pnpm build` → PASS。
- [ ] **Step 3:** `pnpm build-procsvc --locked`，在 Routa 开发版里「更新组件」（需管理员，用户在 UAC 中确认）。
- [ ] **Step 4: 实机检查**（逐条记录结果）：
  1. 用现有配置启动：旧进程分流规则被丢弃，订阅、节点等其他配置都在。
  2. Chrome 正式版与 Canary：规则 A `[Name chrome.exe]` 走节点 X，规则 B `[Path Canary]` 走节点 Y；两边连接分别走 X / Y（路径规则在前）。
  3. 规则 A 同时加正式版路径、再新建一条同节点同网卡的规则后，代理配置数不增加。
  4. 规则 A 选阻断保存：toast 出现；Chrome 联网后「已记录」出现正式版路径；Canary 显示为「不记录」。
  5. 停止 mihomo 内核：规则 A（阻断）Chrome 正式版无法联网；规则 B（直连）Canary 直连。
  6. 托盘「退出」：弹出退出确认，列出规则 A；选「停用进程分流并退出」后 Chrome 可以联网。重新启动 Routa，再「仍然退出」：Chrome 无法联网。
  7. 停止分流服务（管理员 `sc stop RoutaProcSvc`）：Chrome 正式版仍无法联网（WFP）；恢复服务后正常。
  8. 用 `winget upgrade` 或商店更新一个已记录的商店应用：新版本路径在启动前出现在「已记录」里。
  9. 设置页「清理残留阻断规则」后，再次应用规则恢复正常；设置页已没有「退出 Routa 后」。
- [ ] **Step 5: Commit**（如有修复）`fix: issues found in manual verification`

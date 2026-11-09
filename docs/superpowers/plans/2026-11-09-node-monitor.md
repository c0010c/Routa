# 节点监测 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 按固定间隔探测用户关心的节点，节点从可用变为不可用（以及恢复）时发 Windows 系统通知，点击通知定位到节点。

**Architecture:** 设置存进 `Model.settings.node_monitor`（state.json）。Rust 端有一个纯逻辑模块 `monitor`：负责展开监测目标、按轮判定状态、生成通知文案。runtime 里有一个独立的 1 秒检查循环，到期就调用它，复用 `test_delays` 测延迟。通知用新的 `RuntimeEvent::Notify` 送到 `lib.rs`，由 `notify.rs` 弹 WinRT toast。点击 toast 后唤起窗口，并向前端发 `monitor-focus` 事件。前端在设置页加「节点监测」分区，节点卡片上加监测标记和菜单项，所有监测状态都从 `RuntimeStatus.monitor` 读取。

**Tech Stack:** Rust（tokio、serde、specta、tauri 2）、`tauri-winrt-notification`、React 19 + MUI 9 + TanStack Query、vitest + Testing Library。

**Spec:** `docs/superpowers/specs/2026-11-09-node-monitor-design.md`（交互原型：`designs/node-monitor-v1/`）

## Global Constraints

- 界面和交互以原型 `designs/node-monitor-v1/` 为准，文案以 spec 为准。所有文案都是中文，新增控件要有 `aria-label`。
- 默认值：`enabled=false`、`scope=InUse`、`members=[]`、`interval_secs=300`、`fail_threshold=2`、`latency_limit_ms=Some(1000)`、`notify_recovery=true`。
- 间隔只能取 `10, 30, 60, 180, 300, 600, 1800, 3600` 秒（最低 10 秒）。阈值取 `1, 2, 3, 5`。延迟上限取 `1..=60000` 或 `None`（不限）。
- 监测单次探测超时 `5000` ms。并发上限用 `DEFAULT_CONCURRENCY`。两轮不重叠，下一轮从上一轮结束时开始计时。
- 合并通知的门槛是同一轮新出现 ≥3 个不可用节点，正文最多列 5 个名字，超出部分写「等」。
- 只有有效结果 ≥2 个且全部是「无延迟」（超时或连接失败，不含超过上限）时，才判定为疑似断网。
- 修改监测设置不能触发 mihomo 重启或重新 apply（现有 `save_settings` 在 `!core_changed` 时已经满足，不要破坏）。
- 探测期间不能一直持有 engine 锁（复用 `test_delays` 的「先取快照再释放」做法）。`monitor` 锁和 `status` 锁不能同时持有。
- 简洁模式（`preferences.mode === "Simple"`）不显示监测设置和卡片菜单项，但监测照常运行。
- 只处理 Routa 进程存活时的通知点击。不注册 COM 激活或 `routa://` 协议，不改安装器。
- 提交 `docs/` 下的文件要用 `git add -f`（全局 gitignore 忽略了 `docs/`）。
- 改了 Rust 类型或命令后要重新生成 `src/bindings.ts`：在 `src-tauri` 下运行 `cargo run -- --export-bindings`。

## Review Focus

1. 被监测节点正处于不可用状态时，订阅更新把它删掉或改了名：状态应该静默丢弃，不发通知，也不报错。见 Task 2 的测试 `reconcile_drops_removed_target_silently`。
2. 间隔 10 秒，同时监测几十个节点且都超时：同一时刻只能有一轮在跑，不能堆积。见 Task 4 的测试 `monitor_rounds_never_overlap`。
3. 一轮探测进行到一半时内核重启（结果是 `NoCore` 或 `CoreUnreachable`）：整轮作废，任何节点都不计失败。见 Task 2 的测试 `core_errors_void_the_round`。
4. 用户清空了「延迟上限」输入框：应该保存为 `null`（不限），不能存成 0 或报错。见 Task 6 的测试 `clearing_latency_limit_saves_null`。
5. 窗口最小化或关到托盘时点击通知：窗口要显示、取消最小化并获得焦点，然后再定位。见 Task 5 的步骤（复用 `tray::show_main_window`），以及 Task 7 的测试 `monitor_focus_navigates_and_highlights_node`。

---

### Task 1: 设置模型与校验

**Files:**
- Modify: `src-tauri/src/model/types.rs`（`Settings` 加字段；新增 `NodeMonitorSettings`、`MonitorScope`）
- Modify: `src-tauri/src/model/validate.rs`（在 `validate` 中检查监测设置）
- Test: `src-tauri/src/model/types.rs`、`src-tauri/src/model/validate.rs` 已有的 `#[cfg(test)] mod tests`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, specta::Type)]
  pub enum MonitorScope { #[default] InUse, Custom }

  #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
  #[serde(default)]
  pub struct NodeMonitorSettings {
      pub enabled: bool, pub scope: MonitorScope, pub members: Vec<ExitMember>,
      pub interval_secs: u32, pub fail_threshold: u32,
      pub latency_limit_ms: Option<u32>, pub notify_recovery: bool,
  }
  pub const MONITOR_INTERVALS: [u32; 8] = [10, 30, 60, 180, 300, 600, 1800, 3600];
  pub const MONITOR_THRESHOLDS: [u32; 4] = [1, 2, 3, 5];
  // Settings 新增：pub node_monitor: NodeMonitorSettings
  ```
  校验错误字段名：`node_monitor.interval_secs`、`node_monitor.fail_threshold`、`node_monitor.latency_limit_ms`（复用现有 `ValidationError` 中表示字段错误的变体，和 `check_interval` 用的一致）。

- [ ] **Step 1: 写失败的测试**
  - `node_monitor_defaults`：`Settings::default().node_monitor` 和 Global Constraints 里列的默认值逐项相等。
  - `old_state_without_node_monitor_loads_defaults`：反序列化一个不含 `node_monitor` 的 settings JSON，得到的 `node_monitor == NodeMonitorSettings::default()`。
  - `validate_rejects_bad_monitor_values`：`interval_secs = 5`、`fail_threshold = 4`、`latency_limit_ms = Some(0)`、`latency_limit_ms = Some(60001)` 时都返回 `Err`，并且包含对应字段名；`latency_limit_ms = None` 和 `Some(60000)` 时返回 `Ok`。
  - `validate_ignores_missing_monitor_members`：`members` 里引用了一个不存在的节点，`validate` 仍然返回 `Ok`。
- [ ] **Step 2: 运行测试，确认失败**
  Run: `cd src-tauri && cargo test --lib model::`
  Expected: 编译失败或测试 FAIL（`node_monitor` 未定义）。
- [ ] **Step 3: 实现。** 在 `types.rs` 加上述类型，`Default` 写成 Global Constraints 中的值，`Settings::default()` 补上 `node_monitor`。在 `validate.rs` 的 `validate()` 里、`check_interval` 之后加检查，非法值才报错。
- [ ] **Step 4: 运行测试，确认通过**
  Run: `cd src-tauri && cargo test --lib model::`
  Expected: PASS。
- [ ] **Step 5: 提交**
  ```bash
  git add src-tauri/src/model/types.rs src-tauri/src/model/validate.rs
  git commit -m "feat(monitor): node monitor settings model and validation"
  ```

### Task 2: 纯逻辑模块（目标展开、判定、通知文案）

**Files:**
- Create: `src-tauri/src/monitor/mod.rs`（类型 + 对外导出）
- Create: `src-tauri/src/monitor/targets.rs`（展开「正在使用」或「指定节点」）
- Create: `src-tauri/src/monitor/judge.rs`（`MonitorBook`：reconcile + 按轮判定 + 生成通知）
- Modify: `src-tauri/src/lib.rs`（`mod monitor;`）
- Test: 每个文件内的 `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: Task 1 的 `NodeMonitorSettings`、`MonitorScope`；现有的 `model::resolve::{resolve, Resolved, manual_proxy_name}`、`config_gen::{node_proxy_name, member_proxy_name}`、`model::hidden::rule_listener`、`model::health::{Health, ExitStatus}`、`stats::delay::{DelayResult, DelayError}`、`netif::NetInterface`。
- Produces:
  ```rust
  // mod.rs —— 以下类型都 derive Serialize/Deserialize/Clone/Debug/PartialEq/specta::Type
  pub struct TargetSpec { pub proxy: String, pub node: Option<NodeRef>, pub label: String, pub used_by: Vec<String> }
  pub enum MonitorFailure { Timeout, Failed, OverLimit(u32) }
  pub enum MonitorPause { CoreNotRunning }
  pub struct MonitorTarget {
      pub proxy: String, pub node: Option<NodeRef>, pub label: String, pub used_by: Vec<String>,
      pub fails: u32, pub down: bool, pub down_since: Option<i64>, pub last_ms: Option<u32>,
      pub last_failure: Option<MonitorFailure>, pub probed: bool, pub not_applied: bool,
  }
  pub struct MonitorStatus {
      pub last_run: Option<i64>, pub next_run: Option<i64>, pub probing: bool,
      pub paused_reason: Option<MonitorPause>, pub network_down: bool, pub targets: Vec<MonitorTarget>,
  }   // 实现 Default
  pub enum MonitorFocus { Node(NodeRef), Listeners, Settings }
  pub struct MonitorNotice { pub title: String, pub body: String, pub focus: MonitorFocus }

  // targets.rs
  pub fn monitor_targets(model: &Model, health: &Health, nics: &[NetInterface]) -> Vec<TargetSpec>;

  // judge.rs
  #[derive(Default)] pub struct MonitorBook { /* targets + network_fails + network_down */ }
  pub enum RoundOutcome { Applied(Vec<MonitorNotice>), Voided }
  impl MonitorBook {
      pub fn reconcile(&mut self, specs: &[TargetSpec]) -> bool;            // 集合或元数据变了返回 true
      pub fn apply_round(&mut self, results: &[(String, DelayResult)], cfg: &NodeMonitorSettings, now: i64) -> RoundOutcome;
      pub fn clear(&mut self);
      pub fn targets(&self) -> Vec<MonitorTarget>;                           // 按 reconcile 的顺序
      pub fn network_down(&self) -> bool;
  }
  pub fn format_down_for(secs: i64) -> String;   // "不到 1 分钟" / "约 15 分钟" / "约 2 小时 5 分钟"
  ```

**展开规则（`monitor_targets`）：**
- `scope = InUse`：
  - 收集被引用的出口：`visible_listeners()` 中 `user_enabled` 且 `exit` 不为空的监听器，使用者文案为 `"{listener.name} :{port}"`；`process_routing.enabled` 时，`user_enabled` 且 `node.is_some()` 的规则通过 `rule_listener` 找到对应出口，使用者文案为 `"进程分流 · {规则 name，为空时用第一个 process 的 value}"`。
  - 每个出口按类型处理：
    - `Node` → proxy 为 `exit.name`，node 为该节点。
    - `Group{Select, selected: Some(n)}` → 节点可复制时 proxy 为 `member_proxy_name(exit, base_name)`，Tailscale 节点为 `manual_proxy_name`；node 为 `n`。
    - 其它 Group → proxy 为 `exit.name`，node 为 `None`，label 为 `exit.name`。
    - `Direct` → 跳过。
  - 按 proxy 去重，合并 used_by。
  - 同一 node 对应多个 target 时，label 写成 `"{节点名} · 经 {网卡 friendly_name}"`，没有网卡的写 `"{节点名} · 经 默认网卡"`；只有一个 target 时 label 就是节点名。
- `scope = Custom`：`members` 展开为单个节点（整组按当前成员展开，已失效的跳过）。proxy 为 `node_proxy_name` 或 `manual_proxy_name`，`used_by` 为空，按 proxy 去重，保持 members 的顺序。
- `enabled = false` 时返回空。

**判定规则（`apply_round`）：** 严格按 spec 第 4 节实现，并补充以下细节：
- 结果分类：
  - `stale` → 该 target 本轮作废。
  - `error = NotFound` → 置 `not_applied = true`，本轮作废，不计数。
  - `NoCore` 或 `CoreUnreachable` 出现在任一结果中 → 返回 `Voided`，状态不变。
  - `ms = Some(v)` 且设置了上限、`v > limit` → `OverLimit(v)`。
  - `ms = Some` → 成功。
  - `Timeout` → `Timeout`。
  - `Failed` 和 `Other` → `Failed`。
- 通知文案按 spec 3.3 的表格逐字生成。原因文案：`超时`、`连接失败`、`延迟 {ms} ms 超过上限`。
- focus 规则：单个节点通知 → `Node(node)`，node 为 None 时为 `Listeners`；合并通知 → 第一个有 node 的 target，没有则为 `Listeners`；网络通知 → `Settings`。

- [ ] **Step 1: 写 `targets.rs` 的失败测试**
  - `in_use_node_exit_uses_exit_name`
  - `in_use_select_group_uses_member_proxy_name`
  - `in_use_auto_group_monitors_exit_itself`（UrlTest，node 为 None，label 为出口名）
  - `in_use_skips_direct_disabled_listener_and_routing_off`
  - `same_path_used_twice_is_one_target_with_both_users`（used_by 为 `["工作代理 :7890", "进程分流 · 微信"]`）
  - `same_node_two_interfaces_gets_suffixed_labels`（label 为 `"甲 · 经 Ethernet"` 和 `"甲 · 经 默认网卡"`）
  - `custom_expands_groups_and_skips_missing`
  - `disabled_returns_empty`
- [ ] **Step 2: 写 `judge.rs` 的失败测试**（`cfg` 为默认值，阈值 2）
  - `two_failures_mark_down_and_notify_once`：第 1 轮失败没有通知，第 2 轮出现 1 条 `节点不可用：甲`，正文为 `连续 2 次探测失败（超时） · 正在被「工作代理 :7890」使用`；第 3 轮仍然失败，不再通知。
  - `recovery_notifies_with_duration`：down_since 为 0，now 为 900 时成功 → 标题 `节点已恢复：甲`，正文 `不可用约 15 分钟，当前延迟 48 ms`；`notify_recovery = false` 时不发通知。
  - `over_limit_counts_as_failure`：ms 为 1280、上限 1000 → 原因文案为 `延迟 1280 ms 超过上限`。
  - `three_new_downs_merge_into_one`：标题 `3 个节点不可用`。另测 7 个节点时正文只列 5 个名字并以 `等` 结尾。
  - `all_failed_is_network_down_once`：3 个 target 全部超时，连续 2 轮 → 只有 1 条 `网络可能不可用`，正文 `3 个监测节点同时探测失败，请检查本机网络。`；各 target 的 `fails` 仍为 0。之后有一个 target 成功 → 发 `网络已恢复`，并且这一轮按正常规则处理。
  - `single_target_has_no_network_detection`：只有 1 个 target 时连续失败 → 走节点不可用的通知。
  - `over_limit_only_round_is_not_network_down`
  - `core_errors_void_the_round`：任一结果为 `NoCore` → 返回 `Voided`，所有 `fails` 不变。
  - `not_found_voids_only_that_target`
  - `stale_result_is_ignored`
  - `reconcile_drops_removed_target_silently`：处于 down 状态的 target 从 specs 中消失后，下一次 `apply_round` 没有关于它的通知，`targets()` 里也没有它。
  - `threshold_change_applies_next_round`：fails 为 1 时阈值从 2 改成 1，下一轮失败立即标记为 down。
  - `format_down_for_cases`：30 → `不到 1 分钟`，900 → `约 15 分钟`，7500 → `约 2 小时 5 分钟`。
- [ ] **Step 3: 运行测试，确认失败**
  Run: `cd src-tauri && cargo test --lib monitor::`
  Expected: 编译失败（模块不存在）。
- [ ] **Step 4: 实现** `mod.rs`、`targets.rs`、`judge.rs`，写上述签名。节点名用 `resolve`：订阅节点取 `name`，手动节点取 `ManualNode.name`。
- [ ] **Step 5: 运行测试，确认通过**
  Run: `cd src-tauri && cargo test --lib monitor::`
  Expected: PASS。
- [ ] **Step 6: 提交**
  ```bash
  git add src-tauri/src/monitor src-tauri/src/lib.rs
  git commit -m "feat(monitor): target expansion, round judging and notice copy"
  ```

### Task 3: Runtime 状态与「立即探测」命令

**Files:**
- Modify: `src-tauri/src/runtime/mod.rs`（`RuntimeStatus` 加 `monitor`；`Inner` 加 `monitor` 状态；`RuntimeEvent::Notify`；`test_delays` 改为接受超时参数；新增 `Runtime::monitor_probe_now`）
- Modify: `src-tauri/src/commands.rs`（`monitor_probe_now` 命令）
- Modify: `src-tauri/src/lib.rs`（注册命令；事件循环里先给 `Notify` 留个只打日志的分支，Task 5 再替换）
- Regenerate: `src/bindings.ts`

**Interfaces:**
- Consumes: Task 2 的 `MonitorBook`、`MonitorStatus`、`MonitorNotice`。
- Produces:
  ```rust
  pub enum RuntimeEvent { /* 现有 */, Notify(MonitorNotice) }
  // RuntimeStatus 新增：pub monitor: MonitorStatus
  // Inner 新增：monitor: Mutex<MonitorRun>
  struct MonitorRun { book: MonitorBook, force: bool, probing: bool,
                      last_end: Option<tokio::time::Instant>, last_run: Option<i64>,
                      settings_key: Option<NodeMonitorSettings> }
  impl Inner {
      async fn test_delays_with(&self, names: Vec<String>, timeout_ms: u32) -> Vec<(String, DelayResult)>;
      // 原 test_delays(names) 改为调用 test_delays_with(names, DEFAULT_TIMEOUT_MS)
      fn publish_monitor(&self);   // book.targets() 等快照写入 status.monitor，再 emit_status；不同时持有两把锁
  }
  impl Runtime { pub fn monitor_probe_now(&self); }   // 设置 force = true
  // 命令：#[tauri::command] #[specta::specta] pub async fn monitor_probe_now(rt: Rt<'_>) -> Result<(), ()>
  pub const MONITOR_TIMEOUT_MS: u32 = 5000;   // 放在 monitor/mod.rs
  ```

- [ ] **Step 1: 写失败的测试**（`runtime/tests.rs`）
  - `test_delays_with_passes_timeout`：调用 `test_delays_with(vec!["直连出口".into()], 5000)` 后，`api.delay_calls[0].2 == 5000`。
  - `status_has_default_monitor`：`rt.status().monitor == MonitorStatus::default()`。
- [ ] **Step 2: 运行，确认失败** `cd src-tauri && cargo test --lib runtime::tests::test_delays_with_passes_timeout`
- [ ] **Step 3: 实现上述接口**，并注册命令。
- [ ] **Step 4: 运行测试，确认通过** `cd src-tauri && cargo test --lib`（全部 PASS）
- [ ] **Step 5: 重新生成 bindings，检查类型**
  Run: `cd src-tauri && cargo run -- --export-bindings && cd .. && pnpm typecheck`
  Expected: `src/bindings.ts` 中出现 `NodeMonitorSettings`、`MonitorStatus`、`monitorProbeNow`；typecheck 报错只能出现在引用了 `Settings`、需要补 `node_monitor` 的测试 fixture 里，在这一步一并修好，直到 typecheck 通过。
- [ ] **Step 6: 提交**
  ```bash
  git add src-tauri/src src/bindings.ts src
  git commit -m "feat(monitor): runtime monitor status, notify event and probe-now command"
  ```

### Task 4: 监测循环

**Files:**
- Modify: `src-tauri/src/runtime/tasks.rs`（新增 `monitor_loop`，加入 `spawn_all`）
- Test: `src-tauri/src/runtime/tests.rs`

**Interfaces:**
- Consumes: Task 3 的 `MonitorRun`、`test_delays_with`、`publish_monitor`、`RuntimeEvent::Notify`；Task 2 的 `monitor_targets`、`MonitorBook`。
- Produces: `async fn monitor_loop(inner: Arc<Inner>)`，`const MONITOR_TICK: Duration = Duration::from_secs(1);`

**每个 tick 的流程：**
1. 读取 `settings.node_monitor`。如果 `!enabled`：book 不为空就 `clear()` 并发布状态，然后结束本 tick。
2. 根据 model、health、interfaces 计算 `monitor_targets`。若 `book.reconcile(..)` 返回 true，发布状态。
3. 没有 api，或 `last_apply` 不是 `Reloaded`：把 `paused_reason` 置为 `CoreNotRunning` 并发布（状态变化时才发布），然后结束本 tick。
4. 满足以下任一条件就是到期：`force`；`settings_key != Some(当前 enabled/scope/members/interval)`；`last_end` 为 None；`Instant::now() >= last_end + interval`。没有 target 时不跑。
5. 跑一轮：置 `probing = true` 并发布，然后调用 `test_delays_with(proxies, MONITOR_TIMEOUT_MS)`。结果传给 `apply_round(.., now_unix())`。如果是 `Applied(notices)`，逐条发送 `RuntimeEvent::Notify`，并设置 `last_run = now_unix()`。之后清掉 force，记下 settings_key，`last_end = Instant::now()`，`probing = false`，最后发布状态。`next_run = last_run + interval_secs`。
   - 时间约定：调度一律用 tokio `Instant`（暂停时钟下的测试会推进它），显示用的时间戳用 `now_unix()`。

- [ ] **Step 1: 写失败的测试**（`#[tokio::test(start_paused = true)]`，沿用 `harness` 和 `FakeApi::set_delay`）
  - `monitor_probes_immediately_when_enabled_then_on_interval`：用 `save_settings` 开启监测（间隔 10 秒，范围 InUse，`model_with_listener` 的出口）。推进 2 秒 → `delay_calls` 1 次，`timeout_ms` 为 5000；再推进 5 秒 → 仍然 1 次；再推进 6 秒 → 2 次。
  - `monitor_sends_notify_after_threshold`：出口 proxy 设成 `Err(Rejected{status:504})`，跑两轮后 `drain(rx)` 中恰好有 1 个 `RuntimeEvent::Notify`，标题以 `节点不可用：` 开头。
  - `monitor_skips_when_core_not_running`：未启动内核时开启监测并推进 30 秒 → `delay_calls` 为空，`status.monitor.paused_reason == Some(CoreNotRunning)`。
  - `monitor_rounds_never_overlap`：用 `delay_gate` 卡住第一轮，推进 60 秒 → `delay_calls` 只有 1 次；放行后推进 11 秒 → 2 次。
  - `probe_now_runs_without_waiting`：开启监测后推进 2 秒完成首轮，调用 `rt.monitor_probe_now()`，再推进 2 秒 → `delay_calls` 为 2 次。
  - `disabling_clears_targets`：关闭监测后推进 2 秒 → `status.monitor.targets` 为空，没有 Notify。
  - `settings_change_does_not_reapply`：只改 `node_monitor` → `last_reload(&api)` 的调用次数不变。
- [ ] **Step 2: 运行，确认失败** `cd src-tauri && cargo test --lib runtime::tests::monitor_`
- [ ] **Step 3: 实现 `monitor_loop`**，加入 `spawn_all`（`tokio::spawn(monitor_loop(inner.clone()))`）。
- [ ] **Step 4: 运行测试，确认通过** `cd src-tauri && cargo test --lib`（全部 PASS）
- [ ] **Step 5: 提交**
  ```bash
  git add src-tauri/src/runtime
  git commit -m "feat(monitor): scheduled monitor loop with immediate first round"
  ```

### Task 5: Windows 通知与点击唤起

**Files:**
- Modify: `src-tauri/Cargo.toml`（Windows 目标依赖加 `tauri-winrt-notification`）
- Create: `src-tauri/src/notify.rs`
- Modify: `src-tauri/src/lib.rs`（`RuntimeEvent::Notify` 分支调用 `notify::show`；`mod notify;`）

**Interfaces:**
- Consumes: Task 2 的 `MonitorNotice`、`MonitorFocus`；`tray::show_main_window(&AppHandle)`。
- Produces: `pub fn show(app: &AppHandle, notice: MonitorNotice)`；前端事件 `"monitor-focus"`，payload 为 `MonitorFocus`。

- [ ] **Step 1: 实现 `notify::show`。**
  - 用 `tauri_winrt_notification::Toast::new(app_id)`，标题、正文取自 notice，`.on_activated(..)` 回调里：先 `tray::show_main_window(&app)`，再 `app.emit("monitor-focus", focus.clone())`。
  - `app_id` 用 `app.config().identifier`（`dev.routa`）。debug 构建或显示失败时，退回 `Toast::POWERSHELL_APP_ID` 再试一次；仍失败就 `eprintln!` 记录，不 panic。
  - 非 Windows 平台给一个空实现（`#[cfg(not(windows))]`）。
  - 先确认 crate 版本支持 `on_activated`。如果不支持，改用 `windows` crate 的 `UI_Notifications` + `Data_Xml_Dom` feature 直接调用 `ToastNotificationManager::CreateToastNotifierWithId` 和 `ToastNotification::Activated`，对外接口不变。
- [ ] **Step 2: 编译检查** `cd src-tauri && cargo build`，Expected：成功。
- [ ] **Step 3: 手动验证（安装包）**
  Run: `pnpm tauri build`，安装后开启监测（间隔 10 秒），把一个被监测节点改成不可达（例如手动节点改为 `127.0.0.1:1`）。
  Expected：
  - 约 20 秒内右下角出现「节点不可用：…」，应用名显示为 Routa。
  - 关闭窗口到托盘后点击通知，窗口重新出现并获得焦点（前端定位在 Task 7 完成后再验收）。
  - 把节点改回可达后出现「节点已恢复：…」。
  - 记录 NSIS 安装后 AppUserModelID 是否就是 `dev.routa`；如果不是，在 `notify.rs` 里改用实际值并注释说明。
- [ ] **Step 4: 提交**
  ```bash
  git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/notify.rs src-tauri/src/lib.rs
  git commit -m "feat(monitor): windows toast notifications with click-to-focus"
  ```

### Task 6: 设置页「节点监测」分区

**Files:**
- Create: `src/lib/monitor.ts`
- Create: `src/components/NodeMonitorSection.tsx`
- Modify: `src/pages/Settings.tsx`（在 network 面板中，`<Section title="测速">` 之后插入 `<NodeMonitorSection />`）
- Test: `src/lib/monitor.test.ts`、`src/components/NodeMonitorSection.test.tsx`

**Interfaces:**
- Consumes: bindings 中的 `NodeMonitorSettings`、`MonitorStatus`、`MonitorTarget`、`commands.monitorProbeNow`；现有的 `useSettingsSave`、`useSaveFeedback`、`SaveFeedback`、`Row`、`Section`、`MemberPicker`、`useApplyResult().showMessage`、`useModel`、`useStatus`、`lib/refs` 的 `nodeKey`。
- Produces（`src/lib/monitor.ts`）：
  ```ts
  export const MONITOR_INTERVALS: number[];                 // [10,30,60,180,300,600,1800,3600]
  export function intervalLabel(secs: number): string;     // "10 秒" / "5 分钟" / "1 小时"
  export function customMembersFrom(status: MonitorStatus): { members: ExitMember[]; dropped: string[] };
  export function withNode(s: NodeMonitorSettings, status: MonitorStatus, node: NodeRef): { settings: NodeMonitorSettings; dropped: string[] };
  export function withoutNode(s: NodeMonitorSettings, status: MonitorStatus, node: NodeRef): { settings: NodeMonitorSettings; dropped: string[] };
  export function targetsForNode(status: MonitorStatus, node: NodeRef): MonitorTarget[];
  export function failureText(f: MonitorFailure): string;  // 超时 / 连接失败 / 延迟 N ms 超过上限
  export function droppedMessage(dropped: string[]): string; // 自动选择的出口「A」、「B」不能加入指定节点，已不再监测
  ```
  `withNode` 和 `withoutNode` 的规则见 spec 3.2；切换到「指定节点」的预填规则见 spec 3.1。

**UI 规格：** 行、文案、选项、footer 按 spec 3.1 和原型 `designs/node-monitor-v1/preview.tsx` 的 `MonitorSection` 实现，但不复制原型里的 store，数据全部来自 model 和 `status.monitor`。

- 每项改动即时保存，`useSaveFeedback` 的 key 依次为 `"monitorEnabled" | "monitorScope" | "monitorMembers" | "monitorInterval" | "monitorThreshold" | "monitorLatency" | "monitorRecovery"`。
- 延迟上限在 blur 或 Enter 时提交，空字符串保存为 `null`。非 1–60000 的整数显示 helperText「请输入 1–60000 的整数」，不保存。
- footer：
  - 暂停时显示「内核未运行，监测已暂停」，按钮禁用。
  - 列表次要信息：InUse 时 node 为 null 显示 `自动选择 · ${used_by.join("，")}`，否则显示 `used_by.join("，")`；Custom 时显示 `proxy` 第一个 `/` 之前的部分。
  - 状态文字：未 probed →「等待探测」；not_applied →「出口未生效」；down →「不可用 · 原因」；fails>0 →「失败 n/阈值」；否则「N ms」。

- [ ] **Step 1: 写 `monitor.test.ts` 的失败测试**
  - `intervalLabel`：10 → `10 秒`，180 → `3 分钟`，3600 → `1 小时`。
  - `customMembersFrom drops auto groups`：status 有 2 个 target（一个有 node，一个 node 为 null、label 为「工作出口」）→ members 只含前者，dropped 为 `["工作出口"]`；同一 node 的两个 target 只产出 1 个 member。
  - `withNode` 的三种情况：关闭时 → `{enabled:true, scope:"Custom", members:[node]}`；InUse 时 → 正在使用的节点加上该 node；Custom 时 → 追加，已存在则不重复。
  - `withoutNode`：Custom 范围下 members 含整组 → 结果按 `status.targets` 展开成单节点，并去掉该 node。
  - `droppedMessage(["工作出口"])` 等于 `自动选择的出口「工作出口」不能加入指定节点，已不再监测`。
- [ ] **Step 2: 写 `NodeMonitorSection.test.tsx` 的失败测试**（参照 `src/pages/Settings.test.tsx` 对 `commands` 的 mock 方式）
  - `hides rows when disabled`：只出现「节点监测」开关，看不到「监测范围」。
  - `toggle saves enabled`：点击开关后，`saveSettings` 收到的 `node_monitor.enabled === true`。
  - `interval select offers 10 seconds`：打开「探测间隔」下拉框，有「10 秒」，选中后保存的是 `interval_secs: 10`。
  - `switching to custom prefills in-use nodes`：status.monitor 中有 2 个有 node 的 target → 选「指定节点」后保存的 members 长度为 2。
  - `clearing_latency_limit_saves_null`：清空输入框并 blur → 保存 `latency_limit_ms: null`。
  - `invalid latency is not saved`：输入 `0` 后 blur → 显示「请输入 1–60000 的整数」，不调用 `saveSettings`。
  - `footer shows summary and probe button`：last_run 有值，1 个 down target → 文本含「1 不可用」；点「立即探测」调用 `monitorProbeNow`。
  - `paused footer`：`paused_reason: "CoreNotRunning"` → 显示「内核未运行，监测已暂停」，按钮禁用。
  - `target row texts`：分别覆盖「等待探测」「出口未生效」「不可用 · 超时」「失败 1/2」「48 ms」。
- [ ] **Step 3: 运行，确认失败** `pnpm vitest run src/lib/monitor.test.ts src/components/NodeMonitorSection.test.tsx`
- [ ] **Step 4: 实现** `src/lib/monitor.ts`、`NodeMonitorSection.tsx`，并插入 Settings。
- [ ] **Step 5: 运行，确认通过** `pnpm vitest run src/lib/monitor.test.ts src/components/NodeMonitorSection.test.tsx src/pages/Settings.test.tsx && pnpm typecheck`（全部 PASS）
- [ ] **Step 6: 提交**
  ```bash
  git add src/lib/monitor.ts src/lib/monitor.test.ts src/components/NodeMonitorSection.tsx src/components/NodeMonitorSection.test.tsx src/pages/Settings.tsx
  git commit -m "feat(monitor): node monitor settings section"
  ```

### Task 7: 节点卡片标记、菜单项与点击通知定位

**Files:**
- Create: `src/components/MonitorMark.tsx`
- Create: `src/api/monitorFocus.ts`（小型 store：`setMonitorFocus` / `useMonitorFocus` / `clearMonitorFocus`）
- Modify: `src/api/events.ts`（监听 `"monitor-focus"` 并写入 store）
- Modify: `src/components/Layout.tsx`（新增 `useMonitorFocusNavigation()`：Node → 跳到 `/nodes`；Listeners → 跳到 `/listeners`；Settings → 跳到 `/settings?tab=network`）
- Modify: `src/pages/ResourceCenter.tsx`（卡片 `status` 和菜单项；收到 focus 时展开对应分组、滚动并高亮）
- Test: `src/components/MonitorMark.test.tsx`、`src/pages/ResourceCenter.test.tsx`、`src/api/events.test.tsx`

**Interfaces:**
- Consumes: Task 6 的 `withNode`、`withoutNode`、`targetsForNode`、`droppedMessage`；Task 5 的 `"monitor-focus"` 事件（payload 为 `MonitorFocus`）。
- Produces: `export function MonitorMark({ targets }: { targets: MonitorTarget[] }): JSX.Element | null`
  - 任一 target 为 down → 显示静音铃铛 +「不可用」（`error.main`），tooltip 为 `不可用 · 连续失败 N 次`，多路径时列出每条路径 label。
  - 否则显示铃铛 +「监测」（`primary.main`），tooltip「监测中」。
  - targets 为空时返回 `null`。

- [ ] **Step 1: 写失败的测试**
  - `MonitorMark.test.tsx`：没有 target 时不渲染；有 down target 时 aria-label 为「监测：不可用」；正常时为「监测中」。
  - `ResourceCenter.test.tsx`：
    - `monitored node shows mark`：status.monitor 有该节点的 target，对应卡片里有「监测中」标记；如果卡片已有 Tailscale 状态，就不显示监测标记。
    - `menu adds node to monitor`：未监测节点的「⋯」菜单里有「加入监测」，点击后 `saveSettings` 收到的 `node_monitor` 为 `{enabled:true, scope:"Custom", members:[该节点]}`（初始为关闭）。
    - `menu removes node and reports dropped auto group`：InUse 范围下有一个自动组 target → 点「移出监测」后 members 不含该节点，并出现 Snackbar 文案 `自动选择的出口「工作出口」不能加入指定节点，已不再监测`。
    - `simple mode has no monitor menu item`。
    - `monitor_focus_navigates_and_highlights_node`：`setMonitorFocus({ Node: ref })` 后，该节点所在的折叠分组被展开，卡片被 `scrollIntoView`（mock）并带上 `data-monitor-focus="true"`；3 秒后该属性被移除（fake timers）。
  - `events.test.tsx`：触发 `"monitor-focus"` 事件后，`useMonitorFocus()` 返回该 payload。
- [ ] **Step 2: 运行，确认失败** `pnpm vitest run src/components/MonitorMark.test.tsx src/pages/ResourceCenter.test.tsx src/api/events.test.tsx`
- [ ] **Step 3: 实现。**
  - 菜单项追加在 `actions` 末尾，只在 `!simple` 时添加。
  - 保存走 `useSettingsSave`，传 `{ node_monitor: result.settings }`；`dropped` 非空时调用 `showMessage(droppedMessage(dropped), "info")`。
  - 高亮用 MUI 的 `outline`，3 秒后清除，处理完调用 `clearMonitorFocus()`。
- [ ] **Step 4: 运行，确认通过** `pnpm test && pnpm typecheck`（全部 PASS）
- [ ] **Step 5: 提交**
  ```bash
  git add src/components/MonitorMark.tsx src/components/MonitorMark.test.tsx src/api/monitorFocus.ts src/api/events.ts src/api/events.test.tsx src/components/Layout.tsx src/pages/ResourceCenter.tsx src/pages/ResourceCenter.test.tsx
  git commit -m "feat(monitor): node card marks, monitor menu items and toast focus"
  ```

### Task 8: 端到端验收与收尾

**Files:**
- Modify: `ACCEPTANCE.md`（新增「节点监测」验收条目，格式和现有条目一致）

- [ ] **Step 1: 全量检查** `cd src-tauri && cargo test && cargo clippy --all-targets -- -D warnings && cd .. && pnpm typecheck && pnpm test && pnpm build`，Expected：全部通过。
- [ ] **Step 2: 安装包手动验收**（`pnpm tauri build` 后安装）。逐项对照原型 `designs/node-monitor-v1/` 确认：
  - 设置分区的外观与各行；
  - 「正在使用」的使用者文案；
  - 节点卡片标记和菜单项；
  - 节点不可用、恢复、合并、断网四种通知（断网可以拔网线或禁用网卡模拟）；
  - 点击通知后定位到节点（窗口在托盘时也要能定位）；
  - 修改设置不会让 mihomo 重启（连接不断）。
- [ ] **Step 3: 把验收条目写入 `ACCEPTANCE.md`**，并提交
  ```bash
  git add ACCEPTANCE.md
  git commit -m "docs(acceptance): node monitor"
  ```

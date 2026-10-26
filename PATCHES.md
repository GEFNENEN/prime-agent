# 补丁清单（实时）

本文件只描述 **当前 `custom` 分支相对上游基线的补丁**，以 HEAD 为准。
补丁被移除或合并进上游后，对应条目直接删除，不保留历史记录（历史见
`git log main..custom`）。

- 基线：`v0.10.2-beta.10`（上游 `c4c238ca0` 的 tag-only 提交，`main` 当前头）
- 补丁数：**16**
- 维护流程见 [MAINTAINING.md](MAINTAINING.md)

| # | 提交 | 类型 | 摘要 |
| --- | --- | --- | --- |
| 1 | `9e4335a76` | fix | 顶栏始终显示会话工作目录 |
| 2 | `a1e9e51d2` | fix | 内核忙时等待更久（5s → 30s） |
| 3 | `77708a80b` | fix | 从 agent 主题目录解析自定义主题 |
| 4 | `ba9936cb1` | fix | 分离进程不再弹出控制台窗口 |
| 5 | `4e54b50cb` | feat | 每个被接纳的 turn 前运行 turnHook |
| 6 | `be481d871` | feat | 运行结束后通过 stopHook 写回 |
| 7 | `2c5fa729c` | fix | 在运行结束前同步 spawn stopHook |
| 8 | `63da66dbf` | feat | 用 updateGuard 拦截自更新 |
| 9 | `0a31a298f` | fix | Windows 剪贴板走 UTF-8 控制台 |
| 10 | `87c6ff653` | fix | 所有多行粘贴折叠为标记 |
| 11 | `db9606852` | feat | 新增 clean-kernel-state 命令 |
| 12 | `8ed36305c` | fix | 空回复自动重试 + 流空闲超时 |
| 13 | `c1ff59958` | fix | 停滞的子代理重新派活 + 重试提示按回合隔离 |
| 14 | `99e5e75a0` | fix | 停滞通知只发给父代理一次 |
| 15 | `9428c8307` | fix | 收养无 bootstrap 记录的 kernel venv |
| 16 | `b9f39406d` | fix | 空回复重试前固定等 3 秒 |

补丁之外还有文档与版本戳提交（`a7207133a`、`8a860c7d5`、`84706f55f`、
`7186e2728`、`5e383efbc`、`15a825082`），以及两个尚未定性的修复：

- `8c7b1dd85` / `0581b7fef`（refine 的机器可解析与空回复诊断）——保留。
  上游 `c4c238ca0` 之后 `default_refiner_call` 多了 `session_id` 参数，
  我们的改动加在函数体内，位置不冲突。
- `1e9266975`（daemon 把 provider 头贴到 `Model` 上给直连接缝用）——**待定**。
  上游 `c4c238ca0` 已用 `StreamOptions.session_id` + `opencode_headers::apply`
  覆盖了同样的 400 `MissingSessionID` 场景（摘要、refine、分支摘要都带 id
  且走 SSE）。两者机制不同、可以共存，但我们的可能已经多余。需要一次
  真实 `/refine` 调用确认后再决定删除。

---

## 0. 已移交上游（不要再当本地补丁）

**Authorization 去重**：原 `1004b1698` 已删除，改由上游 `85755cb8a` 承担。
上游的做法是「默认 bearer 先入列，model/caller header 大小写不敏感地覆盖它」，
与我们「删掉已有再写 api_key」相反：`authHeader: true` 时**显式 header 赢**，
而不是强制用 `apiKey`。本机 12 个 provider 全是 `authHeader: false`，
正常路径不带 `Authorization` 头部，两者行为等价。

**`ipython_state` 封顶与去重**：由上游 `e2bcc6add` 承担。名字清单封顶
200 个 / 4KB（超出加 `+N more`），`build_session_context` 只保留最新一份，
摘要调用不再接收该行。这条直接修掉了 p8 会话「压缩后地板过高 → 连环压缩」的病。

**opencode 会话身份**：由上游 `c4c238ca0` 承担。`session_id` 进入
`StreamOptions`，`opencode_headers::apply` 为 `opencode`/`opencode-go`
补 `User-Agent` 与 `x-opencode-session`，且 caller header 优先。

## 1. 顶栏显示 cwd（`9e4335a76`）

**行为**：会话顶栏的显示名附带当前工作目录，便于区分同名会话。

**文件**：`crates/pa-tui/src/session_ui.rs`

## 2. 内核忙等待 30s（`a1e9e51d2`）

**问题**：中断或快速切换后内核仍忙时，5 秒超时会过早报 "kernel busy"。

**行为**：`KERNEL_BUSY_REUSE_WAIT_MS` 由 `5_000` 改为 `30_000`。

**文件**：`crates/pa-core/src/kernel/shared.rs`

## 3. 自定义主题（`77708a80b`）

**问题**：0.10.0 的 `load_theme` 硬编码只认 `prime/dark/light`，未知名字静默
回退；`theme.rs` 里已有的 `load_theme_from_path` 没有任何调用者。

**行为**：新增 `resolve_theme(name, mode)`：先查内置名，未命中则读取
`<agent_dir>/themes/<name>.json`（沿用既有解析器）。

**文件**：`crates/pa-tui/src/app.rs`、`crates/pa-tui/src/theme.rs`、
`crates/pa-tui/src/session_ui/settings.rs`

## 4. 黑窗修复（`ba9936cb1`）

**问题**：Windows 上 `DETACHED_PROCESS` 会静默取消 `CREATE_NO_WINDOW`，
后台进程（MCP、更新器）各自弹出可见控制台窗口。

**行为**：分离 spawn 统一使用
`CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`，不再使用 `DETACHED_PROCESS`。

**文件**：`crates/pa-core/src/platform/process.rs`、
`crates/pa-core/src/update/installer.rs`

**验证**：spawn 前基线枚举窗口，批量触发 MCP/更新路径 0 弹窗。

## 5. turnHook（`4e54b50cb`）

**行为**：`settings.json` 配置
`"turnHook": {"command": "...", "timeoutMs": 30000}`。每个被接纳的 turn 在
首个 LLM 请求前执行命令（stdin 收 `{prompt, cwd, session_id}`），stdout 的
`hookSpecificOutput.additionalContext` 追加进本轮 system prompt（请求级，
不写入会话历史）。

**文件**：`crates/pa-core/src/session_engine/{turn_hook,engine,wiring,mod,admission}.rs`、
`crates/pa-core/src/settings/{types,load,mod}.rs`、`crates/pa-agent/src/agent.rs`

**验证**：`cargo test -p pa-core turn_hook --lib`（10 项）+ e2e 假 provider
确认注入标记出现在请求体中、每 turn 一次。

## 6. stopHook（`be481d871`）

**行为**：`settings.json` 配置
`"stopHook": {"command": "...", "timeoutMs": 70000}`。运行结束时把本次转录
按 dcode 格式（`{"schema_version":1,"role":...}` 每行一条）写到
`<agent_dir>/stop-hook/<session_id>.jsonl`，并向命令 stdin 传
`{session_id, transcript_path, cwd}`。用于把对话写回外部记忆系统
（如 hindsight）。

**文件**：`crates/pa-core/src/session_engine/{stop_hook,engine,wiring,mod}.rs`、
`crates/pa-core/src/settings/{types,load,mod}.rs`

**验证**：`cargo test -p pa-core stop_hook --lib`（8 项）+ e2e 假 hindsight
服务收到 `POST /v1/default/banks/<bank>/memories`。

## 7. stopHook 同步 spawn（`2c5fa729c`）

**问题**：在 `tokio::spawn` 的分离任务里启动 hook，一次性运行时会在任务
被取消前退出，命令从未执行。

**行为**：改为在 `agent_end` 监听器内同步 spawn 进程，另起
`stop-hook-reaper` 线程负责轮询退出与超时 kill。

**文件**：`crates/pa-core/src/session_engine/{stop_hook,turn_hook,wiring}.rs`

## 8. updateGuard（`63da66dbf`）

**行为**：`settings.json` 出现 `"updateGuard": {"hold": true, "message": "..."}`
时拦截 `/update`、`prime-agent update` 与包自更新（`hold:false` 放行）；
`--check` 仍可用。用于防止 `/update` 用官方包覆盖定制构建。

**文件**：`crates/pa-core/src/settings/{types,load,manager,mod}.rs`、
`crates/pa-cli/src/{self_update,client_update,client_settings,public_command}.rs`、
`crates/pa-tui/src/{client_settings,session_ui/commands}.rs`

## 9. 剪贴板中文乱码（`0a31a298f`）

**问题**：Windows 的 `clip.exe` 按控制台代码页（部分中文系统为 936/GBK）
解释 stdin，UTF-8 中文粘贴后变乱码。

**行为**：通过隐藏的 `cmd /d /s /c "chcp 65001>nul & clip"` 管道写入，
强制 UTF-8 代码页；不使用 UTF-16LE+BOM（会污染 ASCII 文本）。

**文件**：`crates/pa-tui/src/clipboard.rs`

## 10. 多行粘贴折叠（`87c6ff653`）

**行为**：任何多行粘贴（≥2 行）都折叠为 `[paste #N +M lines]` 标记，
展开后仍是原文；原阈值 ≥10 行只对超长粘贴生效。

**文件**：`crates/pa-tui/src/editor/mod.rs`

## 11. clean-kernel-state（`db9606852`）

**行为**：新增命令

```text
prime-agent clean-kernel-state [--days <n>] [--dry-run] [--json]
```

递归删除 `<agent_dir>/session-artifacts/**` 下 mtime 超过阈值（默认 1 天）的
`kernel-state.dill`、`kernel-state.json` 与 `kernel-state.*.tmp`。不删除会话、
`scheduled-jobs.json` 或目录结构；不跟随符号链接；占用中的文件只上报。

**文件**：`crates/pa-cli/src/{clean_kernel_state,command_registry,lib,public_command}.rs`

**验证**：6 项单测 + `cargo test -p pa-cli --lib`；实测 dry-run 与正式清理
均正常（可回收数百个文件、数 GB 空间）。

## 12. 空回复重试 + 流空闲超时（`8ed36305c`）

**问题 A**：模型返回 `stopReason=stop` 但内容只有 thinking（无文本、无工具
调用）时，prime-agent 视为正常结束，用户看到"突然停住"。在一份真实会话
日志中观察到 120 次静默结束、17 次用户随后发 `?`/`继续`。

**问题 B**：响应头到达后若连接半死（无 RST），`next_text/next_bytes` 会永久
等待；`timeout_ms` 只覆盖响应头阶段。

**行为**：
- A：daemon 的 turn 循环检测空回复 → 移除已追加的空 assistant → 重发一次，
  并附加请求级提示（"上一条为空，继续任务"，不写入会话历史）；仍为空则发出
  `model_prompt_error` 警告行（TUI 显示为黄色状态）。
- B：流式 chunk 读取加 120s 空闲超时，超时转为
  `ProviderError::Connection{kind: Timeout}`，交给既有重试策略。

**文件**：`crates/pa-daemon/src/agent_engine/turn/model.rs`、
`crates/pa-core/src/session_engine/{retry_nudge,engine,wiring,mod}.rs`、
`crates/pa-ai/src/utils_inner/http.rs`
（测试：`crates/pa-daemon/src/agent_engine/tests/empty_reply.rs`）

**验证**：`cargo test -p pa-daemon empty_reply --lib`（真实 HTTP/SSE：重试请求
携带提示、最终输出正常、提示不落会话文件、连续空回复只产生 1 条警告）。

**限制**：`-p` 打印模式不经过该 daemon 路径，空回复重试仅对 TUI 生效；
流空闲超时两条路径都生效。

## 13. 停滞子代理重新派活 + 重试提示按回合隔离（`c1ff59958`）

**问题 A**：补丁 13 的重试仍失败时，顶层会话会显示警告行，但子代理只会变
为 idle。子代理仅在 **settle**（worker 队列清空）时由父代理收到
`CompletedWithoutReply`；常驻子代理空回复后不触发 settle，父代理永远等不到
结果，用户看到子代理"默默停住"。

**问题 B**：重试提示存在 `Option<String>` 槽里，只由下一次请求的 transform
取走。若提示入槽后、重试请求发出前发生 abort 或 worker 关闭，槽中残留的
提示会混进之后某次无关请求。

**行为**：
- A：检测到空回复且重试已用尽时，除发出警告行外，若当前引擎的
  `rlm_depth > 0`（子代理），通过 `AgentMessageController` 找到父代理
  （roster + durable 父子边）并发一条 agent message 说明"本轮空停、上下文
  未丢、重新派活即可继续"。警告文案按顶层/子代理区分。
- B：每次 attempt 开始时清空提示槽（`clear_retry_nudge`），被放弃的重试
  不会把提示泄漏给后续回合。

**文件**：`crates/pa-daemon/src/agent_engine/turn/model.rs`、
`crates/pa-daemon/src/agent_engine/{config,lifecycle}.rs`、
`crates/pa-daemon/src/agent_engine.rs`、
`crates/pa-core/src/session_engine/wiring.rs`
（测试：`crates/pa-daemon/src/agent_engine/tests/empty_reply.rs`）

**验证**：`cargo test -p pa-daemon empty_reply --lib`（4 项：重试携带提示、
连续空回复告警、子代理通知父代理、残留提示不泄漏）；
`cargo test -p pa-core retry_nudge --lib`（3 项）。

**限制**：同补丁 13，`-p` 模式不经过 daemon 路径，此项仅对 TUI 生效。

## 14. 停滞通知只发给父代理一次（`99e5e75a0`）

**问题**：补丁 14 的通知无条件发送。子代理每开始一个新 turn 又空回复时都会再发
一次，父代理收到通知后回消息、子代理再跑一轮又空又发——形成正反馈，父代理被
同一句话刷屏，即使该子代理已被从 roster 删除。

**行为**：引擎新增 `stalled_run_announced: AtomicBool`。通知只在未置位时发送并
置位；投递失败则复位（一次网络抖动不能永久静默）；出现可见回复也复位（说明停滞
已解除，下次停滞应重新告知）。

**文件**：`crates/pa-daemon/src/agent_engine.rs`、
`crates/pa-daemon/src/agent_engine/{lifecycle,turn/model}.rs`
（测试：`crates/pa-daemon/src/agent_engine/tests/empty_reply.rs`）

**验证**：`cargo test -p pa-daemon empty_reply --lib`（5 项，新增
`a_repeat_stall_does_not_flood_the_parent`：连续两轮空回复只产生 1 条通知，
但每轮各 1 条警告行）。

## 15. 收养无 bootstrap 记录的 kernel venv（`9428c8307`）

**问题**：`.bootstrap-version` 是 bootstrap 最后一步写的。若 bootstrap 在装完
运行时、写记录前被杀（部署重启、多 worker 同时启动都常见），会留下一个完整可用
但没有记录的 venv。此后每次启动都判定 venv 过期并 `remove_dir_all`；只要还有别的
worker 的 kernel python 在该目录里运行，删除就会以"拒绝访问"失败，并永久重复——
表现为所有 `python_repl` 调用在几毫秒内失败。

**行为**：
- 记录缺失时先探测现有 venv：若其 python 能 import prime-agent runtime 与
  `dill`，则视为可用，补写缺失的记录并继续使用，不再删除。
- 探测不通过时才重建；重建前若旧目录删不掉（仍有 kernel python 在运行），直接报错说明阻塞原因，不把半删状态的目录交给 `uv venv`（它只会报 "already exists" 掩盖真因）。

**文件**：`crates/pa-core/src/kernel/bootstrap/mod.rs`、
`crates/pa-core/src/kernel/bootstrap/venv.rs`、
`crates/pa-core/src/kernel/bootstrap/venv/probe.rs`
（测试：`crates/pa-core/src/kernel/bootstrap/venv/tests.rs`、
`crates/pa-core/src/kernel/bootstrap/mod.rs`）

**验证**：`cargo test -p pa-core bootstrap:: --lib`（32 项，新增收养成功、
运行时损坏不收养、已有记录不动、无记录但可用的 venv 绝不被删除）。

## 16. 空回复重试前固定等 3 秒（`b9f39406d`）

**问题**：补丁 13 的重试是立即重发。空回复多由 provider 瞬时抖动引起，立刻重试
往往撞上同一个抖动，等于白耗掉唯一一次重试。

**行为**：重试前固定等待 3 秒（`EMPTY_REPLY_RETRY_DELAY_MS`）。用固定值而非指数
退避，因为只有一次重试。等待可取消：暂停期间收到 abort 会立即返回中止消息，不会
硬撑着再发请求。

**文件**：`crates/pa-daemon/src/agent_engine/turn/model.rs`

**验证**：`cargo test -p pa-daemon empty_reply --lib`（5 项；整组耗时从 0.11s
增至约 6.2s，证明等待真实生效）。

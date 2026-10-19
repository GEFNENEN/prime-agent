# 补丁清单（实时）

本文件只描述 **当前 `custom` 分支相对上游基线的补丁**，以 HEAD 为准。
补丁被移除或合并进上游后，对应条目直接删除，不保留历史记录（历史见
`git log main..custom`）。

- 基线：`v0.10.1-beta.35`（上游 tag-only 提交 `eb11e0c7a`，即 `main` 当前头）
- 补丁数：**13**
- 维护流程见 [MAINTAINING.md](MAINTAINING.md)

| # | 提交 | 类型 | 摘要 |
| --- | --- | --- | --- |
| 1 | `ea43fc292` | fix | 不再重复发送 Authorization 头 |
| 2 | `d3f6917a0` | fix | 顶栏始终显示会话工作目录 |
| 3 | `8a626643e` | fix | 内核忙时等待更久（5s → 30s） |
| 4 | `fd7779ec0` | fix | 从 agent 主题目录解析自定义主题 |
| 5 | `207b616b5` | fix | 分离进程不再弹出控制台窗口 |
| 6 | `70fcc6ed9` | feat | 每个被接纳的 turn 前运行 turnHook |
| 7 | `5d4df7fc3` | feat | 运行结束后通过 stopHook 写回 |
| 8 | `aae1659ad` | fix | 在运行结束前同步 spawn stopHook |
| 9 | `b1c355e60` | feat | 用 updateGuard 拦截自更新 |
| 10 | `5e2e7c7e6` | fix | Windows 剪贴板走 UTF-8 控制台 |
| 11 | `a9807e007` | fix | 所有多行粘贴折叠为标记 |
| 12 | `513830af0` | feat | 新增 clean-kernel-state 命令 |
| 13 | `d7410216f` | fix | 空回复自动重试 + 流空闲超时 |

---

## 1. Authorization 去重（`ea43fc292`）

**问题**：`openai-completions` / `openai-responses` 两条路径无条件追加
`Authorization: Bearer ...`，而 provider 配置（`authHeader: true`）已经注入过
一次。边缘网关对重复头返回 `400`（响应体为 CDN 错误页），表现为所有 provider
"同时不可用"。

**行为**：发请求前先按大小写不敏感删除已有的 `Authorization`，再写入唯一值。
根因是 Rust 移植把 TS 的 `HeaderMap`（同名覆盖）改成了裸 `Vec<(String, String)>`。

**文件**：`crates/pa-ai/src/providers/openai_completions/params.rs`、
`crates/pa-ai/src/providers/openai_responses.rs`

**验证**：`cargo test -p pa-ai authorization_header --lib`；e2e 抓包确认每请求
恰好 1 个头（此前 2 个）。

## 2. 顶栏显示 cwd（`d3f6917a0`）

**行为**：会话顶栏的显示名附带当前工作目录，便于区分同名会话。

**文件**：`crates/pa-tui/src/session_ui.rs`

## 3. 内核忙等待 30s（`8a626643e`）

**问题**：中断或快速切换后内核仍忙时，5 秒超时会过早报 "kernel busy"。

**行为**：`KERNEL_BUSY_REUSE_WAIT_MS` 由 `5_000` 改为 `30_000`。

**文件**：`crates/pa-core/src/kernel/shared.rs`

## 4. 自定义主题（`fd7779ec0`）

**问题**：0.10.0 的 `load_theme` 硬编码只认 `prime/dark/light`，未知名字静默
回退；`theme.rs` 里已有的 `load_theme_from_path` 没有任何调用者。

**行为**：新增 `resolve_theme(name, mode)`：先查内置名，未命中则读取
`<agent_dir>/themes/<name>.json`（沿用既有解析器）。

**文件**：`crates/pa-tui/src/app.rs`、`crates/pa-tui/src/theme.rs`、
`crates/pa-tui/src/session_ui/settings.rs`

## 5. 黑窗修复（`207b616b5`）

**问题**：Windows 上 `DETACHED_PROCESS` 会静默取消 `CREATE_NO_WINDOW`，
后台进程（MCP、更新器）各自弹出可见控制台窗口。

**行为**：分离 spawn 统一使用
`CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`，不再使用 `DETACHED_PROCESS`。

**文件**：`crates/pa-core/src/platform/process.rs`、
`crates/pa-core/src/update/installer.rs`

**验证**：spawn 前基线枚举窗口，批量触发 MCP/更新路径 0 弹窗。

## 6. turnHook（`70fcc6ed9`）

**行为**：`settings.json` 配置
`"turnHook": {"command": "...", "timeoutMs": 30000}`。每个被接纳的 turn 在
首个 LLM 请求前执行命令（stdin 收 `{prompt, cwd, session_id}`），stdout 的
`hookSpecificOutput.additionalContext` 追加进本轮 system prompt（请求级，
不写入会话历史）。

**文件**：`crates/pa-core/src/session_engine/{turn_hook,engine,wiring,mod,admission}.rs`、
`crates/pa-core/src/settings/{types,load,mod}.rs`、`crates/pa-agent/src/agent.rs`

**验证**：`cargo test -p pa-core turn_hook --lib`（10 项）+ e2e 假 provider
确认注入标记出现在请求体中、每 turn 一次。

## 7. stopHook（`5d4df7fc3`）

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

## 8. stopHook 同步 spawn（`aae1659ad`）

**问题**：在 `tokio::spawn` 的分离任务里启动 hook，一次性运行时会在任务
被取消前退出，命令从未执行。

**行为**：改为在 `agent_end` 监听器内同步 spawn 进程，另起
`stop-hook-reaper` 线程负责轮询退出与超时 kill。

**文件**：`crates/pa-core/src/session_engine/{stop_hook,turn_hook,wiring}.rs`

## 9. updateGuard（`b1c355e60`）

**行为**：`settings.json` 出现 `"updateGuard": {"hold": true, "message": "..."}`
时拦截 `/update`、`prime-agent update` 与包自更新（`hold:false` 放行）；
`--check` 仍可用。用于防止 `/update` 用官方包覆盖定制构建。

**文件**：`crates/pa-core/src/settings/{types,load,manager,mod}.rs`、
`crates/pa-cli/src/{self_update,client_update,client_settings,public_command}.rs`、
`crates/pa-tui/src/{client_settings,session_ui/commands}.rs`

## 10. 剪贴板中文乱码（`5e2e7c7e6`）

**问题**：Windows 的 `clip.exe` 按控制台代码页（部分中文系统为 936/GBK）
解释 stdin，UTF-8 中文粘贴后变乱码。

**行为**：通过隐藏的 `cmd /d /s /c "chcp 65001>nul & clip"` 管道写入，
强制 UTF-8 代码页；不使用 UTF-16LE+BOM（会污染 ASCII 文本）。

**文件**：`crates/pa-tui/src/clipboard.rs`

## 11. 多行粘贴折叠（`a9807e007`）

**行为**：任何多行粘贴（≥2 行）都折叠为 `[paste #N +M lines]` 标记，
展开后仍是原文；原阈值 ≥10 行只对超长粘贴生效。

**文件**：`crates/pa-tui/src/editor/mod.rs`

## 12. clean-kernel-state（`513830af0`）

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

## 13. 空回复重试 + 流空闲超时（`d7410216f`）

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

# 本 fork 的维护与迭代指南

本文件说明 `GEFNENEN/prime-agent` 与上游仓库的关系、分支模型，以及同步上游、
验证和部署的标准操作流程。补丁内容本身见 [PATCHES.md](PATCHES.md)。

文中的 `<repo>` 指本仓库根目录，`<install-dir>` 指 prime-agent 的安装目录，
`<agent_dir>` 指运行配置目录。请按本机实际位置替换，不要把绝对路径写进文档。

## 1. 仓库关系

| 角色 | 仓库 | 说明 |
| --- | --- | --- |
| 上游（原版） | `PrimeIntellect-ai/prime-agent` | 只读参考；所有补丁都在它之上 |
| `upstream` remote | 同上 | `git fetch upstream --tags` |
| `origin` remote | `GEFNENEN/prime-agent` | 本 fork，推送目标 |

上游不接受未经邀请的 PR（见 `CONTRIBUTING.md`：agent-generated 贡献洪流）。
发现上游 bug 时先开 Discussion 等 maintainer 邀请，不要直接提 PR。

## 2. 分支模型

- `main`：上游镜像。**只沿稳定 tag 快进**，不落任何本地提交。
- `custom`：我们的补丁分支。提交链 = 基线 + 补丁（数量以
  [PATCHES.md](PATCHES.md) 的表格为准，本文件不再写死数字）。
- 基线选择：
  - 有新的稳定 tag（`v0.10.x`）时，优先 rebase 到稳定 tag；
  - 上游长期不发稳定版时，可临时以最新 beta tag 为基线，待稳定版发布后再
    rebase 过去。
- 版本号显示：二进制优先读取安装目录同级的 `package.json`
  （`crates/pa-cli/src/config.rs` 的 `packaged_manifest_version`），beta 基线
  构建会显示 beta 版本号。部署时应同步该文件（以及存在时的
  `.prime-agent-install`）。
- custom 构建的版本号带 build metadata 后缀 `+custom.<N>`（如
  `0.10.1+custom.18`）：`--version` 能区分 fork 构建，而 semver 比较忽略
  build metadata，不会让更新器把 fork 判成比基线更旧。部署时
  `package.json` 与 `.prime-agent-install` 必须写入完整字符串。
- 上游 beta tag 常带一个 tag-only 提交（只改 `Cargo.toml`/`Cargo.lock`
  版本行，分支不包含），基线直接取该 tag 提交。
- **不采用上游的 `.changes/` 变更片段**：那个目录的检查只在向
  `PrimeIntellect-ai/prime-agent` 开 PR 时触发（`.changes/README.md` 的
  changelog 片段门禁），而本 fork 直接推 `custom` 分支，从不向该仓库开 PR，
  门禁永远不会运行。变更记录以 [PATCHES.md](PATCHES.md) 为准。

## 3. 同步上游（标准流程）

> 预计耗时：fetch/分析 5–15 分钟，rebase 冲突 10–40 分钟，构建 3–5 分钟，
> 测试 5–15 分钟。建议先与使用者确认，期间不要部署正在运行的实例。

```powershell
cd <repo>
git status --short          # 必须干净；有改动先提交/暂存
git fetch upstream --tags --prune
```

1. **确认目标基线**：`git tag --list "v0.10.*" | Sort-Object`。优先稳定 tag，
   否则取最新 beta tag（`git log -1 --oneline <tag>`）。
2. **快进 main**：
   ```powershell
   git switch main
   git merge --ff-only upstream/main
   git switch custom
   ```
3. **打保护标签**（rebase 会改写历史，出问题可回退）：
   ```powershell
   git tag "custom-pre-sync-$(Get-Date -Format yyyyMMdd)" custom
   ```
4. **rebase**（`GIT_EDITOR` 必须设，否则环境无编辑器会挂起）：
   ```powershell
   $env:GIT_EDITOR = ':'
   git rebase <target-tag-or-main>
   ```
5. **处理冲突**（高发区见第 4 节）。原则：保留上游的新结构，把我们的补丁
   作为增量重新接上去；不要为了少冲突而丢弃上游逻辑。
6. **存活检查**（至少跑一遍，防止补丁被静默吞掉）：
   ```powershell
   rg -n "KERNEL_BUSY_REUSE_WAIT_MS|30_000" crates/pa-core/src/kernel/shared.rs
   rg -n "opencode_headers::apply|eq_ignore_ascii_case" crates/pa-ai/src/providers
   rg -n "CREATE_NO_WINDOW" crates/pa-core/src/platform/process.rs
   rg -n "turn_hook|set_turn_hook" crates/pa-core/src/session_engine/engine.rs
   rg -n "stop_hook|set_stop_hook" crates/pa-core/src/session_engine/engine.rs
   rg -n "retry_nudge" crates/pa-core/src/session_engine
   rg -n "clean_kernel_state" crates/pa-cli/src
   rg -n "resolve_theme" crates/pa-tui/src
   ```
   并确认无冲突标记：`rg -n "^(<<<<<<<|=======|>>>>>>>)" crates`
7. **构建**：`cargo check -p pa-core` → `cargo build --release`。
   若设置了共享的 `CARGO_TARGET_DIR`，注意跨分支/worktree 的 stale 产物。
8. **测试**：按第 5 节验证矩阵执行。**必须做基线对照**，区分「本次引入的
   失败」和「平台上本来就存在的失败」。
9. **部署**：先让使用者停止实例，再执行部署（第 6 节）。
10. **推送**：
    ```powershell
    git push --force-with-lease origin custom   # rebase 改写了历史，必须 force
    git push origin main                        # 快进
    ```
11. **记录**：更新 `PATCHES.md`（若有补丁增删/行为变化）。

## 4. 冲突高发区

| 位置 | 原因 | 处理要点 |
| --- | --- | --- |
| `crates/pa-core/src/session_engine/engine.rs` | turnHook 接在 `transform_context` / `get_system_prompt` 组装处；stopHook 接在 AgentEnd | 保留上游新的闭包/结构，把 hook 包装插到最外层；确认 `set_turn_hook` / `set_stop_hook` / `set_retry_nudge_slot` 都还在 |
| `crates/pa-core/src/settings/{types,load}.rs` | 上游新增设置键（如保留未知键） | 我们的 `turnHook/stopHook/updateGuard` 字段作为增量保留 |
| `crates/pa-core/src/session_engine/mod.rs`、`wiring.rs` | `AgentSession` 字段与构造 | 新字段（如 `retry_nudge`）与初始化、setter 一起保留 |
| `crates/pa-daemon/src/agent_engine/turn/model.rs` | 空回复重试在 attempt 闭包内 | 对齐上游循环结构后重新插入；`drop_trailing_assistant` 必须在重跑前调用 |
| `crates/pa-ai/src/utils_inner/http.rs` | 流式空闲超时包在 `next_text/next_bytes` | 上游若重构 stream 读取，超时逻辑要跟着搬到新的底层读取函数 |

rebase 期间不要用 `git rebase -i` 手工压缩历史；补丁保持独立提交，便于逐项
移植和逐项验证。

## 5. 验证矩阵

| 改动领域 | 命令 |
| --- | --- |
| turnHook / stopHook / retry_nudge | `cargo test -p pa-core --lib turn_hook`，`stop_hook`、`retry_nudge` 同样逐个跑（`cargo test` 只接受一个 TESTNAME） |
| 空回复重试（daemon 真实 HTTP/SSE） | `cargo test -p pa-daemon empty_reply --lib` |
| Authorization 去重 | `cargo test -p pa-ai --lib authorization` |
| CLI 命令 | `cargo test -p pa-cli --lib`（端口类失败可能是环境性，见下） |
| 全量回归对照 | 见下方「基线对照」 |

跑会启动引擎/内核的测试前，先把 `PRIME_AGENT_CODING_AGENT_DIR` 指到临时目录
（并配独立的 `PRIME_AGENT_DAEMON_SOCKET`）。用默认配置目录时，测试实例会真实
触发 `kernel-venv` 的 bootstrap，甚至尝试重建你正在使用的 venv。

**基线对照**（判断失败是否由补丁引入的唯一可靠方法）：

```powershell
git worktree add ../pa-base-check <target-tag>
cd ../pa-base-check
cargo test -p pa-core --lib 2>&1 | Select-Object -Last 3
# 与 custom 上的失败数量/名称对比；数量一致且名称相同 => 平台性，非本次引入
cd <repo>
git worktree remove --force ../pa-base-check
```

已知的**环境性**失败（两个分支上同样出现，不代表回归）：

- `pa-core --lib`：`session::window`（稀疏文件权限）、`shell_utils`（bash）、
  `path_utils`（HOME）、`packages`（npm/git）、`kernel::orphan_journal`
  （Linux 进程标识）等，在 Windows 上可能成批失败。
- `session_engine::tests::an_agent_end_writes_back_through_the_stop_hook` 与
  `session_engine::stop_hook::tests::a_custom_only_run_skips_the_command`：
  在 Windows + `more > <file>` 重定向下会读到「文件已存在但内容为空」的瞬态，
  已在上游 `v0.10.2-beta.10` 上 10/10 复现，属于测试自身的写入竞态，不是回归。
- 端口相关测试（`mcp_login`、OAuth 回调等）：Windows 上某些端口段可能被
  HNS/WinNAT 类组件保留，报 `os error 10048`（端口被占用），与代码无关。
  管理员执行 `net stop winnat && net start winnat` 可临时缓解。

**基线自己编不过时，先证伪再修**：beta tag 可能带着编译不过的测试目标
（实测 `v0.10.3-beta.5` 的 `pa-tui` lib 测试有 5 处 `E0061`）。遇到这种情况，
在干净的上游 worktree 上用同一命令复现一次，确认不是本 fork 引入的，再决定
补实参还是跳过该测试目标；结论写进 [PATCHES.md](PATCHES.md) 的 rebase 记录。

**不要用共享 `CARGO_TARGET_DIR` 做 A/B 对照**：两棵 worktree 会互相覆盖同名
测试二进制，导致两边都"失败"的假象。对照时给每棵树单独的 `CARGO_TARGET_DIR`，
或者先在一棵树上跑完再切另一棵。

## 6. 部署与回滚

建议把部署流程写成仓库外的本地脚本（不随仓库分发），至少包含：

1. 校验构建产物（记录 SHA256）；
2. graceful shutdown，必要时强制结束残留 daemon；
3. 备份现有二进制为 `prime-agent.exe.bak-<stamp>`；
4. 幂等写入 `stopHook` / `updateGuard` 到 `<agent_dir>/settings.json`
   （各留 `.bak-before-*`）；
5. 安装新二进制并校验哈希；
6. 同步随二进制分发的 runtime 负载与 skills（见下）；
7. 同步安装目录的 `package.json`（及存在时的 `.prime-agent-install`）版本号；
8. 运行 `--version` 验证。

**必须同步 runtime 负载**：内核 venv 是从 `<install-dir>/prime-agent-runtime`
（以及 skills 目录）安装的。只换 `prime-agent.exe` 会让新二进制配旧
`rlm`，内核的运行时探测失败（例如 0.10.1 要求 `bash(persistent, yield_after)`）。
部署时把仓库的 `prime-agent-runtime/` 与 `skills/` 一起覆盖到安装位置。

**两种包在 venv 里的存在形式不同**，这决定了刷新机制，别混为一谈：

| 包 | 形式 | 覆盖 `<install-dir>` 后的效果 |
| --- | --- | --- |
| `prime-agent-runtime`（`rlm`） | **物理复制**进 `site-packages`（旁边是普通 `prime_agent_runtime-0.1.0.dist-info`，无 `.pth`） | venv 里的副本仍是旧的，必须重建 |
| `skills/*` | **editable**，`_editable_impl_*.pth` 指向 `<install-dir>/skills/<name>/src` | 立即生效，无需重装 |

健康 venv 里 `rlm.__file__` 指向
`<agent_dir>/kernel-venv/Lib/site-packages/rlm/__init__.py`（不是安装目录），
而 `agent_observe.__file__` 指向 `<install-dir>/skills/agent-observe/src/…`。
用这两个路径判断某次装的是哪种形态。

**`ensure_kernel_venv` 有两条分支，行为完全不同**
（`crates/pa-core/src/kernel/bootstrap/mod.rs:237-305`）：

**(a) 只缺 skill，负载没变** —— `kernel_base_ready` 为真，走
`sync_python_skills`（`crates/pa-core/src/kernel/bootstrap/venv.rs:192-250`），
**只装缺的那部分**。判据是 `bootstrap_base_version_current`
（`…/bootstrap/venv/version.rs:49-65`），它会比对 `runtime_identity`、
schema、snapshot、额外 uv 参数，**不含 skill 列表**；skill 的覆盖检查在
`kernel_ready` 里的 `recorded_skills_cover`（同文件 `:30-47`）。
这一支是增量补装，不需要任何人工干预。

**(b) `kernel_base_ready` 为假** —— 走 `mod.rs:276-299`：
`std::fs::remove_dir_all(&venv)` 然后 `bootstrap_venv` **整库重建**。

`kernel_base_ready`（`crates/pa-core/src/kernel/bootstrap/venv.rs:260-270`）
= `bootstrap_base_version_current` 且 `has_prime_agent_runtime_memoized`，
所以为假的成因不止一类：

- **负载被覆盖**（最常见）：`runtime_identity` 是 `prime-agent-runtime/` 源码
  哈希（`hash_runtime_source`），负载一变它就变。
- **schema / snapshot / 额外 uv 参数不匹配**：`version.rs:49-65` 除
  `runtime_identity` 外还比对这三项。
- **运行时探测失败**：memo 未命中时 `has_prime_agent_runtime_memoized`
  退回 `has_prime_agent_runtime(python) && python_imports(python, "dill")`
  （`crates/pa-core/src/kernel/bootstrap/venv/probe.rs:190-191`），
  探测不过同样为假。

三类都走同一条整库重建路径。所以看到「rebuilding kernel venv」不要一口断定
是自己换了负载——也可能是 schema/snapshot/uv 参数变了，或者 venv 里的
`rlm` / `dill` 已经损坏。

换负载时是**程序自己删整个 venv**，不是「手工删才稳妥」。

**由此得出 `os error 5` 的真正含义**：重建要清空 venv，而 Windows 上目录被
运行中的 kernel python 占着就清不掉。`mod.rs:284-289` 捕获这个失败并给出
「Stop the prime-agent instance that is using it, then retry」。
所以手工删 venv 只在**这一种**被占用卡死的情况下是补救手段，不是常规步骤。

换 runtime 负载的正确顺序：覆盖负载 → 确认没有实例在跑 → 起一个真实会话
（daemon/TUI/ACP）跑一次 `python_repl`，它自己会删库重建。
若有实例在跑，会得到 `os error 5`，此时停实例再重试。

**哪些技能被装载取决于该会话注册了哪些 host handler**，不是固定全集。
`model_visible_skills`（`crates/pa-core/src/session_engine/runtime_wiring.rs:215-224`）
把 `CONTROLLER_BACKED_SKILLS`（同文件 `:202-209`：`agent-message`、
`agent-observe`、`rlm-heartbeat`）中「对应 host request 未注册」的技能从模型可见
列表中**过滤掉**；`kernel_python_skills`（同文件 `:228-238`）再把剩下的转成
`KernelPythonSkill` 交给 provisioner。**这一步只做过滤和转换，不安装任何包**。

`prime-agent -p` 的 handler 由 `inline_subagent_wiring`
（`crates/pa-cli/src/print_rlm.rs:232-244`）装配，它只调
`register_agent_message_host_handlers`，而后者只注册
`agent_message.list_agents` 与 `agent_message.send`
（`crates/pa-core/src/session_engine/agent_messaging.rs:389-400`）。
因此 `-p` 的目标列表里没有 `agent-observe` / `rlm-heartbeat`。

**但 `-p` 只会往 venv 里加，不会减。** `sync_python_skills` 先把旧
`.bootstrap-version` 里记录、且 `package_path` 目录仍在的 skill 读成
`installed`（`crates/pa-core/src/kernel/bootstrap/venv.rs:178-190`），只把目标列表
里缺的追加进去，最后把 `merged`（旧记录 ∪ 新装）写回
（同文件 `:251-257`）。所以在**已经装了这 8 个 skill 的 venv** 上跑 `-p`，
`agent_observe` / `rlm_heartbeat` 不会被移除，记录也照旧——此时看 venv 仍会
看到 8 个。

这里还有第二道保险：若已有记录覆盖目标列表，`recorded_skills_cover` 为真，
`kernel_ready` 直接返回真，`mod.rs:237-239` 在进入 `sync_python_skills`
之前就 return 了，连读写的机会都没有。

结论要分两种 venv：

- **干净 venv**（刚删过或从未建过）跑 `-p`：只有目标列表里的 skill 会被装，
  `agent-observe` / `rlm-heartbeat` 缺席。这不是缺包，也不代表部署出问题，
  **此时用 `-p` 验证 venv 完整性会得出错误结论**。
- **已有 venv** 跑 `-p`：旧 skill 被保留，看不出差异，`-p` 也不会替你清理
  多余项。

所以判断 venv 是否完整应当直接看内容，而不是靠跑 `-p` 观察：用 ACP 或 daemon
会话跑一次 `python_repl`，然后在 venv 里
`import agent_observe, rlm_heartbeat` 应成功，且 `__file__` 指向
`<install-dir>/skills/…`。

本机 `-p` 还观察到过无输出挂起（20 秒内 stdout/stderr 全空），原因未查清，
这也是一条别拿 `-p` 做验收的理由。

约定：`<stamp>` 取 `<版本>+custom<N>`（build metadata，如 `0.10.1+custom.18`；
备份文件名据此区分）。

- 安装位置与启动器：由安装器决定；确保 PATH 上的 `prime-agent` 指向
  `<install-dir>`。
- 回滚：把备份文件拷回 `<install-dir>/prime-agent.exe`。
- `updateGuard` 已写入 `settings.json` 时，`/update` 与 `prime-agent update`
  会被拦截；该 guard 挡不住上游安装器被直接运行，也不应被移除——否则
  `/update` 会下载官方包覆盖定制构建。

## 7. 可移植性注意事项

- 不要把本机的绝对路径、盘符、端口号、用户目录写进仓库文档或脚本；
  统一用 `<repo>`、`<install-dir>`、`<agent_dir>` 占位，或通过环境变量读取。
- 配置目录非默认位置时，用 `PRIME_AGENT_CODING_AGENT_DIR` 指定；
  测试隔离同理（配合独立的 `PRIME_AGENT_DAEMON_SOCKET`）。
- 构建产物目录若通过 `CARGO_TARGET_DIR` 共享，跨分支构建前先确认没有
  stale 产物。
- **增量编译在本仓库默认关闭**（`Cargo.toml` 的 `[profile.dev] incremental =
  false`）。共享 target 目录时 `debug/incremental/` 会无界增长，成为 `debug/`
  里最大的单一占用——它也是最不值钱的部分，删掉只损失重编速度。
  动因是 `cargo test`：仓库没有 `[profile.test]`，它继承 `dev`，所以关掉 `dev`
  的增量就把它一起盖住了。`cargo bench` 不在此列——`bench` 继承的是 `release`，
  而 `release` 本来就默认不产生增量单位（只会建一个空的 `release/incremental/`），
  本次改动与它无关。
  代价是改一个模块要重编整 crate，不是改几行重编几行。
  只在 `dev` 上关闭；`release` 不受影响，交付二进制的 debug-split 流程也不变。
  临时需要时用 `CARGO_INCREMENTAL=1 cargo ...` 覆盖。
  注意：关闭后 cargo 仍会建一个空的 `debug/incremental/`（0 个子目录），
  这是正常的，不代表开关没生效——判断标准是重编后子目录数是否仍为 0。
- MCP 服务器避免用 `npx` / `cmd /c` 启动（Windows 会弹控制台窗），改用
  `node <绝对路径>`；该路径属于使用者配置，不写入仓库。
- 测试用假 provider 时，`openai-completions` 必须返回 SSE（`stream:true`），
  否则 CLI 报 `Provider dropped the response stream`。
- `-p` 打印模式走 pa-core 的 SessionEngine，不经过 daemon 的
  `agent_engine/turn/model.rs`；依赖该路径的修复（如空回复重试）对 `-p`
  不生效。

## 8. 已知限制

- 空回复自动重试仅覆盖 daemon/TUI 路径；`-p` 模式仍可能静默结束。
- 自定义主题从 `<agent_dir>/themes/<name>.json` 加载，仅支持
  `prime/dark/light` 三个内置名之外的 JSON 主题文件。
- `clean-kernel-state` 只清理 `kernel-state.*` 文件，不删除会话与目录结构。
- **秘书常驻块的「压缩后保活」方案已评估并否决，未实施。** 评估稿与配套的校验
  脚本都没有入库，已删除；这里只留结论，避免重走一遍。若将来重启这条线：
  - system prompt 层是唯一免疫压缩的位置。注入点是 `build_system_prompt`，
    它只在建会话时执行一次，所以门控一旦定了就不会中途翻转。
  - `append_system_prompt` 在 resources 层已经被合成为单个 `Vec<String>`，
    到引擎注入点已无法区分来源。**因此不能在这一层做「只对 root 生效」的门控**
    —— 门一关，所有子会话会连通用护栏一起失去。
  - `[SECRETARY-STATE:*]` 这类对话内标记在 Rust 代码里没有读者；状态只活在
    transcript 里，且只有压缩器在读。注入侧不读 transcript（`messages_path`
    仅被格式化成一行显示文本）。所以「让注入侧尊重对话里的开关」等于新增读
    transcript 的机制，并且要动 `build_system_prompt` 的执行时序（它在既有
    消息读取之前执行），不是在现成位置插十几行。
  - 存在的 0 字节提示词文件等价于关闭（`resolve_prompt_input` 返回 `Some("")`，
    渲染侧有空串守卫），可以当文件级开关；但它是静默的（不产生 diagnostic），
    且项目级文件存在即不再合并全局，所以项目槽位会按仓库生效。
- `--append-system-prompt` / `--system-prompt` 在交互 TUI 下被静默丢弃：这两个值
  只经 `pa-cli` 的 print 路径转成 create config 的 `appendSystemPrompt` 传给引擎，
  而 `pa-tui` 的 `create_config()` 不写这两个键、全目录对 `appendSystem` 零引用。
  `-p` 模式不受此影响。

## 9. 相关文档

- [PATCHES.md](PATCHES.md)：当前补丁清单（实时，与 HEAD 对齐）
- [README.md](README.md)：上游项目说明
- [CONTRIBUTING.md](CONTRIBUTING.md)：上游贡献政策

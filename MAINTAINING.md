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
- `custom`：我们的补丁分支。提交链 = 基线 + 补丁（当前 17 个，见 PATCHES.md）。
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
   rg -n "eq_ignore_ascii_case\(\"Authorization\"\)" crates/pa-ai/src/providers
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
| turnHook / stopHook / retry_nudge | `cargo test -p pa-core turn_hook stop_hook retry_nudge --lib` |
| 空回复重试（daemon 真实 HTTP/SSE） | `cargo test -p pa-daemon empty_reply --lib` |
| Authorization 去重 | `cargo test -p pa-ai authorization_header --lib` |
| CLI 命令 | `cargo test -p pa-cli --lib`（端口类失败可能是环境性，见下） |
| 全量回归对照 | 见下方「基线对照」 |

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
- 端口相关测试（`mcp_login`、OAuth 回调等）：Windows 上某些端口段可能被
  HNS/WinNAT 类组件保留，报 `os error 10048`（端口被占用），与代码无关。
  管理员执行 `net stop winnat && net start winnat` 可临时缓解。

## 6. 部署与回滚

建议把部署流程写成仓库外的本地脚本（不随仓库分发），至少包含：

1. 校验构建产物（记录 SHA256）；
2. graceful shutdown，必要时强制结束残留 daemon；
3. 备份现有二进制为 `prime-agent.exe.bak-<stamp>`；
4. 幂等写入 `stopHook` / `updateGuard` 到 `<agent_dir>/settings.json`
   （各留 `.bak-before-*`）；
5. 安装新二进制并校验哈希；
6. 同步安装目录的 `package.json`（及存在时的 `.prime-agent-install`）版本号；
7. 运行 `--version` 验证。

约定：`<stamp>` 取 `<上游版本>-custom<N>`，备份文件名据此区分。

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

## 9. 相关文档

- [PATCHES.md](PATCHES.md)：当前补丁清单（实时，与 HEAD 对齐）
- [README.md](README.md)：上游项目说明
- [CONTRIBUTING.md](CONTRIBUTING.md)：上游贡献政策

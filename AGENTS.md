# Repository Guidelines

## 项目结构

本仓库是基于 Tauri 2、React、TypeScript 和 Rust 的 Windows 桌面应用。

- `src/`：React 渲染层、样式、类型和 Tauri API 适配层。
- `src-tauri/src/`：Rust 后端、宏状态机、存储、热键、托盘和 Win32 输入实现。
- `src-tauri/capabilities/`：Tauri 前端权限配置。
- `src-tauri/icons/`：应用和安装包图标。
- `src-tauri/tauri.conf.json`：窗口与 Windows 安装包配置。
- `src-tauri/app.manifest`：Windows DPI 感知及执行权限清单。

不要提交 `node_modules/`、`dist/`、`src-tauri/target/` 或其他生成目录。

## 开发与验证命令

- `pnpm install`：安装前端和 Tauri CLI 依赖。
- `pnpm dev`：仅启动 Vite 前端。
- `pnpm tauri:dev`：启动完整桌面应用。
- `pnpm typecheck`：检查 TypeScript。
- `pnpm build`：构建前端生产资源。
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`：检查 Rust 格式。
- `cargo check --manifest-path src-tauri/Cargo.toml`：检查 Rust 编译。
- `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`：运行 Rust 静态检查。
- `pnpm tauri:build`：生成 Windows NSIS 安装包。

## 编码规范

TypeScript 和 React 使用 2 空格、单引号、无分号、每行不超过 100 字符。组件使用 PascalCase，函数、变量和 hooks 使用 camelCase。Rust 必须通过 `cargo fmt`，模块和函数使用 snake_case，类型使用 PascalCase。

保持职责边界清晰：界面只通过 `src/lib/macro-api.ts` 调用后端；系统级能力、磁盘写入、热键和输入注入必须留在 Rust 端。新增或修改命令时，同步更新 TypeScript 类型与 Rust 参数/返回类型。

## 提交规范

- 所有新提交的标题和说明性正文使用中文；命令、路径、类型名等技术标识可以保留原文。
- 标题使用“分类：中文说明”，分类限于：`新增`、`修复`、`优化`、`重构`、`文档`、
  `测试`、`构建`、`发布`、`维护`、`合并`。
- 可用中文全角括号补充范围，例如 `修复（更新器）：避免重复安装同一版本`。
- Pull Request 标题也使用同一格式；压缩合并或创建合并提交时，确认最终提交标题已改为中文，
  不保留 GitHub 自动生成的英文 `Merge pull request` 标题。
- 发版提交使用 `发布：v1.15.0`，注解标签说明使用 `发布 v1.15.0`。
- 克隆仓库后运行 `pwsh -File scripts/setup-git-hooks.ps1` 启用本地校验；GitHub 提交检查和
  发布流水线还会检查规范启用后的全部提交，不能通过跳过本地钩子绕过。
- 发行说明使用发布范围内的中文提交标题，并自动排除 `发布` 类提交；首次启用规范时从规范
  启用提交开始，避免旧英文历史进入新发行说明。标题应面向用户简洁概括实际变化，不写含糊的
  “更新代码”“修复问题”。

## 功能验证

当前没有自动化 UI 测试。提交前至少验证 TypeScript 构建、Rust `fmt/check/clippy` 和 Tauri 安装包构建。涉及宏执行时还应手工覆盖：

- 100%、125%、150% 缩放以及混合 DPI 多显示器
- 左侧或上方副屏产生的负坐标
- 托盘隐藏后全局热键、开始、停止和紧急停止
- 鼠标点击、普通按键、功能键与组合键
- 方案持久化、旧数据迁移、导入和导出

## 发版

执行版本更新、打标签、构建安装包或创建发行版前，必须完整阅读并遵循
[`docs/release.md`](docs/release.md)。正式发行使用 GitHub Actions 的 Windows 节点构建，并把
产物发布到 Gitee；不要把 Gitee 令牌写入仓库、脚本、日志或聊天记录。不得移动或重建已经
公开的版本标签。只有两端代码与标签同步、自动工作流成功、Gitee 附件可下载且校验值正确时，
发版任务才算完成。

## 版本与在线更新

- 界面显示的应用版本必须从 Rust 编译包版本读取，禁止在 React 中硬编码版本号。
- `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 和
  `src-tauri/Cargo.lock` 中的应用版本必须保持一致。
- updater feed 只能向更高版本前进；同版本修复必须显式使用发布脚本的修复模式。
- 已公开标签中的发布脚本存在问题时，应在 `main` 修复脚本，再通过 `workflow_dispatch`
  修复原发行版，不得移动或重建标签。
- Gitee Contents API 的 PUT 请求必须显式设置受支持的 Content-Type，不能依赖 PowerShell
  默认行为。
- 发布验收不能只查看 GitHub Actions 状态；必须匿名下载 EXE、SIG、Release `latest.json`
  和公开 updater feed，并核对安装包实际 SHA-256、发行说明 SHA-256、下载 URL 和签名内容。

## 测试稳定性

- 在资源紧张的 Windows 环境中，避免并行运行完整前端测试和 Rust 编译。重型页面测试可能因
  CPU 争用超过 Vitest 默认超时；发生纯超时时应先单独重跑，不得直接视为功能失败或随意提高
  全局超时。

## 临时验证文件

- 本地升级测试配置和公开附件验证文件只能放在系统临时目录，不得进入仓库。
- 验证完成后删除临时配置和下载文件，并确认 `git status --short` 没有意外修改或未跟踪文件。

## 安全提示

不要向前端开放通用文件系统、Shell 或任意命令执行权限。Win32 输入只接受经过清洗的坐标和支持的虚拟键。应用默认使用普通用户权限，不应通过默认管理员权限绕过 Windows UIPI。

# Repository Guidelines

## 项目结构

本仓库是基于 Tauri 2、React、TypeScript 和 Rust 的 Windows Buff 检测桌面应用。

- `src/features/buff-assistant/`：Buff 配置、模板编辑和悬浮窗界面。
- `src/hooks/useBuffAssistantController.ts`：前端运行状态控制器。
- `src/lib/buff-sentinel-api.ts`：前端可调用的 Tauri API 边界。
- `src-tauri/src/buff_assistant/`：窗口捕获、模板检测、时间轴、声音和持久化。
- `src-tauri/capabilities/`：主窗口和悬浮窗权限。
- `src-tauri/resources/buff-sounds/`：内置提示音。

不要提交 `node_modules/`、`dist/`、`src-tauri/target/` 或其他生成目录。

## 开发与验证命令

- `pnpm install`
- `pnpm dev`
- `pnpm tauri:dev`
- `pnpm typecheck`
- `pnpm test`
- `pnpm build`
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
- `pnpm tauri:build`

## 编码规范

TypeScript 和 React 使用 2 空格、单引号、无分号、每行不超过 100 字符。Rust 必须通过
`cargo fmt`。界面只通过 `src/lib/buff-sentinel-api.ts` 调用后端；窗口捕获、磁盘写入和系统能力留在
Rust 端。新增命令时同步更新 TypeScript 与 Rust 的参数和返回类型。

## 功能验证

提交前至少验证 TypeScript 构建、前端测试、Rust `fmt/check/clippy` 和 Tauri 安装包构建。
涉及捕获与悬浮窗时，还应手工覆盖：

- 100%、125%、150% 缩放和混合 DPI 多显示器
- 左侧或上方副屏产生的负坐标
- 游戏窗口最小化、关闭、重启后的自动重连
- 悬浮窗拖动、缩放、鼠标穿透和录屏排除
- 模板、遮罩、自定义声音和设置持久化

## 安全提示

不要向前端开放通用文件系统、Shell 或任意命令执行权限。应用默认使用普通用户权限，不应通过
默认管理员权限绕过 Windows UIPI。

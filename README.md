# Buff 哨兵

Buff Sentinel 是基于 Tauri 2、React、TypeScript 和 Rust 开发的 Windows Buff 检测与提醒工具。

项目从原 `shree-macro-flow-tauri` 中的金周天 Buff 助手独立而来，后续将在这里演进为可配置、
可扩展的通用 Buff / Debuff 状态识别软件。

## 当前功能

- 使用 Windows Graphics Capture 监听指定窗口，不读取游戏内存、不注入游戏进程
- 框选 Buff 搜索区域并裁剪图标模板
- 支持动态数字、层数和闪光区域遮罩
- 连续帧模板匹配，减少单帧误判
- 可配置检测阈值、确认帧数、消失帧数、周期和触发宽限期
- 提前 3、2、1 秒发出悬浮窗和声音提醒
- 支持内置提示音、自定义 WAV 和在线 TTS 辅助制作
- 透明置顶悬浮窗支持鼠标穿透、拖动、缩放和录屏排除
- 系统托盘常驻、关闭到托盘和单实例运行

当前版本仍以金周天的固定周期检测流程为第一种内置规则。后续会逐步抽象为多 Buff 模板、
独立规则和组合提醒。

## 开发环境

- Node.js 20+
- pnpm 10
- Rust stable（MSVC）
- Visual Studio 2022 Build Tools
- Windows 10/11 SDK
- Microsoft Edge WebView2 Runtime

## 开发命令

```powershell
pnpm install
pnpm dev
pnpm tauri:dev
pnpm typecheck
pnpm test
pnpm build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
pnpm tauri:build
```

## 项目结构

- `src/features/buff-assistant/`：Buff 配置、模板编辑与悬浮提醒界面
- `src/hooks/useBuffAssistantController.ts`：前端 Buff 状态控制器
- `src/lib/buff-sentinel-api.ts`：最小化的 Tauri API 类型与调用适配层
- `src-tauri/src/buff_assistant/`：捕获、识别、时间轴、声音和存储实现
- `src-tauri/capabilities/`：主窗口与悬浮窗权限配置
- `src-tauri/resources/buff-sounds/`：内置声音模板

## 安全边界

应用默认以普通用户权限运行。前端不开放通用文件系统、Shell 或任意命令执行权限；画面识别
仅处理用户选择的窗口和区域。

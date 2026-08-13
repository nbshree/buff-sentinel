# 自动发版与在线更新

本项目沿用 `shree-macro-flow-tauri` 的发布架构：GitHub Actions 负责 Windows 验证和签名构建，
发布脚本通过 Gitee API 创建 Release，并把 Tauri updater 的 `latest.json` 写入独立的
`updater-feed` 分支。

## 首次配置

1. GitHub 仓库使用 `nbshree/buff-sentinel`；本地已配置为 `github` 远端，Gitee 保持为 `origin`。
2. 在 GitHub Actions Secrets 配置：
   - `TAURI_SIGNING_PRIVATE_KEY`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
   - `GITEE_TOKEN`
3. 当前 updater 公钥沿用 `shree-macro-flow-tauri`，因此前两个 secret 应使用旧项目的同一套私钥。
4. Gitee token 需要仓库和 Release 写权限。

## 发版

同时更新以下三个版本号，保持完全一致：

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

提交并推送后创建注解标签，再把分支与标签同时推到两个远端：

```powershell
git tag -a v0.2.0 -m "Buff 哨兵 v0.2.0"
git push origin main v0.2.0
git push github main v0.2.0
```

GitHub 收到 `v*` 标签后自动执行测试、构建签名安装包、发布 Gitee Release 和更新在线更新源。
已有版本需要修复附件时，在 GitHub Actions 手工运行工作流并填写标签。

## 本地发布

本地已配置签名环境变量和 `GITEE_TOKEN` 时可运行：

```powershell
./scripts/publish-gitee.ps1 -Version 0.2.0
```

首个包含 updater 的版本仍需用户手动安装一次；之后应用启动时会静默检查，也可点击标题栏的更新按钮。

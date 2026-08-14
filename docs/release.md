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

### 人工确认门禁

收到“发布”指令后，第一步必须向发布人展示拟发布版本和拟使用的更新说明，并等待发布人明确确认。
确认前禁止执行以下操作：

- 修改任何版本号
- 创建或提交版本变更
- 创建发布标签
- 推送发布标签或触发发布工作流

不得根据提交记录自行确定更新说明，也不得把建议文案视为已经确认。只有发布人明确回复同意某段
更新说明后，才能进入后续发版步骤。

同时更新以下三个版本号，保持完全一致：

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

提交并推送后，先让发布人确认本次更新内容，再把更新说明写入注解标签。发布脚本只使用
标签中的说明，不再根据提交标题自动生成更新日志。

如果发布人回复“随便”，统一使用：`解决了一些小问题`。

确认更新内容后，把分支与标签同时推到两个远端：

```powershell
$releaseNotes = "修复 Buff 识别与悬浮窗显示问题"
if ($releaseNotes -eq "随便") { $releaseNotes = "解决了一些小问题" }
git tag -a v0.2.0 -m $releaseNotes
git push origin main v0.2.0
git push github main v0.2.0
```

标签必须是包含非空更新说明的注解标签；缺少说明时自动发布会失败，避免发布空白或错误日志。

GitHub 收到 `v*` 标签后自动执行测试、构建签名安装包、发布 Gitee Release 和更新在线更新源。
已有版本需要修复附件时，在 GitHub Actions 手工运行工作流并填写标签。

## 本地发布

本地已配置签名环境变量和 `GITEE_TOKEN` 时可运行：

```powershell
./scripts/publish-gitee.ps1 -Version 0.2.0 -Notes "修复 Buff 识别与悬浮窗显示问题"
```

首个包含 updater 的版本仍需用户手动安装一次；之后应用启动时会静默检查，也可点击标题栏的更新按钮。

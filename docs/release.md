# Windows 发版运行手册

本文用于维护者或 Codex 执行 Windows 正式发版。开始发版前必须完整阅读本文。

## 当前发布架构

- 主代码仓库与发行版：<https://gitee.com/nbshree/shree-macro-flow-tauri>
- Windows 构建引擎：GitHub Actions
- GitHub 镜像仓库：<https://github.com/nbshree/shree-macro-flow-tauri>
- 自动发布工作流：`.github/workflows/gitee-release.yml`
- 本地/流水线发布脚本：`scripts/publish-gitee.ps1`
- 正式构建覆盖配置：`src-tauri/tauri.release.conf.json`
- 在线更新源：<https://gitee.com/nbshree/shree-macro-flow-tauri/raw/updater-feed/latest.json>
- 标签格式：`v主版本.次版本.修订号`，例如 `v1.8.0`
- Gitee 安装包名称：`macro-flow_1.8.1_x64-setup.exe`

Gitee Go 官方云构建环境是 Linux，不能可靠生成本项目的 Windows NSIS 安装包。因此使用
GitHub Actions 的 `windows-latest` 构建，再通过 Gitee API 把安装包、Tauri 更新签名和
`latest.json` 发布回 Gitee。

Gitee 不支持 GitHub 风格的 `/releases/latest/download/latest.json` 固定地址。发布脚本会在
Gitee 单独维护 `updater-feed` 分支，并在该分支根目录原子更新 `latest.json`。正式发行版仍
位于版本标签对应的 Gitee Release。

## 凭据、签名与安全

GitHub 仓库必须存在以下 Actions Secret：

- `GITEE_TOKEN`：具有目标仓库代码和发行版权限的 Gitee 私人令牌。
- `TAURI_SIGNING_PRIVATE_KEY`：Tauri updater 私钥的完整内容。
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：私钥密码。

检查 Secret 名称是否存在，不会显示内容：

```powershell
gh secret list --repo nbshree/shree-macro-flow-tauri
```

设置或轮换 Gitee 令牌：

```powershell
gh secret set GITEE_TOKEN --repo nbshree/shree-macro-flow-tauri
```

首次接入 updater 时，只生成一次签名密钥：

```powershell
pnpm tauri signer generate -w "$env:USERPROFILE\.tauri\macro-flow.key"
```

生成命令会交互式要求输入密码。不要加 `--ci`，因为当前 Tauri CLI 会在未通过命令参数提供
密码时生成空密码密钥。本项目维护机约定把密码用当前 Windows 用户的 DPAPI 加密保存到：

```text
%USERPROFILE%\.tauri\macro-flow.key.password.dpapi
```

本机恢复副本位于 `%USERPROFILE%\Documents\MacroFlow-Updater-Key-Backup\`。其中私钥、公钥和
DPAPI 密码文件必须再复制到受控的离线备份；DPAPI 文件只能由生成它的 Windows 用户配置解密，
不能作为跨机器恢复密码的唯一手段。

将生成的公钥内容写入 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`。公钥可以公开，
但私钥和密码不得进入仓库、构建产物、终端命令参数、日志、发行说明或聊天记录。

把私钥内容写入 GitHub Secret 时使用标准输入，避免出现在命令参数中：

```powershell
Get-Content -LiteralPath "$env:USERPROFILE\.tauri\macro-flow.key" -Raw |
  gh secret set TAURI_SIGNING_PRIVATE_KEY --repo nbshree/shree-macro-flow-tauri

gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo nbshree/shree-macro-flow-tauri
```

必须把私钥和密码离线备份到受控位置。私钥丢失后，已经安装该公钥版本的客户端将无法接受
任何后续在线更新。除非实施完整的公钥轮换迁移，否则不要更换 updater 密钥。

Tauri updater 签名用于证明在线更新包来自本项目，不能替代 Windows Authenticode 商业代码
签名。未做商业代码签名时，SmartScreen 仍可能显示安全提示。

Gitee 令牌泄露后应立即撤销并更新 `GITEE_TOKEN`。updater 私钥疑似泄露时停止发版，不要仅
通过生成新密钥继续发布，因为旧客户端仍信任旧公钥。

## 首个在线更新版本

`v1.8.0` 是计划中的首个 updater-enabled 版本。`v1.7.1` 及更早版本没有检查更新能力，无法
自行升级到 `v1.8.0`；这些用户必须先从 Gitee 手动下载安装一次。安装 `v1.8.0` 后，后续
更高版本才能在应用内完成检查、下载、签名校验和安装。

为了在不增加公开测试版本的情况下验收首版，可以先使用临时覆盖配置在本机构建并安装一个
不公开的 `v1.7.99`，再发布 `v1.8.0`，由该临时版本通过正式 Gitee feed 在线升级。临时版本
不得创建或推送标签，也不得上传到公开 Release。

## 本地构建与正式构建

普通本地安装包不生成 updater 签名，因此不需要私钥：

```powershell
pnpm tauri:build
```

正式发行必须使用 release 覆盖配置，并在当前进程中提供签名环境变量：

```powershell
$privateKeyPath = "$env:USERPROFILE\.tauri\macro-flow.key"
$passwordBackupPath = "$env:USERPROFILE\.tauri\macro-flow.key.password.dpapi"
$secureSigningPassword = ConvertTo-SecureString (
  Get-Content -LiteralPath $passwordBackupPath -Raw
)
$signingPasswordPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR(
  $secureSigningPassword
)

try {
  $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -LiteralPath $privateKeyPath -Raw
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD =
    [Runtime.InteropServices.Marshal]::PtrToStringBSTR($signingPasswordPointer)
  pnpm tauri:build:release
} finally {
  Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
  Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
  [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($signingPasswordPointer)
}
```

正式构建会在 `src-tauri/target/release/bundle/nsis/` 生成：

- `自动点击流程台_<版本>_x64-setup.exe`
- `自动点击流程台_<版本>_x64-setup.exe.sig`

不要用普通 `pnpm tauri:build` 的产物创建正式发行版。发布脚本会拒绝缺少 `.sig` 的构建。

## 发版前检查

1. 确认工作区状态，保留并理解所有用户修改：

   ```powershell
   git status --short
   ```

2. 首次在当前克隆中提交前启用仓库钩子：

   ```powershell
   pwsh -File scripts/setup-git-hooks.ps1
   ```

3. 确认本次所有提交遵守 `AGENTS.md` 的中文提交规范。发行说明会直接使用这些标题并排除
   `发布` 类提交。首次启用规范时，发行说明从规范启用提交开始，避免旧英文历史进入更新内容；
   后续版本仍按两个版本标签之间的提交生成。日常提交检查、发布脚本和发布流水线都会校验
   规范启用后的提交历史。
4. 确认当前分支是 `main`，并与 Gitee、GitHub 的 `main` 对齐。
5. 确定新版本号，且该标签从未发布。不要移动或重建已经公开的版本标签。
6. 新版本必须严格高于 updater feed 中的版本；发布脚本会用 SemVer 比较并拒绝降级覆盖。
7. 同步修改以下版本号：

   - `package.json`
   - `src-tauri/tauri.conf.json`
   - `src-tauri/Cargo.toml`

8. 运行依赖或 Rust 命令后，把相应锁文件变化一并纳入发版提交：

   - `pnpm-lock.yaml`
   - `src-tauri/Cargo.lock`

9. 至少执行以下验证：

   ```powershell
   pnpm install --frozen-lockfile
   pnpm typecheck
   pnpm test
   pnpm build
   cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
   cargo check --manifest-path src-tauri/Cargo.toml
   cargo test --manifest-path src-tauri/Cargo.toml
   cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
   ```

涉及宏输入、游戏操作录制、DPI、托盘或持久化的改动，还应执行 `AGENTS.md` 中列出的
Windows 手工验证。涉及 updater 时，还要手工检查宏录制/执行以及游戏录制/回放期间均不能
开始安装。

## 自动发版步骤

下面以 `v1.8.0` 为例。标签中的版本必须和 `package.json` 完全一致，发布脚本会强制校验。

```powershell
git add package.json pnpm-lock.yaml `
  src-tauri/tauri.conf.json `
  src-tauri/Cargo.toml `
  src-tauri/Cargo.lock

git commit -m "发布：v1.8.0"
git tag -a v1.8.0 -m "发布 v1.8.0"
git push origin main --follow-tags
```

当前开发机的 `origin` 配置了两个 push URL，因此最后一条命令会同时推送到 Gitee 和
GitHub。检查方式：

```powershell
git config --get-all remote.origin.pushurl
```

预期输出：

```text
git@gitee.com:nbshree/shree-macro-flow-tauri.git
git@github.com:nbshree/shree-macro-flow-tauri.git
```

如果换了电脑或重新克隆仓库，需要重新配置：

```powershell
git config --unset-all remote.origin.pushurl
git config --add remote.origin.pushurl git@gitee.com:nbshree/shree-macro-flow-tauri.git
git config --add remote.origin.pushurl git@github.com:nbshree/shree-macro-flow-tauri.git
```

标签到达 GitHub 后，单次发版按以下流程执行：

1. 在两个独立 Windows job 中并行执行源码验证和正式签名构建。
2. 验证 job 先检查中文提交标题，再执行 TypeScript 检查、前端测试、Rust 格式检查、测试和
   Clippy。Rust 测试和 Clippy 均会完成编译检查，因此流水线不再额外重复执行 `cargo check`；
   本地发版前检查仍保留 `cargo check`。
3. 构建 job 只在正式构建步骤注入 Tauri 私钥，执行 `pnpm tauri:build:release`，再把 `.exe`
   和 `.sig` 作为短期 Actions artifact 传给发布 job。
4. 只有验证与构建同时成功后才启动发布 job，并只在该 job 注入 `GITEE_TOKEN`。
5. 创建或修复 Gitee Release，上传 ASCII 名称的 `.exe`、`.exe.sig` 和 `latest.json`。
6. 从 Release 的公开 URL 重新下载并验证三个附件及安装包 SHA-256。
7. 首次发布时自动创建 `updater-feed` 分支，再更新分支根目录的 `latest.json`。
8. 最后重新读取公开 updater feed，确认版本、下载 URL 和签名已正确生效。

不同标签共用同一个 concurrency group，避免较老版本晚于较新版本更新 feed。第一次工作流
尝试不允许覆盖同版本；手动重新运行失败的 Actions run 时，`github.run_attempt > 1` 会自动
启用修复模式。

如果失败原因位于已经打标签的发布脚本本身，不得移动标签。修复 main 上的脚本后，从 main
手动触发同一工作流；它会检出原标签源码，只从 main 覆盖发布脚本并启用修复模式：

```powershell
gh workflow run gitee-release.yml `
  --repo nbshree/shree-macro-flow-tauri `
  --ref main `
  -f release_ref=v1.8.0 `
  -f repair_existing=true
```

## 观察与验收

查看最近的流水线：

```powershell
gh run list `
  --workflow gitee-release.yml `
  --repo nbshree/shree-macro-flow-tauri `
  --limit 5
```

等待某次流水线结束：

```powershell
gh run watch <run-id> `
  --repo nbshree/shree-macro-flow-tauri `
  --exit-status
```

失败时查看日志：

```powershell
gh run view <run-id> `
  --repo nbshree/shree-macro-flow-tauri `
  --log-failed
```

流水线成功后还必须检查 Gitee 发行版：

- 标签、标题和版本号一致。
- 存在 `macro-flow_<版本>_x64-setup.exe`。
- 存在 `macro-flow_<版本>_x64-setup.exe.sig`。
- 存在 `latest.json`，且三者均没有 MIME 编码乱码。
- 安装包可以公开下载，下载后的 SHA-256 与发行说明一致。
- Release 附件中的 `latest.json` 与 `updater-feed/latest.json` 版本、URL、签名一致。
- feed 的 `platforms.windows-x86_64.url` 指向本次 Gitee 安装包。
- feed 的签名是 `.sig` 文件内容，而不是路径或下载 URL。
- 发行说明中的中文提交信息显示正常。

正式发版不要求在发布机安装旧版本，也不要求通过本机应用内 updater 执行实际升级。以流水线
验证、公开附件匿名下载、SHA-256、签名内容和 updater feed 一致性检查作为在线更新验收依据。

公开 feed 检查：

```powershell
Invoke-RestMethod `
  'https://gitee.com/nbshree/shree-macro-flow-tauri/raw/updater-feed/latest.json'
```

## 手动发布与修复

自动流水线不可用时，先在安全的本地会话中设置 updater 私钥环境变量，再正式构建和发布：

```powershell
pnpm tauri:build:release
pwsh -File .\scripts\publish-gitee.ps1 -Version 1.8.0 -SkipBuild
```

脚本会隐藏输入 Gitee 令牌。不要用命令行参数传递令牌。完成后清理当前 PowerShell 会话中的
签名环境变量：

```powershell
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY -ErrorAction SilentlyContinue
Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
```

修复已经存在的发行说明、附件或同版本 feed：

```powershell
pwsh -File .\scripts\publish-gitee.ps1 `
  -Version 1.8.0 `
  -SkipBuild `
  -RepairExisting
```

修复模式会替换脚本管理的 `.exe`、`.sig`、`latest.json`，再重新验证公开内容。同版本只有
显式 `-RepairExisting` 才允许写入；即使使用修复模式，也绝不允许用低版本覆盖高版本 feed。

早期脚本上传中文文件名时，Gitee 可能把附件显示为 `=?utf-8?B?...?=`。当前脚本会使用
ASCII 文件名避免该问题。乱码旧附件需要在 Gitee 发行版编辑页面手动删除。

## 常见故障

### 推送标签后没有触发 GitHub Actions

先确认标签确实同时存在：

```powershell
git ls-remote git@gitee.com:nbshree/shree-macro-flow-tauri.git refs/tags/v1.8.0
git ls-remote git@github.com:nbshree/shree-macro-flow-tauri.git refs/tags/v1.8.0
```

如果 GitHub 缺少标签，检查本机 `remote.origin.pushurl`。不要删除已经推送到 Gitee 的标签；
修正 push URL 后只向 GitHub 补推相同标签即可。

### 工作流提示 Secret 未配置或签名失败

用 `gh secret list` 检查三个 Secret 的名称。Gitee 认证失败时撤销旧令牌并更新
`GITEE_TOKEN`。构建没有生成 `.sig` 时，检查 updater 私钥内容、密码和 release 构建命令，
不要改用普通构建产物绕过签名。

### Gitee 提示发行版或 feed 已经存在

不要创建同名标签或重复正式发布。若自动任务在上传附件或更新 feed 时中断，重新运行失败的
GitHub Actions run，或使用 `-RepairExisting -SkipBuild` 修复。

### 发布脚本拒绝旧版本覆盖

先读取公开 `latest.json`，确认准备发布的版本严格更高。如果错误地提前发布了更高版本，不要
移动公开标签或降低 feed；应选择新的、更高 SemVer 修复。

### 中文提交说明乱码

发布脚本已强制 `git log` 使用 UTF-8。不要删除脚本中的 `$OutputEncoding`、
`[Console]::OutputEncoding` 或 Git `--encoding=UTF-8` 设置。

### 提交被中文规范校验拒绝

标题必须采用“分类：中文说明”，例如 `新增：支持方案批量导入` 或
`修复（热键）：托盘隐藏后仍可紧急停止`。不要再使用 `feat:`、`fix:`、`chore:` 等英文分类。
通过 Pull Request 合并时，也要把压缩提交或合并提交的最终标题改成中文，不能保留 GitHub
自动生成的英文 `Merge pull request` 标题。
如果当前克隆没有即时提示，运行 `pwsh -File scripts/setup-git-hooks.ps1` 重新启用本地钩子。

## 发版完成条件

只有同时满足以下条件，发版任务才算完成：

- 新版本提交已推送到 Gitee 和 GitHub。
- 注解标签已推送到两个仓库，且指向同一个提交。
- GitHub Actions 工作流成功。
- Gitee 发行版存在，`.exe`、`.sig`、`latest.json` 均可公开下载。
- 安装包 SHA-256、更新签名、feed URL 和版本号均正确。
- `updater-feed/latest.json` 只前进、不降级，并指向本次发行附件。
- 工作区没有意外的未提交修改。

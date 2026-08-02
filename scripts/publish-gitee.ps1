param(
  [string]$Version,
  [switch]$SkipBuild,
  [switch]$RepairExisting
)

$ErrorActionPreference = 'Stop'
$utf8WithoutBom = [Text.UTF8Encoding]::new($false)
$OutputEncoding = $utf8WithoutBom
[Console]::OutputEncoding = $utf8WithoutBom

if ($PSVersionTable.PSVersion.Major -lt 7) {
  throw '请使用 PowerShell 7 或更高版本运行此脚本。'
}

function ConvertTo-SemanticVersion {
  param([Parameter(Mandatory)][string]$Value)

  $normalized = $Value -replace '^v', ''
  try {
    return [System.Management.Automation.SemanticVersion]::new($normalized)
  } catch {
    throw "版本号不是有效的 SemVer：$Value"
  }
}

function Get-HttpStatusCode {
  param([Parameter(Mandatory)]$ErrorRecord)

  $response = $ErrorRecord.Exception.Response
  if ($null -eq $response -or $null -eq $response.StatusCode) {
    return $null
  }
  return [int]$response.StatusCode
}

function Get-RemoteTagCommit {
  param(
    [Parameter(Mandatory)][string]$RepositoryUrl,
    [Parameter(Mandatory)][string]$TagName
  )

  $remoteTag = git ls-remote $RepositoryUrl "refs/tags/$TagName^{}"
  if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace([string]$remoteTag)) {
    throw "远程仓库不存在注解标签 $TagName：$RepositoryUrl"
  }
  return ([string]$remoteTag -split '\s+')[0]
}

function Get-GiteeBranch {
  param(
    [Parameter(Mandatory)][string]$ApiBase,
    [Parameter(Mandatory)][hashtable]$Headers,
    [Parameter(Mandatory)][string]$Branch
  )

  try {
    return Invoke-RestMethod `
      -Uri "$ApiBase/branches/$([Uri]::EscapeDataString($Branch))" `
      -Headers $Headers `
      -Method Get
  } catch {
    if ((Get-HttpStatusCode $_) -eq 404) {
      return $null
    }
    throw
  }
}

function Get-GiteeContentFile {
  param(
    [Parameter(Mandatory)][string]$ApiBase,
    [Parameter(Mandatory)][hashtable]$Headers,
    [Parameter(Mandatory)][string]$Branch,
    [Parameter(Mandatory)][string]$Path
  )

  $encodedBranch = [Uri]::EscapeDataString($Branch)
  try {
    $file = Invoke-RestMethod `
      -Uri "$ApiBase/contents/$Path`?ref=$encodedBranch" `
      -Headers $Headers `
      -Method Get
    if (@($file).Count -eq 0) {
      return $null
    }
    return $file
  } catch {
    if ((Get-HttpStatusCode $_) -eq 404) {
      return $null
    }
    throw
  }
}

function Get-PublicJsonWithRetry {
  param(
    [Parameter(Mandatory)][string]$Uri,
    [scriptblock]$Validate,
    [int]$Attempts = 12,
    [int]$DelaySeconds = 5
  )

  for ($attempt = 1; $attempt -le $Attempts; $attempt++) {
    $requestUri = if ($Uri.Contains('?')) {
      "$Uri&attempt=$attempt"
    } else {
      "$Uri`?attempt=$attempt"
    }
    try {
      $document = (Invoke-WebRequest -Uri $requestUri -UseBasicParsing).Content |
        ConvertFrom-Json
      if ($null -ne $Validate) {
        & $Validate $document
      }
      return $document
    } catch {
      if ($attempt -eq $Attempts) {
        throw
      }
      Start-Sleep -Seconds $DelaySeconds
    }
  }
}

function Read-GiteeJsonContent {
  param([Parameter(Mandatory)]$File)

  if ([string]::IsNullOrWhiteSpace([string]$File.content)) {
    throw "Gitee 文件缺少可解码内容：$($File.path)"
  }
  try {
    $bytes = [Convert]::FromBase64String(([string]$File.content -replace '\s', ''))
    $json = [Text.Encoding]::UTF8.GetString($bytes)
    return $json | ConvertFrom-Json
  } catch {
    throw "Gitee 文件不是有效的 UTF-8 JSON：$($File.path)"
  }
}

function Assert-FeedVersionCanAdvance {
  param(
    $FeedFile,
    [Parameter(Mandatory)][System.Management.Automation.SemanticVersion]$NextVersion,
    [Parameter(Mandatory)][bool]$AllowSameVersion
  )

  if ($null -eq $FeedFile) {
    return
  }

  $feed = Read-GiteeJsonContent $FeedFile
  if ([string]::IsNullOrWhiteSpace([string]$feed.version)) {
    throw '现有 updater feed 缺少 version，拒绝覆盖。'
  }
  $currentVersion = ConvertTo-SemanticVersion ([string]$feed.version)
  if ($NextVersion -lt $currentVersion) {
    throw "拒绝用旧版本 $NextVersion 覆盖 updater feed 的 $currentVersion。"
  }
  if ($NextVersion -eq $currentVersion -and -not $AllowSameVersion) {
    throw "updater feed 已是 $currentVersion；同版本只能使用 -RepairExisting 修复。"
  }
}

function Upload-GiteeReleaseAsset {
  param(
    [Parameter(Mandatory)][string]$ApiBase,
    [Parameter(Mandatory)][hashtable]$Headers,
    [Parameter(Mandatory)]$ReleaseId,
    [Parameter(Mandatory)][string]$Path
  )

  return Invoke-RestMethod `
    -Uri "$ApiBase/releases/$ReleaseId/attach_files" `
    -Headers $Headers `
    -Method Post `
    -Form @{ file = Get-Item -LiteralPath $Path }
}

function Get-GiteeReleaseAssets {
  param(
    [Parameter(Mandatory)][string]$ApiBase,
    [Parameter(Mandatory)][hashtable]$Headers,
    [Parameter(Mandatory)]$ReleaseId
  )

  $assets = @()
  $page = 1
  $pageSize = 100
  do {
    $pageResponse = Invoke-RestMethod `
        -Uri "$ApiBase/releases/$ReleaseId/attach_files?page=$page&per_page=$pageSize" `
        -Headers $Headers `
        -Method Get
    # PowerShell 7 emits a JSON array from Invoke-RestMethod as one pipeline object.
    $pageAssets = @($pageResponse | ForEach-Object { $_ })
    $assets += $pageAssets
    $page++
  } while ($pageAssets.Count -eq $pageSize)

  return $assets
}

function Get-GiteeReleaseByTag {
  param(
    [Parameter(Mandatory)][string]$ApiBase,
    [Parameter(Mandatory)][hashtable]$Headers,
    [Parameter(Mandatory)][string]$TagName
  )

  try {
    return Invoke-RestMethod `
      -Uri "$ApiBase/releases/tags/$([Uri]::EscapeDataString($TagName))" `
      -Headers $Headers `
      -Method Get
  } catch {
    if ((Get-HttpStatusCode $_) -eq 404) {
      return $null
    }
    throw
  }
}

function Assert-UpdaterFeedDocument {
  param(
    [Parameter(Mandatory)]$Document,
    [Parameter(Mandatory)][string]$ExpectedVersion,
    [Parameter(Mandatory)][string]$ExpectedUrl,
    [Parameter(Mandatory)][string]$ExpectedSignature
  )

  if ([string]$Document.version -ne $ExpectedVersion) {
    throw "updater feed 版本校验失败：$($Document.version)"
  }
  $windows = $Document.platforms.'windows-x86_64'
  if ($null -eq $windows) {
    throw 'updater feed 缺少 windows-x86_64 平台。'
  }
  if ([string]$windows.url -ne $ExpectedUrl) {
    throw 'updater feed 安装包 URL 校验失败。'
  }
  if ([string]$windows.signature -ne $ExpectedSignature) {
    throw 'updater feed 签名校验失败。'
  }
}

$repositoryRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $repositoryRoot

$package = Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json
$tauriConfig = Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json
$cargoManifest = Get-Content -LiteralPath 'src-tauri/Cargo.toml' -Raw
$cargoPackageMatch = [regex]::Match(
  $cargoManifest,
  '(?ms)^\[package\]\s*.*?^version\s*=\s*"([^"]+)"'
)
if (-not $cargoPackageMatch.Success) {
  throw '无法读取 src-tauri/Cargo.toml 的 package.version。'
}
if ([string]::IsNullOrWhiteSpace($Version)) {
  $Version = [string]$package.version
}
$Version = $Version -replace '^v', ''
if ($Version -ne [string]$package.version) {
  throw "标签版本 $Version 与 package.json 版本 $($package.version) 不一致。"
}
if ($Version -ne [string]$tauriConfig.version) {
  throw "标签版本 $Version 与 src-tauri/tauri.conf.json 版本 $($tauriConfig.version) 不一致。"
}
if ($Version -ne [string]$cargoPackageMatch.Groups[1].Value) {
  throw "标签版本 $Version 与 src-tauri/Cargo.toml 版本 $($cargoPackageMatch.Groups[1].Value) 不一致。"
}
$semanticVersion = ConvertTo-SemanticVersion $Version

$tagName = "v$Version"
$productName = '自动点击流程台'
$repositoryPath = 'nbshree/shree-macro-flow-tauri'
$apiBase = "https://gitee.com/api/v5/repos/$repositoryPath"
$releasePageUrl = "https://gitee.com/$repositoryPath/releases/tag/$tagName"
$updaterBranch = 'updater-feed'
$updaterFeedPath = 'latest.json'
$updaterFeedUrl = "https://gitee.com/$repositoryPath/raw/$updaterBranch/$updaterFeedPath"
$sourceAttachmentName = "${productName}_${Version}_x64-setup.exe"
$uploadAttachmentName = "macro-flow_${Version}_x64-setup.exe"
$uploadSignatureName = "$uploadAttachmentName.sig"
$uploadFeedName = 'latest.json'
$installerPath = Join-Path `
  $repositoryRoot `
  "src-tauri\target\release\bundle\nsis\$sourceAttachmentName"
$signaturePath = "$installerPath.sig"

$localTag = git tag --list $tagName
if ($LASTEXITCODE -ne 0 -or $localTag -ne $tagName) {
  throw "本地不存在标签 $tagName，请先创建并推送该标签。"
}
$localTagCommit = (git rev-list -n 1 $tagName).Trim()
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($localTagCommit)) {
  throw "无法解析本地标签 $tagName 指向的提交。"
}
$remoteRepositories = [ordered]@{
  Gitee = "https://gitee.com/$repositoryPath.git"
  GitHub = "https://github.com/$repositoryPath.git"
}
foreach ($remoteRepository in $remoteRepositories.GetEnumerator()) {
  $remoteTagCommit = Get-RemoteTagCommit `
    -RepositoryUrl $remoteRepository.Value `
    -TagName $tagName
  if ($remoteTagCommit -ne $localTagCommit) {
    throw "$($remoteRepository.Key) 标签 $tagName 未指向本地提交 $localTagCommit。"
  }
}

if (-not $SkipBuild) {
  if ([string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY)) {
    throw '正式构建缺少 TAURI_SIGNING_PRIVATE_KEY。'
  }
  if ([string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD)) {
    throw '正式构建缺少 TAURI_SIGNING_PRIVATE_KEY_PASSWORD。'
  }

  pnpm typecheck
  if ($LASTEXITCODE -ne 0) { throw 'TypeScript 检查失败。' }

  pnpm test
  if ($LASTEXITCODE -ne 0) { throw '测试失败。' }

  cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
  if ($LASTEXITCODE -ne 0) { throw 'Rust 格式检查失败。' }

  cargo check --manifest-path src-tauri/Cargo.toml
  if ($LASTEXITCODE -ne 0) { throw 'Rust 编译检查失败。' }

  cargo test --manifest-path src-tauri/Cargo.toml
  if ($LASTEXITCODE -ne 0) { throw 'Rust 测试失败。' }

  cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
  if ($LASTEXITCODE -ne 0) { throw 'Rust Clippy 检查失败。' }

  pnpm tauri:build:release
  if ($LASTEXITCODE -ne 0) { throw 'Tauri 正式安装包构建失败。' }
}

if (-not (Test-Path -LiteralPath $installerPath)) {
  throw "安装包不存在：$installerPath"
}
if (-not (Test-Path -LiteralPath $signaturePath)) {
  throw "更新签名不存在：$signaturePath；请使用 pnpm tauri:build:release 正式构建。"
}

$signatureText = (Get-Content -LiteralPath $signaturePath -Raw).Trim()
if ([string]::IsNullOrWhiteSpace($signatureText)) {
  throw "更新签名为空：$signaturePath"
}

$installerHash = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash
$installerSize = [math]::Round((Get-Item -LiteralPath $installerPath).Length / 1MB, 2)
$previousTag = git tag --sort=-version:refname |
  Where-Object { $_ -ne $tagName } |
  Select-Object -First 1

if ($previousTag) {
  $changeLines = @(
    git -c i18n.logOutputEncoding=utf-8 log `
      --encoding=UTF-8 `
      --pretty=format:'- %s' `
      "$previousTag..$tagName"
  )
} else {
  $changeLines = @('- 首个公开发行版')
}

if ($changeLines.Count -eq 0) {
  $changeLines = @('- 稳定性改进和问题修复')
}

$changeText = $changeLines -join "`n"
$bootstrapNotice = if ($Version -eq '1.8.0') {
  @"
> ``v1.7.1`` 及更早版本尚未内置在线更新，需要手动下载安装本版本一次；从本版本开始，
> 后续发行版可以在应用内检查并安装。
"@
} else {
  ''
}
$releaseNotes = @"
## 更新内容

$changeText

$bootstrapNotice

## 下载与安装

下载附件 ``$uploadAttachmentName`` 后运行即可安装。支持应用内更新的版本也会通过签名校验后
下载同一个安装包。

- 系统要求：Windows 10/11 64 位
- 安装方式：当前用户安装，无需管理员权限
- 安装包大小：$installerSize MB
- SHA-256：``$installerHash``

> Tauri 更新签名用于验证在线更新来源，但当前安装程序未使用商业代码签名，Windows
> SmartScreen 仍可能显示安全提示。
"@

$tempBase = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$releaseTempDirectory = Join-Path $tempBase "macro-flow-release-$([Guid]::NewGuid().ToString('N'))"
$releaseTempFullPath = [IO.Path]::GetFullPath($releaseTempDirectory)
if (-not $releaseTempFullPath.StartsWith($tempBase, [StringComparison]::OrdinalIgnoreCase)) {
  throw '临时发行目录不在系统临时目录内。'
}
New-Item -ItemType Directory -Path $releaseTempFullPath | Out-Null

$uploadInstallerPath = Join-Path $releaseTempFullPath $uploadAttachmentName
$uploadSignaturePath = Join-Path $releaseTempFullPath $uploadSignatureName
$uploadFeedPath = Join-Path $releaseTempFullPath $uploadFeedName
$verifiedInstallerPath = Join-Path $releaseTempFullPath 'verified-installer.exe'
$verifiedSignaturePath = Join-Path $releaseTempFullPath 'verified-installer.exe.sig'
$verifiedFeedPath = Join-Path $releaseTempFullPath 'verified-latest.json'

Copy-Item -LiteralPath $installerPath -Destination $uploadInstallerPath
Copy-Item -LiteralPath $signaturePath -Destination $uploadSignaturePath

$secureToken = $null
$tokenPointer = [IntPtr]::Zero
$plainToken = $null

try {
  if ([string]::IsNullOrWhiteSpace($env:GITEE_TOKEN)) {
    $secureToken = Read-Host '请输入新的 Gitee 私人令牌' -AsSecureString
    $tokenPointer = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secureToken)
    $plainToken = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($tokenPointer)
  } else {
    $plainToken = $env:GITEE_TOKEN.Trim()
  }
  if ($plainToken -notmatch '^[0-9a-fA-F]{32}$') {
    throw 'Gitee 令牌格式无效。'
  }

  $headers = @{
    Authorization = "token $plainToken"
    Accept = 'application/json'
  }

  $user = Invoke-RestMethod `
    -Uri 'https://gitee.com/api/v5/user' `
    -Headers $headers `
    -Method Get
  if ($user.login -ne 'nbshree') {
    throw '令牌所属账号不是预期的 nbshree。'
  }

  $feedBranch = Get-GiteeBranch `
    -ApiBase $apiBase `
    -Headers $headers `
    -Branch $updaterBranch
  $existingFeedFile = if ($null -eq $feedBranch) {
    $null
  } else {
    Get-GiteeContentFile `
      -ApiBase $apiBase `
      -Headers $headers `
      -Branch $updaterBranch `
      -Path $updaterFeedPath
  }
  Assert-FeedVersionCanAdvance `
    -FeedFile $existingFeedFile `
    -NextVersion $semanticVersion `
    -AllowSameVersion ([bool]$RepairExisting)

  $existingRelease = Get-GiteeReleaseByTag `
    -ApiBase $apiBase `
    -Headers $headers `
    -TagName $tagName
  if ($existingRelease -and -not $RepairExisting) {
    throw "$tagName 的发行版已经存在；同版本只能使用 -RepairExisting 修复。"
  }

  $payload = @{
    tag_name = $tagName
    name = "$productName $tagName"
    body = $releaseNotes
    prerelease = $false
    target_commitish = 'main'
  } | ConvertTo-Json

  if ($existingRelease) {
    $release = Invoke-RestMethod `
      -Uri "$apiBase/releases/$($existingRelease.id)" `
      -Headers $headers `
      -Method Patch `
      -ContentType 'application/json; charset=utf-8' `
      -Body ([Text.Encoding]::UTF8.GetBytes($payload))
  } else {
    $release = Invoke-RestMethod `
      -Uri "$apiBase/releases" `
      -Headers $headers `
      -Method Post `
      -ContentType 'application/json; charset=utf-8' `
      -Body ([Text.Encoding]::UTF8.GetBytes($payload))
  }

  $managedAssetNames = @($uploadAttachmentName, $uploadSignatureName, $uploadFeedName)
  $managedAssets = @(Get-GiteeReleaseAssets `
      -ApiBase $apiBase `
      -Headers $headers `
      -ReleaseId $release.id) |
    Where-Object { $_.name -in $managedAssetNames }

  if ($managedAssets.Count -gt 0 -and -not $RepairExisting) {
    throw "$tagName 已存在自动更新附件，拒绝重复上传。"
  }
  if ($RepairExisting) {
    foreach ($asset in $managedAssets) {
      if ($null -eq $asset.id) {
        throw "Gitee 附件缺少 ID，无法安全替换：$($asset.name)"
      }
      Invoke-RestMethod `
        -Uri "$apiBase/releases/$($release.id)/attach_files/$($asset.id)" `
        -Headers $headers `
        -Method Delete | Out-Null
    }
  }

  $installerAttachment = Upload-GiteeReleaseAsset `
    -ApiBase $apiBase `
    -Headers $headers `
    -ReleaseId $release.id `
    -Path $uploadInstallerPath
  if ([string]::IsNullOrWhiteSpace([string]$installerAttachment.browser_download_url)) {
    throw 'Gitee 未返回安装包公开下载 URL。'
  }
  if ([string]$installerAttachment.name -ne $uploadAttachmentName) {
    throw "Gitee 安装包附件名称异常：$($installerAttachment.name)"
  }

  $feedDocument = [ordered]@{
    version = $Version
    notes = $changeText
    pub_date = [DateTimeOffset]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = [ordered]@{
      'windows-x86_64' = [ordered]@{
        url = [string]$installerAttachment.browser_download_url
        signature = $signatureText
      }
    }
  }
  $feedJson = $feedDocument | ConvertTo-Json -Depth 6
  [IO.File]::WriteAllText($uploadFeedPath, $feedJson, $utf8WithoutBom)

  $signatureAttachment = Upload-GiteeReleaseAsset `
    -ApiBase $apiBase `
    -Headers $headers `
    -ReleaseId $release.id `
    -Path $uploadSignaturePath
  $feedAttachment = Upload-GiteeReleaseAsset `
    -ApiBase $apiBase `
    -Headers $headers `
    -ReleaseId $release.id `
    -Path $uploadFeedPath
  if ([string]$signatureAttachment.name -ne $uploadSignatureName) {
    throw "Gitee 签名附件名称异常：$($signatureAttachment.name)"
  }
  if ([string]$feedAttachment.name -ne $uploadFeedName) {
    throw "Gitee feed 附件名称异常：$($feedAttachment.name)"
  }

  $verifiedAssets = @(Get-GiteeReleaseAssets `
      -ApiBase $apiBase `
      -Headers $headers `
      -ReleaseId $release.id)
  $verifiedInstaller = $verifiedAssets |
    Where-Object { $_.name -eq $uploadAttachmentName } |
    Select-Object -First 1
  $verifiedSignature = $verifiedAssets |
    Where-Object { $_.name -eq $uploadSignatureName } |
    Select-Object -First 1
  $verifiedFeed = $verifiedAssets |
    Where-Object { $_.name -eq $uploadFeedName } |
    Select-Object -First 1
  if ($null -eq $verifiedInstaller -or $null -eq $verifiedSignature -or $null -eq $verifiedFeed) {
    throw 'Gitee 发行版缺少 exe、sig 或 latest.json 附件。'
  }

  Invoke-WebRequest `
    -Uri $verifiedInstaller.browser_download_url `
    -OutFile $verifiedInstallerPath `
    -UseBasicParsing | Out-Null
  $publicInstallerHash = (Get-FileHash -LiteralPath $verifiedInstallerPath -Algorithm SHA256).Hash
  if ($publicInstallerHash -ne $installerHash) {
    throw "公开安装包 SHA-256 校验失败：$publicInstallerHash"
  }

  Invoke-WebRequest `
    -Uri $verifiedSignature.browser_download_url `
    -OutFile $verifiedSignaturePath `
    -UseBasicParsing | Out-Null
  $publicSignature = (Get-Content -LiteralPath $verifiedSignaturePath -Raw).Trim()
  if ($publicSignature -ne $signatureText) {
    throw '公开签名附件与本地签名不一致。'
  }

  Invoke-WebRequest `
    -Uri $verifiedFeed.browser_download_url `
    -OutFile $verifiedFeedPath `
    -UseBasicParsing | Out-Null
  $releaseFeedDocument = Get-Content -LiteralPath $verifiedFeedPath -Raw | ConvertFrom-Json
  Assert-UpdaterFeedDocument `
    -Document $releaseFeedDocument `
    -ExpectedVersion $Version `
    -ExpectedUrl $verifiedInstaller.browser_download_url `
    -ExpectedSignature $signatureText

  $feedBranch = Get-GiteeBranch `
    -ApiBase $apiBase `
    -Headers $headers `
    -Branch $updaterBranch
  if ($null -eq $feedBranch) {
    $feedBranch = Invoke-RestMethod `
      -Uri "$apiBase/branches" `
      -Headers $headers `
      -Method Post `
      -Body @{
        refs = 'main'
        branch_name = $updaterBranch
      }
  }

  $currentFeedFile = Get-GiteeContentFile `
    -ApiBase $apiBase `
    -Headers $headers `
    -Branch $updaterBranch `
    -Path $updaterFeedPath
  Assert-FeedVersionCanAdvance `
    -FeedFile $currentFeedFile `
    -NextVersion $semanticVersion `
    -AllowSameVersion ([bool]$RepairExisting)

  $encodedFeed = [Convert]::ToBase64String($utf8WithoutBom.GetBytes($feedJson))
  $feedCommit = @{
    content = $encodedFeed
    message = "chore: update updater feed to $tagName"
    branch = $updaterBranch
  }
  if ($null -eq $currentFeedFile) {
    Invoke-RestMethod `
      -Uri "$apiBase/contents/$updaterFeedPath" `
      -Headers $headers `
      -Method Post `
      -ContentType 'application/x-www-form-urlencoded' `
      -Body $feedCommit | Out-Null
  } else {
    $feedCommit.sha = [string]$currentFeedFile.sha
    Invoke-RestMethod `
      -Uri "$apiBase/contents/$updaterFeedPath" `
      -Headers $headers `
      -Method Put `
      -ContentType 'application/x-www-form-urlencoded' `
      -Body $feedCommit | Out-Null
  }

  $feedVerificationUrl = "${updaterFeedUrl}?verify=$([Guid]::NewGuid().ToString('N'))"
  $publicFeedDocument = Get-PublicJsonWithRetry `
    -Uri $feedVerificationUrl `
    -Validate {
      param($document)
      Assert-UpdaterFeedDocument `
        -Document $document `
        -ExpectedVersion $Version `
        -ExpectedUrl $verifiedInstaller.browser_download_url `
        -ExpectedSignature $signatureText
    }

  Write-Host ''
  Write-Host "发行成功：$releasePageUrl"
  Write-Host "安装包：$($verifiedInstaller.browser_download_url)"
  Write-Host "更新源：$updaterFeedUrl"
  Write-Host "SHA-256：$installerHash"

  $garbledAttachments = $verifiedAssets |
    Where-Object { $_.name -match '^=\?utf-8\?[bq]\?.+\?=$' }
  if ($garbledAttachments.Count -gt 0) {
    Write-Warning '检测到旧的乱码附件，请在 Gitee 发行版编辑页面中手动删除它。'
  }
} finally {
  if (Test-Path -LiteralPath $releaseTempFullPath) {
    Remove-Item -LiteralPath $releaseTempFullPath -Recurse -Force
  }
  if ($tokenPointer -ne [IntPtr]::Zero) {
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($tokenPointer)
  }
  $plainToken = $null
  Remove-Variable plainToken -ErrorAction SilentlyContinue
  Remove-Variable secureToken -ErrorAction SilentlyContinue
}

param(
  [string]$Version,
  [switch]$SkipBuild,
  [switch]$RepairExisting
)

$ErrorActionPreference = 'Stop'
$utf8 = [Text.UTF8Encoding]::new($false)
$OutputEncoding = $utf8
[Console]::OutputEncoding = $utf8

if ($PSVersionTable.PSVersion.Major -lt 7) {
  throw '请使用 PowerShell 7 或更高版本。'
}

function Invoke-GiteeJson {
  param([string]$Uri, [string]$Method = 'Get', $Body, [string]$ContentType)
  $arguments = @{ Uri = $Uri; Headers = $script:headers; Method = $Method }
  if ($null -ne $Body) { $arguments.Body = $Body }
  if ($ContentType) { $arguments.ContentType = $ContentType }
  Invoke-RestMethod @arguments
}

function Get-ReleaseByTag([string]$Tag) {
  try { Invoke-GiteeJson "$script:apiBase/releases/tags/$([Uri]::EscapeDataString($Tag))" }
  catch { if ([int]$_.Exception.Response.StatusCode -eq 404) { return $null }; throw }
}

function Get-ContentFile([string]$Branch, [string]$Path) {
  try { Invoke-GiteeJson "$script:apiBase/contents/$Path`?ref=$([Uri]::EscapeDataString($Branch))" }
  catch { if ([int]$_.Exception.Response.StatusCode -eq 404) { return $null }; throw }
}

function Upload-Asset($ReleaseId, [string]$Path) {
  Invoke-RestMethod -Uri "$script:apiBase/releases/$ReleaseId/attach_files" `
    -Headers $script:headers -Method Post -Form @{ file = Get-Item -LiteralPath $Path }
}

$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root
$package = Get-Content package.json -Raw | ConvertFrom-Json
$tauri = Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json
$cargo = Get-Content src-tauri/Cargo.toml -Raw
$cargoVersion = [regex]::Match($cargo, '(?ms)^\[package\].*?^version\s*=\s*"([^"]+)"').Groups[1].Value
if ([string]::IsNullOrWhiteSpace($Version)) { $Version = [string]$package.version }
$Version = $Version -replace '^v', ''
if ($Version -ne $package.version -or $Version -ne $tauri.version -or $Version -ne $cargoVersion) {
  throw "版本不一致：tag=$Version package=$($package.version) tauri=$($tauri.version) cargo=$cargoVersion"
}

$tag = "v$Version"
$repository = 'nbshree/buff-sentinel'
$apiBase = "https://gitee.com/api/v5/repos/$repository"
$installerName = "Buff 哨兵_${Version}_x64-setup.exe"
$builtInstaller = "src-tauri/target/release/bundle/nsis/$installerName"
$builtSignature = "$builtInstaller.sig"
$assetName = "buff-sentinel_${Version}_x64-setup.exe"
$signatureName = "$assetName.sig"

if ((git tag --list $tag) -ne $tag) { throw "本地不存在标签 $tag。" }
$localCommit = (git rev-list -n 1 $tag).Trim()
$remoteTag = git ls-remote https://gitee.com/$repository.git "refs/tags/$tag^{}"
if (-not $remoteTag -or ($remoteTag -split '\s+')[0] -ne $localCommit) {
  throw "Gitee 标签 $tag 未指向本地提交 $localCommit。"
}

if (-not $SkipBuild) {
  if ([string]::IsNullOrWhiteSpace($env:TAURI_SIGNING_PRIVATE_KEY)) { throw '缺少 TAURI_SIGNING_PRIVATE_KEY。' }
  pnpm typecheck; if ($LASTEXITCODE) { throw 'TypeScript 检查失败。' }
  pnpm test; if ($LASTEXITCODE) { throw '测试失败。' }
  cargo fmt --manifest-path src-tauri/Cargo.toml -- --check; if ($LASTEXITCODE) { throw 'Rust fmt 失败。' }
  cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings; if ($LASTEXITCODE) { throw 'Rust clippy 失败。' }
  pnpm tauri:build:release; if ($LASTEXITCODE) { throw 'Tauri 构建失败。' }
}

if (-not (Test-Path -LiteralPath $builtInstaller) -or -not (Test-Path -LiteralPath $builtSignature)) {
  throw '缺少正式安装包或 updater 签名。'
}
$signature = (Get-Content -LiteralPath $builtSignature -Raw).Trim()
$hash = (Get-FileHash -LiteralPath $builtInstaller -Algorithm SHA256).Hash
$previousTag = git tag --sort=-version:refname | Where-Object { $_ -ne $tag } | Select-Object -First 1
$changes = if ($previousTag) { @(git log --pretty=format:'- %s' "$previousTag..$tag") } else { @('- 首个公开发行版') }
if (-not $changes) { $changes = @('- 稳定性改进和问题修复') }
$notes = $changes -join "`n"
$releaseBody = "## 更新内容`n`n$notes`n`n## 下载与安装`n`n- Windows 10/11 64 位`n- SHA-256：``$hash``"

$token = $env:GITEE_TOKEN
if ([string]::IsNullOrWhiteSpace($token)) { $token = Read-Host 'Gitee 私人令牌' }
$headers = @{ Authorization = "token $($token.Trim())"; Accept = 'application/json' }
$user = Invoke-GiteeJson 'https://gitee.com/api/v5/user'
if ($user.login -ne 'nbshree') { throw 'Gitee token 账号不是 nbshree。' }

$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$temp = Join-Path $tempRoot "buff-sentinel-release-$([Guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $temp | Out-Null
try {
  $installer = Join-Path $temp $assetName
  $signatureFile = Join-Path $temp $signatureName
  $feedFile = Join-Path $temp 'latest.json'
  Copy-Item -LiteralPath $builtInstaller -Destination $installer
  Copy-Item -LiteralPath $builtSignature -Destination $signatureFile

  $release = Get-ReleaseByTag $tag
  if ($release -and -not $RepairExisting) { throw "$tag 的 Gitee Release 已存在。" }
  $payload = @{ tag_name = $tag; name = "Buff 哨兵 $tag"; body = $releaseBody; prerelease = $false; target_commitish = 'main' } | ConvertTo-Json
  if ($release) {
    $release = Invoke-GiteeJson "$apiBase/releases/$($release.id)" Patch ([Text.Encoding]::UTF8.GetBytes($payload)) 'application/json; charset=utf-8'
  } else {
    $release = Invoke-GiteeJson "$apiBase/releases" Post ([Text.Encoding]::UTF8.GetBytes($payload)) 'application/json; charset=utf-8'
  }

  if ($RepairExisting) {
    $assets = @(Invoke-GiteeJson "$apiBase/releases/$($release.id)/attach_files?per_page=100")
    foreach ($asset in $assets | Where-Object { $_.name -in @($assetName, $signatureName, 'latest.json') }) {
      Invoke-GiteeJson "$apiBase/releases/$($release.id)/attach_files/$($asset.id)" Delete | Out-Null
    }
  }

  $uploadedInstaller = Upload-Asset $release.id $installer
  $feed = [ordered]@{
    version = $Version
    notes = $notes
    pub_date = [DateTimeOffset]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = [ordered]@{ 'windows-x86_64' = [ordered]@{ url = $uploadedInstaller.browser_download_url; signature = $signature } }
  }
  $feedJson = $feed | ConvertTo-Json -Depth 6
  [IO.File]::WriteAllText($feedFile, $feedJson, $utf8)
  Upload-Asset $release.id $signatureFile | Out-Null
  Upload-Asset $release.id $feedFile | Out-Null

  $branch = try { Invoke-GiteeJson "$apiBase/branches/updater-feed" } catch { $null }
  if (-not $branch) {
    Invoke-GiteeJson "$apiBase/branches" Post @{ refs = 'main'; branch_name = 'updater-feed' } | Out-Null
  }
  $currentFeed = Get-ContentFile 'updater-feed' 'latest.json'
  if ($currentFeed) {
    $currentJson = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String(($currentFeed.content -replace '\s', ''))) | ConvertFrom-Json
    if ([version]$currentJson.version -gt [version]$Version) { throw '拒绝用旧版本覆盖 updater feed。' }
    if ([version]$currentJson.version -eq [version]$Version -and -not $RepairExisting) { throw '同版本 feed 只能修复。' }
  }
  $commit = @{ content = [Convert]::ToBase64String($utf8.GetBytes($feedJson)); message = "chore: update updater feed to $tag"; branch = 'updater-feed' }
  if ($currentFeed) {
    $commit.sha = $currentFeed.sha
    Invoke-GiteeJson "$apiBase/contents/latest.json" Put $commit 'application/x-www-form-urlencoded' | Out-Null
  } else {
    Invoke-GiteeJson "$apiBase/contents/latest.json" Post $commit 'application/x-www-form-urlencoded' | Out-Null
  }
  Write-Host "发行成功：https://gitee.com/$repository/releases/tag/$tag"
  Write-Host "更新源：https://gitee.com/$repository/raw/updater-feed/latest.json"
} finally {
  if (Test-Path -LiteralPath $temp) { Remove-Item -LiteralPath $temp -Recurse -Force }
}

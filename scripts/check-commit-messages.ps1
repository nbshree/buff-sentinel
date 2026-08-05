param(
  [string]$MessageFile,
  [string]$CommitRange,
  [switch]$SincePolicyStart,
  [string]$ToRef = 'HEAD'
)

$ErrorActionPreference = 'Stop'
$utf8WithoutBom = [Text.UTF8Encoding]::new($false)
$OutputEncoding = $utf8WithoutBom
[Console]::OutputEncoding = $utf8WithoutBom

if ($PSVersionTable.PSVersion.Major -lt 7) {
  throw '请使用 PowerShell 7 或更高版本运行此脚本。'
}

$allowedCategories = @(
  '新增',
  '修复',
  '优化',
  '重构',
  '文档',
  '测试',
  '构建',
  '发布',
  '维护',
  '合并'
)
$categoryPattern = $allowedCategories -join '|'
$subjectPattern = "^(?:$categoryPattern)(?:（[^）]+）)?：\S.*[\p{IsCJKUnifiedIdeographs}]"
$releaseSubjectPattern = '^发布：v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$'

function Test-CommitSubject {
  param(
    [Parameter(Mandatory)][string]$Subject,
    [string]$Commit = ''
  )

  if ($Subject -match $subjectPattern -or $Subject -match $releaseSubjectPattern) {
    return $true
  }

  $location = if ([string]::IsNullOrWhiteSpace($Commit)) { '' } else { " [$Commit]" }
  Write-Host "提交标题不符合中文规范$location：$Subject" -ForegroundColor Red
  Write-Host '请使用“分类：中文说明”，例如“新增：支持连招方案导入”或“修复（更新器）：避免重复安装”。' -ForegroundColor Red
  Write-Host "允许的分类：$($allowedCategories -join '、')。" -ForegroundColor Red
  return $false
}

if (-not [string]::IsNullOrWhiteSpace($MessageFile)) {
  if (-not (Test-Path -LiteralPath $MessageFile)) {
    throw "提交信息文件不存在：$MessageFile"
  }

  $subject = Get-Content -LiteralPath $MessageFile -Encoding utf8 |
    Where-Object { $_ -notmatch '^\s*#' -and -not [string]::IsNullOrWhiteSpace($_) } |
    Select-Object -First 1
  if ([string]::IsNullOrWhiteSpace($subject)) {
    throw '提交标题不能为空。'
  }
  if (-not (Test-CommitSubject -Subject $subject)) {
    exit 1
  }
  exit 0
}

if ($SincePolicyStart) {
  $policyStart = git log `
    --diff-filter=A `
    --reverse `
    --format='%H' `
    $ToRef `
    -- scripts/check-commit-messages.ps1 |
    Select-Object -First 1
  if ($LASTEXITCODE -ne 0) {
    throw '无法查找中文提交规范的启用提交。'
  }
  if ([string]::IsNullOrWhiteSpace($policyStart)) {
    Write-Host '当前引用尚未启用中文提交规范，跳过历史提交检查。'
    exit 0
  }
  $CommitRange = "$policyStart^..$ToRef"
}

if ([string]::IsNullOrWhiteSpace($CommitRange)) {
  throw '请指定 -MessageFile、-CommitRange 或 -SincePolicyStart。'
}

$records = @(git -c i18n.logOutputEncoding=utf-8 log `
  --encoding=UTF-8 `
  --format='%H%x09%s' `
  $CommitRange)
if ($LASTEXITCODE -ne 0) {
  throw "无法读取提交范围：$CommitRange"
}

$hasInvalidSubject = $false
foreach ($record in $records) {
  $parts = $record -split "`t", 2
  if ($parts.Count -ne 2) {
    continue
  }
  if (-not (Test-CommitSubject -Commit $parts[0].Substring(0, 8) -Subject $parts[1])) {
    $hasInvalidSubject = $true
  }
}

if ($hasInvalidSubject) {
  exit 1
}

Write-Host "中文提交标题检查通过，共检查 $($records.Count) 个提交。"

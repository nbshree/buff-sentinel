$ErrorActionPreference = 'Stop'

git config core.hooksPath .githooks
if ($LASTEXITCODE -ne 0) {
  throw '配置 Git 提交钩子失败。'
}

Write-Host '已启用仓库 Git 钩子；后续提交标题必须使用中文规范。'

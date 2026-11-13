# 将本地仓库绑定到 Gitee（可选：创建仓库并推送）
param(
    [string]$Owner = $env:GITEE_OWNER,
    [string]$Repo = 'Fretec-StudyHall',
    [string]$Token = $env:GITEE_TOKEN,
    [switch]$Private = $true,
    [switch]$Push
)

$ErrorActionPreference = 'Stop'
$api = 'https://gitee.com/api/v5'

if (-not $Token) { throw '缺少 Gitee 访问令牌：请设置环境变量 GITEE_TOKEN 或传 -Token' }
if (-not $Owner) {
    $user = Invoke-RestMethod -Method Get -Uri "$api/user?access_token=$Token"
    $Owner = $user.login
}
if (-not $Owner) { throw '无法解析 Gitee 用户名' }

$encodedRepo = [uri]::EscapeDataString($Repo)
$exists = $false
try {
    Invoke-RestMethod -Method Get -Uri "$api/repos/$Owner/$encodedRepo?access_token=$Token" | Out-Null
    $exists = $true
} catch {
    $exists = $false
}

if (-not $exists) {
    $body = @{
        access_token = $Token
        name         = $Repo
        private      = [bool]$Private
        auto_init    = $false
        has_issues   = $true
    }
    Invoke-RestMethod -Method Post -Uri "$api/user/repos" -Body $body | Out-Null
    Write-Host "已在 Gitee 创建仓库：$Owner/$Repo"
} else {
    Write-Host "Gitee 仓库已存在：$Owner/$Repo"
}

$remoteUrl = "https://gitee.com/$Owner/$Repo.git"
$existingRemote = git remote get-url gitee 2>$null
if ($LASTEXITCODE -eq 0) {
    git remote set-url gitee $remoteUrl
} else {
    git remote add gitee $remoteUrl
}
Write-Host "remote gitee => $remoteUrl"

if ($Push) {
    $authed = "https://${Owner}:${Token}@gitee.com/$Owner/$Repo.git"
    git -c credential.helper= push $authed HEAD:refs/heads/main
    if ($LASTEXITCODE -ne 0) { throw '推送失败' }
    Write-Host '已推送到 Gitee'
}

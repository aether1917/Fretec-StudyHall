# Create / bind the Gitee repository and optionally push.
param(
    [string]$Owner = $env:GITEE_OWNER,
    [string]$Repo = 'Fretec-StudyHall',
    [string]$Token = $env:GITEE_TOKEN,
    [switch]$Private = $true,
    [switch]$Push
)

$ErrorActionPreference = 'Stop'
$api = 'https://gitee.com/api/v5'

if (-not $Token) { throw 'missing token: set GITEE_TOKEN or pass -Token' }
if (-not $Owner) {
    $user = Invoke-RestMethod -Method Get -Uri "$api/user?access_token=$Token"
    $Owner = $user.login
}
if (-not $Owner) { throw 'cannot resolve Gitee owner' }

$encodedRepo = [uri]::EscapeDataString($Repo)
$exists = $false
try {
    Invoke-RestMethod -Method Get -Uri "$api/repos/$Owner/$encodedRepo`?access_token=$Token" | Out-Null
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
    Write-Host "created Gitee repo: $Owner/$Repo"
} else {
    Write-Host "Gitee repo exists: $Owner/$Repo"
}

$remoteUrl = "https://gitee.com/$Owner/$Repo.git"
$remotes = @(git remote)
if ($remotes -contains 'gitee') {
    git remote set-url gitee $remoteUrl
} else {
    git remote add gitee $remoteUrl
}
Write-Host "remote gitee => $remoteUrl"

if ($Push) {
    $authed = "https://${Owner}:${Token}@gitee.com/$Owner/$Repo.git"
    git -c credential.helper= push $authed HEAD:refs/heads/main
    if ($LASTEXITCODE -ne 0) { throw 'push failed' }
    Write-Host 'pushed to Gitee'
}

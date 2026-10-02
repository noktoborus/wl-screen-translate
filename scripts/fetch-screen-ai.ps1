# Downloads the Chrome Screen AI library from Google's package server into
# $Data\screen-ai, or copies it from a Chrome profile with -FromChrome.
param([switch]$FromChrome)
. "$PSScriptRoot\common.ps1"

$Target = Join-Path $Data 'screen-ai'
$ChromeComponent = if ($env:CHROME_COMPONENT) { $env:CHROME_COMPONENT } else {
    Join-Path $env:LOCALAPPDATA 'Google\Chrome\User Data\screen_ai'
}
$Api = 'https://chrome-infra-packages.appspot.com/prpc/cipd.Repository'
$Package = if ($env:PACKAGE) { $env:PACKAGE } else { 'chromium/third_party/screen-ai/windows-amd64' }

if ($FromChrome) {
    # Chrome keeps one directory per version; the newest is taken.
    $source = Get-ChildItem -Directory $ChromeComponent -ErrorAction SilentlyContinue |
        Sort-Object { [version]$_.Name } | Select-Object -Last 1
    if (-not $source) { throw "no Screen AI component in $ChromeComponent" }
    New-Item -ItemType Directory -Force -Path $Target | Out-Null
    Copy-Item -Recurse -Force "$($source.FullName)\*" $Target
    Write-Host "copied $($source.FullName) to $Target"
    exit 0
}

# The server answers JSON behind a ")]}'" line.
function Invoke-Prpc($Method, $Body) {
    $answer = Invoke-WebRequest -UseBasicParsing -Method Post -Uri "$Api/$Method" `
        -ContentType 'application/json' -Headers @{ Accept = 'application/json' } -Body $Body
    ($answer.Content -split "`n", 2)[1] | ConvertFrom-Json
}

$work = New-WorkDirectory
try {
    $instance = (Invoke-Prpc ResolveVersion (@{ package = $Package; version = 'latest' } | ConvertTo-Json)).instance
    $url = (Invoke-Prpc GetInstanceURL (@{ package = $Package; instance = $instance } | ConvertTo-Json)).signedUrl
    Get-File $url "$work\screen-ai.zip"
    Expand-Archive -Path "$work\screen-ai.zip" -DestinationPath "$work\package"
    New-Item -ItemType Directory -Force -Path $Target | Out-Null
    Copy-Item -Recurse -Force "$work\package\resources\*" $Target
    Write-Host "Screen AI is in $Target"
} finally {
    Remove-Item -Recurse -Force $work
}

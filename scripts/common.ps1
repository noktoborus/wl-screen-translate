# Shared by the scripts: the data directory of the program and small helpers.

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'  # the progress bar slows downloads down

$AppId = 'wl-screen-translate'
# The directory the program reads its models from; DATA overrides it.
$Data = if ($env:DATA) { $env:DATA } else { Join-Path $env:APPDATA "$AppId\data" }

function Get-File($Url, $Path) {
    Write-Host "downloading $Url"
    Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $Path
}

function New-WorkDirectory {
    $work = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $work | Out-Null
    $work
}

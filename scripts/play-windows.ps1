$ErrorActionPreference = 'Stop'
$gameRoot = $PSScriptRoot
$lockStream = $null
try {
    $lockStream = [System.IO.File]::Open((Join-Path $gameRoot '.launch.lock'),
        [System.IO.FileMode]::OpenOrCreate, [System.IO.FileAccess]::ReadWrite,
        [System.IO.FileShare]::None)
} catch [System.IO.IOException] {
    Write-Host 'Hard Time is already running or being updated.'
    exit 0
}
try {
    $dataPath = Join-Path $gameRoot 'Data'
    $dataItem = Get-Item -LiteralPath $dataPath
    if (-not $dataItem.PSIsContainer -or
        ($dataItem.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) {
        throw 'A private Data directory is required.'
    }
    $runStamp = (Get-Date -Format 'yyyyMMdd-HHmmss-fff') + '-' + [guid]::NewGuid().ToString('N').Substring(0, 8)
    $backupRoot = Join-Path $gameRoot 'backups'
    $logRoot = Join-Path $gameRoot 'logs'
    [System.IO.Directory]::CreateDirectory($backupRoot) | Out-Null
    [System.IO.Directory]::CreateDirectory($logRoot) | Out-Null
    Copy-Item -LiteralPath $dataPath -Destination (Join-Path $backupRoot $runStamp) -Recurse
    $logPath = Join-Path $logRoot ('run-' + $runStamp + '.log')
    $errorPath = Join-Path $logRoot ('run-' + $runStamp + '.errors.log')
    if (Test-Path -LiteralPath (Join-Path $gameRoot 'software-renderer.txt')) {
        $env:GALLIUM_DRIVER = 'llvmpipe'
    }
    $gameProcess = Start-Process -FilePath (Join-Path $gameRoot 'Hard Time.exe') `
        -WorkingDirectory $gameRoot -WindowStyle Normal -PassThru -Wait `
        -RedirectStandardOutput $logPath -RedirectStandardError $errorPath
    $gameExitCode = $gameProcess.ExitCode
    Add-Content -LiteralPath $logPath -Value ('Exit status: ' + $gameExitCode)
    Set-Content -LiteralPath (Join-Path $logRoot 'latest.txt') -Value @($logPath, $errorPath)
    if ($gameExitCode -ne 0) {
        Write-Host ('Hard Time exited with status ' + $gameExitCode + '. See ' + $logRoot)
    }
    exit $gameExitCode
} finally {
    if ($lockStream) { $lockStream.Dispose() }
}

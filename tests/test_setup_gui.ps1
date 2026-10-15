param(
    [string]$Setup = (Join-Path $PSScriptRoot '..\dist\Hard-Time-Setup.exe'),
    [switch]$DownloadOriginal
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$setupPath = (Resolve-Path -LiteralPath $Setup).Path
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$evidence = Join-Path $repoRoot ('.work\validation\setup\gui-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
[System.IO.Directory]::CreateDirectory($evidence) | Out-Null
$assembly = [System.Reflection.Assembly]::LoadFrom($setupPath)
$windowType = $assembly.GetType('HardTimeSetup.SetupWindow', $true)
$flags = [System.Reflection.BindingFlags]'Instance, NonPublic'
[System.Windows.Forms.Application]::EnableVisualStyles()
$window = [Activator]::CreateInstance($windowType, $true)
function Field([string]$name) { $windowType.GetField($name, $flags).GetValue($window) }
function Capture([string]$name) {
    $bitmap = New-Object System.Drawing.Bitmap $window.Width, $window.Height
    try {
        $window.DrawToBitmap($bitmap, (New-Object System.Drawing.Rectangle 0, 0, $window.Width, $window.Height))
        $bitmap.Save((Join-Path $evidence $name), [System.Drawing.Imaging.ImageFormat]::Png)
    } finally { $bitmap.Dispose() }
}
try {
    $window.Opacity = 0
    $window.ShowInTaskbar = $false
    $window.Show()
    [System.Windows.Forms.Application]::DoEvents()
    # Capture the real controls with example paths, then install into a disposable folder.
    (Field 'source').Text = 'C:\Users\Player\Downloads\hardtime_setup.exe'
    (Field 'destination').Text = 'C:\Users\Player\AppData\Local\Programs\Hard Time'
    (Field 'downloadOriginal').Checked = $true
    Capture 'setup.png'
    (Field 'destination').Text = Join-Path $evidence 'Installed game'
    (Field 'shortcuts').Checked = $false
    (Field 'source').Text = Join-Path $env:USERPROFILE 'Downloads\hardtime_setup.exe'
    (Field 'downloadOriginal').Checked = [bool]$DownloadOriginal
    (Field 'install').PerformClick()
    $deadline = (Get-Date).AddSeconds(120)
    while ((Field 'cancellation') -and (Get-Date) -lt $deadline) {
        [System.Windows.Forms.Application]::DoEvents()
        Start-Sleep -Milliseconds 50
    }
    [System.Windows.Forms.Application]::DoEvents()
    $installed = Field 'installed'
    if (-not $installed) { throw ('GUI installation failed: ' + (Field 'status').Text) }
    if ((Field 'install').Text -ne 'Play Hard Time') { throw 'The success action was not displayed.' }
    Capture 'installed.png'
    # Exercise the actual completion button, including the compiled launcher.
    (Field 'install').PerformClick()
    $gamePath = Join-Path $installed.destination 'Hard Time.exe'
    $deadline = (Get-Date).AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 300
        $game = Get-Process -Name 'Hard Time' -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $gamePath } | Select-Object -First 1
    } while (-not $game -and (Get-Date) -lt $deadline)
    if (-not $game) { throw 'The Play button did not start the installed game.' }
    $record = @{ destination = $installed.destination; renderer = $installed.renderer; download_tested = [bool]$DownloadOriginal; game_process = $game.Id; screenshot = (Join-Path $evidence 'setup.png') }
    $record | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $repoRoot '.work\validation\setup\gui-latest.json') -Encoding UTF8
    Write-Output ('PASS GUI install, completion button and launcher; game PID ' + $game.Id)
    Write-Output ('Evidence: ' + $evidence)
} finally {
    if (-not $window.IsDisposed) { $window.Close(); $window.Dispose() }
}

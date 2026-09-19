<#
.SYNOPSIS
    Smoke-tests a built IEEE 1815.2 Test Tool Windows installer end to end.
.DESCRIPTION
    Installs the tool per-user and silently, launches it, checks the API
    health route and the served UI, saves a profile through the API and
    checks where it landed, stops every process it started, then uninstalls
    and checks the install is gone while the per-user data survives.
    Needs no admin rights: everything here uses the per-user install mode.
.PARAMETER SetupExe
    Path to the built ieee-1815-2-test-tool-setup.exe.
.PARAMETER Port
    Port to run the tool on for this test. Defaults away from the tool's
    own default (8000) so a test run does not collide with one already
    running.
.PARAMETER TimeoutSeconds
    How long to wait for /api/health to answer after launch.
.PARAMETER KeepInstalled
    Skip the final uninstall step, so the install is left in place for
    manual poking. Everything the script started is still stopped.
.EXAMPLE
    Needs no admin rights; everything installs and runs per-user.
    Download the CI artifact, then run:
        gh run download <run-id> -n ieee-1815-2-test-tool-setup -D .
        powershell -File installer\smoke-test.ps1 -SetupExe .\ieee-1815-2-test-tool-setup.exe
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$SetupExe,

    [int]$Port = 47610,

    [int]$TimeoutSeconds = 60,

    [switch]$KeepInstalled
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# The .iss AppId (installer\ieee-1815-2-test-tool.iss), with the doubled
# brace unescaped to a literal one. Inno names the per-user uninstall
# registry key "<AppId>_is1" under HKCU (INFERRED from Inno convention).
$script:AppId = '{e6851846-fa2b-4b89-93a6-cb0d1dd6d0e6}'
$script:UninstallKeyPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$($script:AppId)_is1"
$script:DataRoot = Join-Path $env:LOCALAPPDATA 'ieee-1815-2-test-tool'
$script:LogDir = Join-Path $script:DataRoot 'logs'
$script:ToolExeNames = @('web_server.exe', 'reference-outstation.exe', 'reference-control-station.exe')

$script:PassCount = 0
$script:FailCount = 0
$script:InstallLogPath = Join-Path $env:TEMP "smoke-test-install-$($PID).log"
$script:UninstallLogPath = Join-Path $env:TEMP "smoke-test-uninstall-$($PID).log"

function Write-CheckResult {
    param(
        [string]$Name,
        [bool]$Passed,
        [string]$Reason = ''
    )
    if ($Passed) {
        $script:PassCount++
        Write-Host "PASS $Name"
    }
    else {
        $script:FailCount++
        Write-Host "FAIL ${Name}: $Reason"
    }
}

function Test-PortListening {
    param([int]$TestPort)
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $asyncResult = $client.BeginConnect('127.0.0.1', $TestPort, $null, $null)
        $connected = $asyncResult.AsyncWaitHandle.WaitOne(200)
        return ($connected -and $client.Connected)
    }
    catch {
        return $false
    }
    finally {
        $client.Close()
    }
}

function Get-ToolProcesses {
    # Matches by executable path under the install, so a same-named process
    # the user has running elsewhere for unrelated reasons is never touched.
    param([string]$InstallLocation)
    $prefix = $InstallLocation.TrimEnd('\') + '\'
    $found = @()
    foreach ($exeName in $script:ToolExeNames) {
        $procMatches = @(Get-CimInstance -ClassName Win32_Process -Filter "Name='$exeName'")
        foreach ($proc in $procMatches) {
            if ($proc.ExecutablePath -and $proc.ExecutablePath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
                $found += $proc
            }
        }
    }
    return $found
}

function Stop-ToolProcesses {
    param([string]$InstallLocation, [System.Diagnostics.Process]$LauncherProcess)
    if ($InstallLocation) {
        foreach ($proc in (Get-ToolProcesses -InstallLocation $InstallLocation)) {
            try {
                Stop-Process -Id $proc.ProcessId -Force -ErrorAction Stop
            }
            catch {
                Write-Warning "could not stop $($proc.Name) (PID $($proc.ProcessId)): $($_.Exception.Message)"
            }
        }
    }
    # launch.cmd's own :fail path calls "pause" and waits for a keypress;
    # kill the launcher rather than assume it already exited on its own.
    if ($LauncherProcess -and -not $LauncherProcess.HasExited) {
        try {
            Stop-Process -Id $LauncherProcess.Id -Force -ErrorAction Stop
        }
        catch {
            Write-Warning "could not stop the launcher (PID $($LauncherProcess.Id)): $($_.Exception.Message)"
        }
    }
}

function Wait-ForHealth {
    param([int]$TestPort, [int]$MaxSeconds)
    $deadline = (Get-Date).AddSeconds($MaxSeconds)
    $lastReason = 'timed out waiting for a response'
    while ((Get-Date) -lt $deadline) {
        try {
            $response = Invoke-RestMethod -Uri "http://127.0.0.1:$TestPort/api/health" -TimeoutSec 5
            if ($response.status -eq 'ok') {
                return @{ Ok = $true; Reason = '' }
            }
            $lastReason = "unexpected health payload: $($response | ConvertTo-Json -Compress)"
        }
        catch {
            $lastReason = $_.Exception.Message
        }
        Start-Sleep -Milliseconds 500
    }
    return @{ Ok = $false; Reason = $lastReason }
}

function Invoke-BestEffortUninstall {
    param([string]$InstallLocation)
    if (-not $InstallLocation -or -not (Test-Path -LiteralPath $InstallLocation)) {
        return @{ Passed = $false; Reason = 'no install location to uninstall from' }
    }
    # Inno Setup always names its generated uninstaller unins000.exe unless
    # the script defines a second uninstall log mode, which this one does
    # not (INFERRED from Inno convention).
    $uninstaller = Join-Path $InstallLocation 'unins000.exe'
    if (-not (Test-Path -LiteralPath $uninstaller)) {
        return @{ Passed = $false; Reason = "uninstaller not found: $uninstaller" }
    }
    $uninstallArgs = @(
        '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART',
        "/LOG=`"$($script:UninstallLogPath)`""
    )
    $uninstallProc = Start-Process -FilePath $uninstaller -ArgumentList $uninstallArgs -PassThru -Wait
    $exitOk = ($uninstallProc.ExitCode -eq 0)
    $goneOk = -not (Test-Path -LiteralPath $InstallLocation)
    $dataOk = Test-Path -LiteralPath $script:DataRoot
    $passed = ($exitOk -and $goneOk -and $dataOk)
    $reason = ''
    if (-not $passed) {
        $reason = "exit $($uninstallProc.ExitCode), install location gone: $goneOk, user data root kept: $dataOk"
    }
    return @{ Passed = $passed; Reason = $reason }
}

function Invoke-SmokeTest {
    if (-not (Test-Path -LiteralPath $SetupExe)) {
        throw "SetupExe not found: $SetupExe"
    }

    # Safety: never touch an install or a port this run did not create.
    if (Test-Path -LiteralPath $script:UninstallKeyPath) {
        throw "The tool is already installed for this user ($($script:UninstallKeyPath) exists). Refusing to run: uninstall it yourself first. This script never uninstalls or overwrites an install it did not create."
    }
    if (Test-PortListening -TestPort $Port) {
        throw "Port $Port is already in use. Pass a different -Port."
    }

    $installLocation = $null
    $launcherProcess = $null
    $profileName = "smoke-test-$(Get-Date -Format 'yyyyMMddHHmmss')-$($PID)"

    try {
        # Check 1: install
        $installArgs = @(
            '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CURRENTUSER',
            "/LOG=`"$($script:InstallLogPath)`""
        )
        $installProc = Start-Process -FilePath $SetupExe -ArgumentList $installArgs -PassThru -Wait
        $installOk = ($installProc.ExitCode -eq 0)
        Write-CheckResult -Name 'install' -Passed $installOk -Reason "setup.exe exited $($installProc.ExitCode)"
        if (-not $installOk) {
            foreach ($skipped in @('install-files', 'launch-health', 'spa-index', 'profile-save', 'stop', 'uninstall')) {
                Write-CheckResult -Name $skipped -Passed $false -Reason 'skipped: install failed'
            }
            return
        }

        # Check 2: install location found from the registry, expected files present
        try {
            $installLocation = (Get-ItemProperty -LiteralPath $script:UninstallKeyPath).InstallLocation
        }
        catch {
            $installLocation = $null
        }
        $filesOk = $false
        $filesReason = "InstallLocation not found under $($script:UninstallKeyPath)"
        if ($installLocation -and (Test-Path -LiteralPath $installLocation)) {
            $expectedFiles = @(
                (Join-Path (Join-Path $installLocation 'bin') 'web_server.exe'),
                (Join-Path (Join-Path $installLocation 'bin') 'reference-outstation.exe'),
                (Join-Path (Join-Path $installLocation 'bin') 'reference-control-station.exe'),
                (Join-Path (Join-Path $installLocation 'frontend') 'index.html'),
                (Join-Path $installLocation 'data'),
                (Join-Path $installLocation 'launch.cmd')
            )
            $missing = @($expectedFiles | Where-Object { -not (Test-Path -LiteralPath $_) })
            $filesOk = ($missing.Count -eq 0)
            if (-not $filesOk) {
                $filesReason = "missing: $($missing -join ', ')"
            }
        }
        Write-CheckResult -Name 'install-files' -Passed $filesOk -Reason $filesReason
        if (-not $filesOk) {
            foreach ($skipped in @('launch-health', 'spa-index', 'profile-save', 'stop')) {
                Write-CheckResult -Name $skipped -Passed $false -Reason 'skipped: install-files failed'
            }
            $uninstallResult = Invoke-BestEffortUninstall -InstallLocation $installLocation
            Write-CheckResult -Name 'uninstall' -Passed $uninstallResult.Passed -Reason $uninstallResult.Reason
            return
        }

        # Check 3: launch via launch.cmd, wait for /api/health
        $env:TESTTOOL_NO_BROWSER = '1'
        $env:PORT = "$Port"
        $launchCmd = Join-Path $installLocation 'launch.cmd'
        $launcherProcess = Start-Process -FilePath 'cmd.exe' -ArgumentList @('/c', "`"$launchCmd`"") -WindowStyle Hidden -PassThru
        $health = Wait-ForHealth -TestPort $Port -MaxSeconds $TimeoutSeconds
        Write-CheckResult -Name 'launch-health' -Passed $health.Ok -Reason $health.Reason
        if (-not $health.Ok) {
            foreach ($skipped in @('spa-index', 'profile-save')) {
                Write-CheckResult -Name $skipped -Passed $false -Reason 'skipped: launch-health failed'
            }
            Stop-ToolProcesses -InstallLocation $installLocation -LauncherProcess $launcherProcess
            Start-Sleep -Milliseconds 500
            $stillRunning = @(Get-ToolProcesses -InstallLocation $installLocation)
            Write-CheckResult -Name 'stop' -Passed ($stillRunning.Count -eq 0) -Reason "$($stillRunning.Count) process(es) remain"
            $uninstallResult = Invoke-BestEffortUninstall -InstallLocation $installLocation
            Write-CheckResult -Name 'uninstall' -Passed $uninstallResult.Passed -Reason $uninstallResult.Reason
            return
        }

        # Check 4: GET / returns the SPA index
        $spaOk = $false
        $spaReason = ''
        try {
            $indexResponse = Invoke-WebRequest -Uri "http://127.0.0.1:$Port/" -UseBasicParsing -TimeoutSec 10
            $spaOk = ($indexResponse.StatusCode -eq 200 -and $indexResponse.Content -match 'id="root"')
            if (-not $spaOk) {
                $spaReason = "status $($indexResponse.StatusCode), root element not found"
            }
        }
        catch {
            $spaReason = $_.Exception.Message
        }
        Write-CheckResult -Name 'spa-index' -Passed $spaOk -Reason $spaReason

        # Check 5: save a profile through the API, check where it landed
        $saveOk = $false
        $saveReason = ''
        try {
            $body = @{ name = $profileName; profile = @{} } | ConvertTo-Json -Compress
            $saveResponse = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/api/profiles" -Method Post -Body $body -ContentType 'application/json' -TimeoutSec 10
            $workingFile = Join-Path (Join-Path (Join-Path $script:DataRoot 'data') 'working') "$($profileName).json"
            $underInstall = Join-Path (Join-Path (Join-Path $installLocation 'data') 'working') "$($profileName).json"
            $savedInUserRoot = Test-Path -LiteralPath $workingFile
            $notUnderInstall = -not (Test-Path -LiteralPath $underInstall)
            $saveOk = ($saveResponse.status -eq 'saved' -and $savedInUserRoot -and $notUnderInstall)
            if (-not $saveOk) {
                $saveReason = "api status '$($saveResponse.status)', saved under user data: $savedInUserRoot, absent under install: $notUnderInstall"
            }
        }
        catch {
            $saveReason = $_.Exception.Message
        }
        Write-CheckResult -Name 'profile-save' -Passed $saveOk -Reason $saveReason

        # Best-effort cleanup of the profile this run created, while the API
        # is still up. Not a scored check: leaving the user data root as
        # found matters more than one small leftover file, so a failure
        # here is only a warning.
        try {
            Invoke-RestMethod -Uri "http://127.0.0.1:$Port/api/profiles/$profileName" -Method Delete -TimeoutSec 10 | Out-Null
        }
        catch {
            Write-Warning "could not delete the test profile '$profileName' through the API: $($_.Exception.Message)"
        }

        # Check 6: stop, no process of the tool remains
        Stop-ToolProcesses -InstallLocation $installLocation -LauncherProcess $launcherProcess
        Start-Sleep -Milliseconds 500
        $stillRunning = @(Get-ToolProcesses -InstallLocation $installLocation)
        Write-CheckResult -Name 'stop' -Passed ($stillRunning.Count -eq 0) -Reason "$($stillRunning.Count) process(es) remain"

        # Check 7: uninstall
        if ($KeepInstalled) {
            Write-CheckResult -Name 'uninstall' -Passed $true -Reason '-KeepInstalled: uninstall skipped by request'
        }
        else {
            $uninstallResult = Invoke-BestEffortUninstall -InstallLocation $installLocation
            Write-CheckResult -Name 'uninstall' -Passed $uninstallResult.Passed -Reason $uninstallResult.Reason
        }
    }
    finally {
        # Safety net for every early return above: nothing this script
        # started may outlive it, whichever check failed.
        Stop-ToolProcesses -InstallLocation $installLocation -LauncherProcess $launcherProcess
        Remove-Item Env:\TESTTOOL_NO_BROWSER -ErrorAction SilentlyContinue
        Remove-Item Env:\PORT -ErrorAction SilentlyContinue
    }
}

function Invoke-Main {
    try {
        Invoke-SmokeTest
    }
    catch {
        Write-Host "FAIL smoke-test: $($_.Exception.Message)"
        $script:FailCount++
    }

    Write-Host "SMOKE TEST: $script:PassCount passed, $script:FailCount failed"
    if ($script:FailCount -gt 0) {
        Write-Host "install log: $script:InstallLogPath"
        Write-Host "tool log directory: $script:LogDir"
        exit 1
    }
    exit 0
}

Invoke-Main

<#
.SYNOPSIS
    Smoke-tests a built IEEE 1815.2 Test Tool Windows installer end to end.
.DESCRIPTION
    Installs the tool per-user and silently, launches it, checks the API
    health route and the served UI, saves a profile through the API and
    checks where it landed, stops every process it started, then uninstalls
    and checks the install is gone while the per-user data survives.
    Needs no admin rights: everything here uses the per-user install mode.
    If the tool is already installed (per-user or machine-wide), it first
    stops that install's processes and runs its own uninstaller, reported as
    the prior-uninstall check. A machine-wide prior install needs an elevated
    PowerShell to remove; without one the check fails and nothing is
    installed. Per-user data under %LOCALAPPDATA% is left alone. Anything
    still present afterwards (a leftover directory or desktop shortcut, with
    no uninstaller) makes the run refuse; remove it by hand.
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
    The installer workflow builds only on v* tags and manual dispatch
    (Actions, Run workflow). Download the artifact from such a run, then:
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
# brace unescaped to a literal one. Inno names the uninstall registry key
# "<AppId>_is1" (INFERRED from Inno convention).
$script:AppId = '{e6851846-fa2b-4b89-93a6-cb0d1dd6d0e6}'
$script:UninstallSubKey = "Software\Microsoft\Windows\CurrentVersion\Uninstall\$($script:AppId)_is1"
$script:UninstallKeyPath = "HKCU:\$($script:UninstallSubKey)"
# DefaultDirName in the .iss; the install-location check pins it.
$script:DefaultInstallDir = 'C:\IEEE-1815_2-Testtool'
$script:ShortcutFileName = 'IEEE 1815.2 Test Tool.lnk'
$script:DataRoot = Join-Path $env:LOCALAPPDATA 'ieee-1815-2-test-tool'
$script:LogDir = Join-Path $script:DataRoot 'logs'
$script:ToolExeNames = @('web_server.exe', 'reference-outstation.exe', 'reference-control-station.exe')
# Set or read by launch.cmd; a caller's value would change what is tested.
$script:LaunchEnvNames = @(
    'DATA_DIR', 'LOG_DIR', 'FRONTEND_DIR', 'HOST', 'OPENAPI_OUTPUT_PATH',
    'REFERENCE_OUTSTATION_BIN', 'REFERENCE_CONTROL_STATION_BIN',
    'PORT', 'TESTTOOL_NO_BROWSER'
)

$script:InstallTimeoutSeconds = 300
$script:UninstallTimeoutSeconds = 120
$script:RemovalWaitSeconds = 15
$script:StopWaitSeconds = 10

$script:PassCount = 0
$script:FailCount = 0
$script:SkipCount = 0
$script:InstallLogPath = Join-Path $env:TEMP "smoke-test-install-$($PID).log"
$script:UninstallLogPath = Join-Path $env:TEMP "smoke-test-uninstall-$($PID).log"

function Write-CheckResult {
    param(
        [string]$Name,
        [ValidateSet('PASS', 'FAIL', 'SKIP')]
        [string]$Status,
        [string]$Reason = ''
    )
    switch ($Status) {
        'PASS' { $script:PassCount++; Write-Host "PASS $Name" }
        'FAIL' { $script:FailCount++; Write-Host "FAIL ${Name}: $Reason" }
        'SKIP' { $script:SkipCount++; Write-Host "SKIP ${Name}: $Reason" }
    }
}

function Write-SkippedChecks {
    param([string[]]$Names, [string]$Reason)
    foreach ($skipped in $Names) {
        Write-CheckResult -Name $skipped -Status 'SKIP' -Reason $Reason
    }
}

function ConvertTo-Status {
    param([bool]$Passed)
    if ($Passed) { return 'PASS' }
    return 'FAIL'
}

function Wait-Condition {
    param([scriptblock]$Condition, [int]$MaxSeconds, [int]$IntervalMilliseconds = 250)
    $deadline = (Get-Date).AddSeconds($MaxSeconds)
    while ($true) {
        if (& $Condition) { return $true }
        if ((Get-Date) -ge $deadline) { return $false }
        Start-Sleep -Milliseconds $IntervalMilliseconds
    }
}

function Invoke-BoundedProcess {
    param([string]$FilePath, [string[]]$ArgumentList, [int]$MaxSeconds)
    $proc = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -PassThru
    # Touching Handle keeps ExitCode readable after WaitForExit on 5.1.
    $null = $proc.Handle
    if (-not $proc.WaitForExit($MaxSeconds * 1000)) {
        try { Stop-Process -Id $proc.Id -Force -ErrorAction Stop }
        catch { Write-Warning "could not stop timed-out process PID $($proc.Id): $($_.Exception.Message)" }
        return @{ TimedOut = $true; ExitCode = $null }
    }
    return @{ TimedOut = $false; ExitCode = $proc.ExitCode }
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

function Test-UninstallKey {
    param(
        [Microsoft.Win32.RegistryHive]$Hive,
        [Microsoft.Win32.RegistryView]$View
    )
    $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey($Hive, $View)
    try {
        $key = $base.OpenSubKey($script:UninstallSubKey)
        if ($key) { $key.Close(); return $true }
        return $false
    }
    finally {
        $base.Close()
    }
}

function Get-ShortcutPaths {
    $paths = @()
    foreach ($folder in @('Desktop', 'CommonDesktopDirectory')) {
        $dir = [Environment]::GetFolderPath($folder)
        if ($dir) { $paths += (Join-Path $dir $script:ShortcutFileName) }
    }
    return $paths
}

function Test-Elevated {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# Registered installs only. HKCU is shared between the two registry views, so
# entries are de-duplicated on hive, uninstall string and location.
function Get-RegisteredInstalls {
    $installs = @()
    $seen = @{}
    foreach ($hive in @([Microsoft.Win32.RegistryHive]::CurrentUser, [Microsoft.Win32.RegistryHive]::LocalMachine)) {
        foreach ($view in @([Microsoft.Win32.RegistryView]::Registry64, [Microsoft.Win32.RegistryView]::Registry32)) {
            $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey($hive, $view)
            try {
                $key = $base.OpenSubKey($script:UninstallSubKey)
                if (-not $key) { continue }
                try {
                    $location = [string]$key.GetValue('InstallLocation')
                    $uninstallString = [string]$key.GetValue('UninstallString')
                }
                finally {
                    $key.Close()
                }
                $id = "$hive|$uninstallString|$location"
                if ($seen.ContainsKey($id)) { continue }
                $seen[$id] = $true
                $installs += [pscustomobject]@{
                    Hive            = $hive
                    View            = $view
                    Label           = "$hive, $view"
                    InstallLocation = $location.TrimEnd('\')
                    UninstallString = $uninstallString
                }
            }
            finally {
                $base.Close()
            }
        }
    }
    return $installs
}

# Inno writes UninstallString as a quoted path to unins000.exe.
function Get-UninstallerPath {
    param([string]$UninstallString)
    $text = $UninstallString.Trim()
    if ($text -match '^"([^"]+)"') { return $Matches[1] }
    if ($text -match '\.exe$') { return $text }
    return $null
}

function Invoke-PriorUninstall {
    param([object[]]$Installs)
    foreach ($install in $Installs) {
        Write-Host "PRIOR INSTALL FOUND ($($install.Label)): location '$($install.InstallLocation)', uninstall command '$($install.UninstallString)'. It will be uninstalled before the smoke test runs."
    }
    if (-not (Test-Elevated)) {
        $machineWide = @($Installs | Where-Object { $_.Hive -eq [Microsoft.Win32.RegistryHive]::LocalMachine })
        if ($machineWide.Count -gt 0) {
            return @{ Passed = $false; Reason = 'a machine-wide (all-users) install is present and removing it needs an elevated PowerShell; rerun as Administrator or remove it yourself' }
        }
    }
    $index = 0
    foreach ($install in $Installs) {
        $index++
        $uninstaller = Get-UninstallerPath -UninstallString $install.UninstallString
        if (-not $uninstaller -or -not (Test-Path -LiteralPath $uninstaller)) {
            return @{ Passed = $false; Reason = "no usable uninstaller for the install in $($install.Label) (uninstall command '$($install.UninstallString)'); remove it by hand" }
        }
        $location = $install.InstallLocation
        if (-not $location) { $location = Split-Path -Parent $uninstaller }
        Stop-ToolProcesses -InstallLocation $location -LauncherProcess $null
        $stopped = Wait-Condition -MaxSeconds $script:StopWaitSeconds -Condition {
            @(Get-ToolProcesses -InstallLocation $location).Count -eq 0
        }
        if (-not $stopped) {
            return @{ Passed = $false; Reason = "tool processes under '$location' are still running after $($script:StopWaitSeconds) s" }
        }
        $logPath = Join-Path $env:TEMP "smoke-test-prior-uninstall-$($PID)-$index.log"
        $uninstallArgs = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=`"$logPath`"")
        $run = Invoke-BoundedProcess -FilePath $uninstaller -ArgumentList $uninstallArgs -MaxSeconds $script:UninstallTimeoutSeconds
        if ($run.TimedOut) {
            return @{ Passed = $false; Reason = "the prior uninstaller did not finish within $($script:UninstallTimeoutSeconds) s (log: $logPath)" }
        }
        # Inno's uninstaller can hand off to a temp copy and return early.
        $gone = Wait-Condition -MaxSeconds $script:RemovalWaitSeconds -Condition {
            (-not (Test-UninstallKey -Hive $install.Hive -View $install.View)) -and
            (-not ($location -and (Test-Path -LiteralPath $location)))
        }
        if (-not $gone) {
            return @{ Passed = $false; Reason = "after the prior uninstaller (exit $($run.ExitCode)) the key or '$location' is still present (log: $logPath)" }
        }
    }
    return @{ Passed = $true; Reason = '' }
}

# Every way a prior install can occupy the target; whatever remains after the
# prior uninstall has no uninstaller and is reported, never deleted.
function Get-PriorInstallEvidence {
    $found = @()
    foreach ($hive in @('CurrentUser', 'LocalMachine')) {
        foreach ($view in @('Registry64', 'Registry32')) {
            if (Test-UninstallKey -Hive $hive -View $view) {
                $found += "uninstall key present ($hive, $view)"
            }
        }
    }
    if (Test-Path -LiteralPath $script:DefaultInstallDir) {
        $found += "install directory exists ($($script:DefaultInstallDir))"
    }
    foreach ($shortcut in (Get-ShortcutPaths)) {
        if (Test-Path -LiteralPath $shortcut) { $found += "desktop shortcut exists ($shortcut)" }
    }
    return $found
}

function Get-InstallLocation {
    try {
        $location = (Get-ItemProperty -LiteralPath $script:UninstallKeyPath).InstallLocation
        if ($location) { return $location.TrimEnd('\') }
    }
    catch {
        return $null
    }
    return $null
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

function Write-StopCheck {
    param([string]$InstallLocation, [System.Diagnostics.Process]$LauncherProcess)
    Stop-ToolProcesses -InstallLocation $InstallLocation -LauncherProcess $LauncherProcess
    $stopped = Wait-Condition -MaxSeconds $script:StopWaitSeconds -Condition {
        @(Get-ToolProcesses -InstallLocation $InstallLocation).Count -eq 0
    }
    $remaining = @(Get-ToolProcesses -InstallLocation $InstallLocation).Count
    Write-CheckResult -Name 'stop' -Status (ConvertTo-Status $stopped) -Reason "$remaining process(es) remain after $($script:StopWaitSeconds) s"
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
    param([string]$InstallLocation, [string]$MarkerPath)
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
    $run = Invoke-BoundedProcess -FilePath $uninstaller -ArgumentList $uninstallArgs -MaxSeconds $script:UninstallTimeoutSeconds
    if ($run.TimedOut) {
        return @{ Passed = $false; Reason = "uninstaller did not finish within $($script:UninstallTimeoutSeconds) s" }
    }
    # Inno's uninstaller can hand off to a temp copy and return before the
    # directory is removed.
    $goneOk = Wait-Condition -MaxSeconds $script:RemovalWaitSeconds -Condition {
        -not (Test-Path -LiteralPath $InstallLocation)
    }
    $shortcutsGone = (@(Get-ShortcutPaths | Where-Object { Test-Path -LiteralPath $_ }).Count -eq 0)
    $exitOk = ($run.ExitCode -eq 0)
    $dataOk = $true
    if ($MarkerPath) { $dataOk = Test-Path -LiteralPath $MarkerPath }
    $passed = ($exitOk -and $goneOk -and $shortcutsGone -and $dataOk)
    $reason = ''
    if (-not $passed) {
        $reason = "exit $($run.ExitCode), install location gone: $goneOk, desktop shortcut gone: $shortcutsGone, this run's marker in the user data root kept: $dataOk"
    }
    return @{ Passed = $passed; Reason = $reason }
}

function Write-UninstallCheck {
    param([hashtable]$State, [string]$InstallLocation)
    if ($KeepInstalled) {
        $State.UninstallHandled = $true
        Write-CheckResult -Name 'uninstall' -Status 'SKIP' -Reason "-KeepInstalled: left in place at $InstallLocation"
        return
    }
    $result = Invoke-BestEffortUninstall -InstallLocation $InstallLocation -MarkerPath $State.MarkerPath
    $State.UninstallHandled = $true
    Write-CheckResult -Name 'uninstall' -Status (ConvertTo-Status $result.Passed) -Reason $result.Reason
}

function Save-LaunchEnvironment {
    $saved = @{}
    foreach ($name in $script:LaunchEnvNames) {
        $saved[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
    }
    return $saved
}

function Restore-LaunchEnvironment {
    param([hashtable]$Saved)
    foreach ($name in $script:LaunchEnvNames) {
        [Environment]::SetEnvironmentVariable($name, $Saved[$name], 'Process')
    }
}

function Invoke-SmokeTest {
    if (-not (Test-Path -LiteralPath $SetupExe)) {
        throw "SetupExe not found: $SetupExe"
    }

    # Check 0: uninstall a registered prior install so the run starts clean.
    $registered = @(Get-RegisteredInstalls)
    if ($registered.Count -eq 0) {
        Write-CheckResult -Name 'prior-uninstall' -Status 'SKIP' -Reason 'nothing was installed'
    }
    else {
        $priorResult = Invoke-PriorUninstall -Installs $registered
        Write-CheckResult -Name 'prior-uninstall' -Status (ConvertTo-Status $priorResult.Passed) -Reason $priorResult.Reason
        if (-not $priorResult.Passed) { return }
    }
    $prior = @(Get-PriorInstallEvidence)
    if ($prior.Count -gt 0) {
        throw "Leftovers of a previous install remain and have no uninstaller: $($prior -join '; '). Refusing to run: remove each by hand, this script never deletes them."
    }
    if (Test-PortListening -TestPort $Port) {
        throw "Port $Port is already in use. Pass a different -Port."
    }

    $installLocation = $null
    $launcherProcess = $null
    $profileName = "smoke-test-$(Get-Date -Format 'yyyyMMddHHmmss')-$($PID)"
    $state = @{ InstallAttempted = $false; UninstallHandled = $false; MarkerPath = $null }
    $savedEnv = Save-LaunchEnvironment

    try {
        # Check 1: install
        $installArgs = @(
            '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CURRENTUSER',
            "/LOG=`"$($script:InstallLogPath)`""
        )
        $state.InstallAttempted = $true
        $installRun = Invoke-BoundedProcess -FilePath $SetupExe -ArgumentList $installArgs -MaxSeconds $script:InstallTimeoutSeconds
        $installOk = (-not $installRun.TimedOut -and $installRun.ExitCode -eq 0)
        $installReason = "setup.exe exited $($installRun.ExitCode)"
        if ($installRun.TimedOut) { $installReason = "setup.exe did not finish within $($script:InstallTimeoutSeconds) s" }
        Write-CheckResult -Name 'install' -Status (ConvertTo-Status $installOk) -Reason $installReason
        if (-not $installOk) {
            Write-SkippedChecks -Names @('install-location', 'install-files', 'desktop-shortcut', 'launch-health', 'spa-index', 'profile-save', 'stop', 'uninstall') -Reason 'install failed'
            return
        }

        # Check 2: install location from the registry is the expected one
        $installLocation = Get-InstallLocation
        $locationOk = ($installLocation -and ($installLocation -ieq $script:DefaultInstallDir.TrimEnd('\')))
        Write-CheckResult -Name 'install-location' -Status (ConvertTo-Status $locationOk) -Reason "InstallLocation is '$installLocation', expected '$($script:DefaultInstallDir)'"

        # Check 3: expected files present
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
        Write-CheckResult -Name 'install-files' -Status (ConvertTo-Status $filesOk) -Reason $filesReason

        # Check 4: a default silent install creates the desktop shortcut
        $shortcutsFound = @(Get-ShortcutPaths | Where-Object { Test-Path -LiteralPath $_ })
        Write-CheckResult -Name 'desktop-shortcut' -Status (ConvertTo-Status ($shortcutsFound.Count -gt 0)) -Reason "'$($script:ShortcutFileName)' not found on the desktop"

        if (-not $filesOk) {
            Write-SkippedChecks -Names @('launch-health', 'spa-index', 'profile-save', 'stop') -Reason 'install-files failed'
            Write-UninstallCheck -State $state -InstallLocation $installLocation
            return
        }

        # Check 5: launch via launch.cmd, wait for /api/health
        foreach ($name in $script:LaunchEnvNames) {
            [Environment]::SetEnvironmentVariable($name, $null, 'Process')
        }
        $env:TESTTOOL_NO_BROWSER = '1'
        $env:PORT = "$Port"
        $launchCmd = Join-Path $installLocation 'launch.cmd'
        $launcherProcess = Start-Process -FilePath 'cmd.exe' -ArgumentList @('/c', "`"$launchCmd`"") -WindowStyle Hidden -PassThru
        $health = Wait-ForHealth -TestPort $Port -MaxSeconds $TimeoutSeconds
        Write-CheckResult -Name 'launch-health' -Status (ConvertTo-Status $health.Ok) -Reason $health.Reason
        if (-not $health.Ok) {
            Write-SkippedChecks -Names @('spa-index', 'profile-save') -Reason 'launch-health failed'
            Write-StopCheck -InstallLocation $installLocation -LauncherProcess $launcherProcess
            Write-UninstallCheck -State $state -InstallLocation $installLocation
            return
        }

        # launch.cmd has created the data root by now. A marker named for
        # this run lets the uninstall check prove the data it checks is
        # this run's, not a folder that predated it.
        $markerPath = Join-Path $script:DataRoot "smoke-marker-$($PID).txt"
        try {
            Set-Content -LiteralPath $markerPath -Value 'smoke-test marker' -Encoding ASCII
            $state.MarkerPath = $markerPath
        }
        catch {
            Write-Warning "could not write the data-root marker: $($_.Exception.Message)"
        }

        # Check 6: GET / returns the SPA index
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
        Write-CheckResult -Name 'spa-index' -Status (ConvertTo-Status $spaOk) -Reason $spaReason

        # Check 7: save a profile through the API, check where it landed
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
        Write-CheckResult -Name 'profile-save' -Status (ConvertTo-Status $saveOk) -Reason $saveReason

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

        # Check 8: stop, no process of the tool remains
        Write-StopCheck -InstallLocation $installLocation -LauncherProcess $launcherProcess

        # Check 9: uninstall
        Write-UninstallCheck -State $state -InstallLocation $installLocation
    }
    finally {
        # Whatever threw, nothing this script started may outlive it: stop
        # the processes, uninstall what this run installed unless
        # -KeepInstalled, and say plainly what is left if that fails.
        if (-not $installLocation) { $installLocation = Get-InstallLocation }
        Stop-ToolProcesses -InstallLocation $installLocation -LauncherProcess $launcherProcess
        if ($state.InstallAttempted -and -not $state.UninstallHandled -and -not $KeepInstalled) {
            try {
                if (-not $installLocation) {
                    Write-Host 'cleanup: no install location found, nothing to uninstall'
                    $rescue = @{ Passed = $true; Reason = '' }
                }
                else {
                    $rescue = Invoke-BestEffortUninstall -InstallLocation $installLocation -MarkerPath $null
                }
                if ($rescue.Passed) {
                    if ($installLocation) { Write-Host 'cleanup: uninstalled the install this run created' }
                }
                else {
                    Write-Host "FAIL cleanup: left behind: install at '$installLocation' and the uninstall key ($($rescue.Reason)). Remove it before the next run. uninstall log: $script:UninstallLogPath"
                    $script:FailCount++
                }
            }
            catch {
                Write-Host "FAIL cleanup: left behind: install at '$installLocation' ($($_.Exception.Message)). Remove it before the next run."
                $script:FailCount++
            }
        }
        if ($state.MarkerPath) {
            Remove-Item -LiteralPath $state.MarkerPath -ErrorAction SilentlyContinue
        }
        Restore-LaunchEnvironment -Saved $savedEnv
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

    Write-Host "SMOKE TEST: $script:PassCount passed, $script:FailCount failed, $script:SkipCount skipped"
    if ($script:FailCount -gt 0) {
        Write-Host "install log: $script:InstallLogPath"
        Write-Host "uninstall log: $script:UninstallLogPath"
        Write-Host "prior-install uninstall logs: $env:TEMP\smoke-test-prior-uninstall-$($PID)-*.log"
        Write-Host "tool log directory: $script:LogDir"
        exit 1
    }
    exit 0
}

Invoke-Main

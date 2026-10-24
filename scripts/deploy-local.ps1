<#
.SYNOPSIS
    Deploy a locally built bsl-analyzer-app.exe over the one the editor extension runs.

.DESCRIPTION
    install.ps1 only ever downloads release builds from a server, so it cannot help
    a developer who just built bsl-analyzer-app.exe locally and wants to try it out
    in place of the installed worker binary. This script does that swap.

    Two Windows facts drive the order of operations here, same as in install.ps1:
    - The installed binary is usually BUSY: the editor extension keeps worker
      processes running on it. Windows refuses to overwrite a busy file ("The
      process cannot access the file ... because it is being used by another
      process") but DOES allow renaming it aside, so the new binary can take its
      place immediately.
    - Stopping the workers first is not enough by itself, because the extension
      respawns them within seconds and a copy started after that loses the race
      against a worker that just reopened the old file. So the order here is:
      rename the old binary out of the way, copy the new one in, and only then
      stop the running workers - they come back on the new file because nothing
      else occupies that path anymore.

    Only processes whose executable path is the install path (or the path the old
    binary was renamed to) are stopped, and only those owned by the current user.
    Other bsl-analyzer-app.exe processes - test builds, other checkouts - are never
    touched or listed.

    Messages are ASCII so the file needs no BOM: Windows PowerShell 5.1 reads a
    BOM-less file as ANSI and would mangle anything else.

    The defaults are the release build in this checkout's target\release and
    ~/.bsl-analyzer/bin/bsl-analyzer-app.exe, the working binary the launcher
    installed (see docs/mcp/SETUP.md). Pass -InstallPath for any other location.

.EXAMPLE
    .\deploy-local.ps1

.EXAMPLE
    .\deploy-local.ps1 -ExePath C:\build\bsl-analyzer-app.exe -Keep 5 -NoRestart

.EXAMPLE
    .\deploy-local.ps1 -WhatIfOnly
#>

[CmdletBinding()]
param(
    [string] $ExePath     = (Join-Path $PSScriptRoot '..\target\release\bsl-analyzer-app.exe'),
    [string] $InstallPath = (Join-Path $env:USERPROFILE '.bsl-analyzer\bin\bsl-analyzer-app.exe'),
    [int]    $Keep        = 3,
    [switch] $NoRestart,
    [switch] $WhatIfOnly
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Info { param([string] $Message) Write-Host "[INFO] $Message" -ForegroundColor Blue }
function Write-Ok { param([string] $Message) Write-Host "[OK] $Message" -ForegroundColor Green }
function Write-Warn { param([string] $Message) Write-Host "[WARN] $Message" -ForegroundColor Yellow }

function Get-BinaryVersion {
    param([string] $Path)

    # A binary can be built with a broken --version, or none at all; that must not
    # abort the deploy, it only means the retired copy gets a less specific name.
    try {
        $output = & $Path --version 2>$null
    } catch {
        return 'unknown'
    }

    $joined = (($output | Out-String)).Trim()
    if (-not $joined) { return 'unknown' }

    # The binary prints "bsl-analyzer 0.2.80" - product name and version
    # separated by whitespace. Keep only the last token, so the version does not
    # get glued to the product name; a single-token output (just the version)
    # passes through unchanged.
    $tokens = $joined -split '\s+'
    $versionToken = $tokens[-1]

    # Keep only characters safe in a Windows file name; anything else (spaces,
    # parentheses, colons that --version output likes to include) is dropped so
    # the result can be used verbatim in Rename-Item below.
    $safe = ($versionToken -replace '[^A-Za-z0-9.\-]', '')
    if (-not $safe) { return 'unknown' }
    return $safe
}

function Get-NormalizedPath {
    param([string] $Path)

    try {
        return [System.IO.Path]::GetFullPath($Path).TrimEnd('\', '/')
    } catch {
        return $null
    }
}

function Get-MatchingWorkers {
    param([string] $ProcessName, [string[]] $MatchPaths)

    # Only a process whose executable IS one of $MatchPaths (full normalized paths,
    # compared case-insensitively) is a worker of this install. Processes of the same
    # image running from anywhere else - test builds, other checkouts - are not
    # returned at all, so they can be neither stopped nor even listed.
    $wanted = @($MatchPaths | ForEach-Object { Get-NormalizedPath -Path $_ } | Where-Object { $_ })
    $found = @()
    $processes = Get-CimInstance Win32_Process -Filter "Name='$ProcessName'" -ErrorAction SilentlyContinue
    foreach ($proc in $processes) {
        if (-not $proc.ExecutablePath) { continue }
        $procPath = Get-NormalizedPath -Path $proc.ExecutablePath
        if (-not $procPath -or -not ($wanted | Where-Object { $_ -ieq $procPath })) { continue }

        # Get-CimInstance objects have no GetOwner() method - that only exists on the
        # obsolete Get-WmiObject. The CIM equivalent is invoked explicitly like this.
        $owner = $null
        try { $owner = (Invoke-CimMethod -InputObject $proc -MethodName GetOwner).User } catch { $owner = $null }
        $found += [pscustomobject]@{ Pid = $proc.ProcessId; Owner = $owner }
    }
    return @($found)
}

function Stop-OwnWorkers {
    param([string] $ProcessName, [string[]] $MatchPaths)

    $stoppedIds = @()
    $failedIds = @()
    $skippedOthers = @()

    foreach ($worker in @(Get-MatchingWorkers -ProcessName $ProcessName -MatchPaths $MatchPaths)) {
        # This is a multi-user machine: only processes owned by the current user may
        # be touched. Anyone else's worker is left running and reported, never stopped.
        if ($worker.Owner -ne $env:USERNAME) {
            $skippedOthers += [pscustomobject]@{ Pid = $worker.Pid; Owner = $worker.Owner }
            continue
        }

        try {
            Stop-Process -Id $worker.Pid -Force -ErrorAction Stop
            $stoppedIds += $worker.Pid
        } catch {
            $failedIds += [pscustomobject]@{ Pid = $worker.Pid; Error = $_.Exception.Message }
        }
    }

    return [pscustomobject]@{
        Stopped = $stoppedIds
        Failed  = $failedIds
        Skipped = $skippedOthers
    }
}

function Get-RetiredCopiesToDelete {
    param(
        [string]     $InstallDir,
        [string]     $LiveFullPath,
        [int]        $Keep,
        [psobject[]] $ExtraCandidates = @()
    )

    # Single source of truth for which retired copies are stale, used by both the
    # -WhatIfOnly preview and the real prune below, so the preview cannot drift
    # from what actually gets deleted.
    $existing = Get-ChildItem -Path $InstallDir -Filter 'bsl-analyzer-app.*.exe' -File -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -ne $LiveFullPath }

    $candidates = @($existing) + @($ExtraCandidates)
    $sorted = $candidates | Sort-Object LastWriteTime -Descending

    # Both returns are wrapped in @(...): a pipeline that yields zero items collapses
    # to $null and a pipeline that yields exactly one item collapses to a bare
    # object, never a one-element array; under Set-StrictMode both callers' .Count
    # would then throw. @(...) forces an array in every case (0, 1 or many items).
    if ($Keep -le 0) { return @($sorted) }
    return @($sorted | Select-Object -Skip $Keep)
}

function Invoke-Deploy {
    if (-not (Test-Path -LiteralPath $ExePath)) {
        throw "Source binary not found: $ExePath - build it first with cargo build --release"
    }

    $newHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $ExePath).Hash

    $oldVersion = 'none'
    $installedExists = Test-Path -LiteralPath $InstallPath
    if ($installedExists) {
        $installedHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $InstallPath).Hash
        if ($installedHash -ieq $newHash) {
            Write-Ok "Already deployed ($($newHash.Substring(0, 12)))"
            return
        }

        # The retired copy IS the old binary, so it is named after the old binary's
        # own reported version, not the new one that is about to replace it.
        $oldVersion = Get-BinaryVersion -Path $InstallPath
    }

    $newVersion = Get-BinaryVersion -Path $ExePath

    $installDir = Split-Path -Parent $InstallPath
    $baseName = [System.IO.Path]::GetFileNameWithoutExtension((Split-Path -Leaf $InstallPath))
    $retiredName = "$baseName.$oldVersion.exe"
    $retiredPath = Join-Path $installDir $retiredName
    if ($installedExists -and (Test-Path -LiteralPath $retiredPath)) {
        $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
        $retiredName = "$baseName.$oldVersion-$stamp.exe"
        $retiredPath = Join-Path $installDir $retiredName
    }

    if ($WhatIfOnly) {
        Write-Info "Would deploy $newVersion ($($newHash.Substring(0, 12))) over $oldVersion"
        if ($installedExists) {
            Write-Info "Would retire installed binary as: $retiredName"
        } else {
            Write-Info "No binary currently installed at $InstallPath, would install fresh"
        }
        Write-Info "Would copy: $ExePath -> $InstallPath"
        if ($NoRestart) {
            Write-Info 'Would not stop any worker processes (-NoRestart)'
        } else {
            # After the rename a running worker's image path is the retired path, so both
            # names identify a worker of this install.
            $candidates = @(Get-MatchingWorkers -ProcessName 'bsl-analyzer-app.exe' -MatchPaths @($InstallPath, $retiredPath))
            if ($candidates.Count -gt 0) {
                foreach ($worker in $candidates) {
                    if ($worker.Owner -eq $env:USERNAME) {
                        Write-Info "Would stop pid $($worker.Pid) (owned by $($worker.Owner))"
                    } else {
                        Write-Info "Would skip pid $($worker.Pid) (owned by $($worker.Owner))"
                    }
                }
            } else {
                Write-Info 'No running bsl-analyzer-app.exe processes found at the install path'
            }
        }

        # The prune step is the only destructive one (it deletes files permanently),
        # so the preview must cover it too. At this point in a real run the old
        # binary has already been renamed to $retiredName and sits on disk; here it
        # does not exist yet, so it is added as a synthetic candidate (same size and
        # LastWriteTime as the currently installed file, since a rename changes
        # neither) so the preview counts the same file the real run would delete.
        $previewExtra = @()
        $previewLivePath = $InstallPath
        if ($installedExists) {
            $installedItem = Get-Item -LiteralPath $InstallPath
            $previewExtra = @([pscustomobject]@{
                Name          = $retiredName
                FullName      = $retiredPath
                Length        = $installedItem.Length
                LastWriteTime = $installedItem.LastWriteTime
            })
            $previewLivePath = (Resolve-Path -LiteralPath $InstallPath).ProviderPath
        }
        # @(...) here too: even though the helper now always returns an array, an
        # assignment through a bare pipeline call can still unwrap a one-element
        # array back to a scalar; wrapping the call site keeps .Count safe either way.
        $previewToDelete = @(Get-RetiredCopiesToDelete -InstallDir $installDir -LiveFullPath $previewLivePath -Keep $Keep -ExtraCandidates $previewExtra)

        if ($previewToDelete.Count -gt 0) {
            $previewFreedBytes = 0
            foreach ($file in $previewToDelete) {
                $previewFreedBytes += $file.Length
                Write-Info "Would delete retired copy: $($file.Name) ($([math]::Round($file.Length / 1MB, 2)) MiB, $($file.LastWriteTime))"
            }
            Write-Info "Would prune $($previewToDelete.Count) retired copies, freeing $([math]::Round($previewFreedBytes / 1MB, 2)) MiB"
        } else {
            Write-Info 'Would prune no retired copies'
        }

        return
    }

    New-Item -ItemType Directory -Force -Path $installDir | Out-Null

    # Rename first: this is the step that survives the installed binary being busy.
    # See the two Windows facts in the file header for why this has to come before
    # the copy and before any process is stopped.
    if ($installedExists) {
        try {
            Rename-Item -LiteralPath $InstallPath -NewName $retiredName -Force
        } catch {
            throw "Cannot retire $InstallPath - stop running bsl-analyzer-app.exe processes and retry: $($_.Exception.Message)"
        }
        Write-Info "Retired installed binary as: $retiredName"
    }

    try {
        Copy-Item -LiteralPath $ExePath -Destination $InstallPath -Force
    } catch {
        $failure = $_
        if ($installedExists) {
            try {
                Rename-Item -LiteralPath $retiredPath -NewName (Split-Path -Leaf $InstallPath) -Force
                Write-Warn 'Copy failed, the previous binary was restored'
            } catch {
                Write-Warn "Copy failed and the previous binary could not be restored, it is left as $retiredPath"
            }
        }
        throw $failure
    }

    $verifyHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $InstallPath).Hash
    if ($verifyHash -ine $newHash) {
        if ($installedExists) {
            try {
                Remove-Item -LiteralPath $InstallPath -Force -ErrorAction SilentlyContinue
                Rename-Item -LiteralPath $retiredPath -NewName (Split-Path -Leaf $InstallPath) -Force
                Write-Warn 'Verification failed, the previous binary was restored'
            } catch {
                Write-Warn "Verification failed and the previous binary could not be restored, it is left as $retiredPath"
            }
        }
        throw "Copied file hash does not match source hash: expected $newHash, got $verifyHash"
    }

    Write-Ok "Deployed $newVersion ($($newHash.Substring(0, 12))) to $InstallPath"

    $stopResult = [pscustomobject]@{ Stopped = @(); Failed = @(); Skipped = @() }
    if ($NoRestart) {
        Write-Info 'Not stopping worker processes (-NoRestart)'
    } else {
        # Only now, after the new file is verified in place, are the old workers
        # stopped - so whatever the extension respawns next comes back on the new
        # binary instead of racing the copy above.
        $stopResult = Stop-OwnWorkers -ProcessName 'bsl-analyzer-app.exe' -MatchPaths @($InstallPath, $retiredPath)
        foreach ($stoppedId in $stopResult.Stopped) {
            Write-Ok "Stopped worker pid $stoppedId"
        }
        foreach ($entry in $stopResult.Failed) {
            Write-Warn "Could not stop worker pid $($entry.Pid): $($entry.Error)"
        }
        foreach ($entry in $stopResult.Skipped) {
            Write-Warn "Skipped worker pid $($entry.Pid), owned by another user ($($entry.Owner))"
        }
    }

    # Prune retired copies, keeping the $Keep most recently written ones. Uses the
    # same selection helper as the -WhatIfOnly preview above, so the two cannot
    # disagree.
    $liveFullPath = (Resolve-Path -LiteralPath $InstallPath).ProviderPath
    # @(...) here too: same reasoning as the -WhatIfOnly preview call site above -
    # this is the real prune, so an unwrapped single retired copy would throw on
    # .Count below after the binary has already been copied into place.
    $toDelete = @(Get-RetiredCopiesToDelete -InstallDir $installDir -LiveFullPath $liveFullPath -Keep $Keep)
    $freedBytes = 0
    $deletedCount = 0
    foreach ($file in $toDelete) {
        try {
            $freedBytes += $file.Length
            Remove-Item -LiteralPath $file.FullName -Force
            $deletedCount++
        } catch {
            Write-Warn "Could not delete retired copy $($file.Name): $($_.Exception.Message)"
        }
    }
    if ($deletedCount -gt 0) {
        Write-Info "Pruned $deletedCount retired copies, freed $([math]::Round($freedBytes / 1MB, 2)) MiB"
    }

    Write-Host ''
    Write-Host "  $oldVersion -> $newVersion"
    Write-Host "  Installed hash: $($newHash.Substring(0, 12))"
    Write-Host "  Retired as: $retiredName"
    $stoppedText = if ($stopResult.Stopped.Count -gt 0) { $stopResult.Stopped -join ', ' } else { 'none' }
    Write-Host "  Workers stopped: $stoppedText"
    Write-Host "  Retired copies pruned: $deletedCount"
    Write-Host ''
}

Invoke-Deploy

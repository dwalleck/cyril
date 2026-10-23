param(
  [Parameter(Mandatory=$true)][string]$SourceRoot,
  [Parameter(Mandatory=$true)][string]$Nextest,
  [Parameter(Mandatory=$true)][string]$RecordDirectory,
  [switch]$QualifiedProbeGo,
  [ValidateRange(30,600)][int]$DeadlineSeconds = 120
)
$ErrorActionPreference = 'Stop'
if (-not $QualifiedProbeGo)
{ throw 'Separate parent authorization is required before this native run' 
}
$source = (Resolve-Path -LiteralPath $SourceRoot).Path.TrimEnd('\')
$nextestPath = (Resolve-Path -LiteralPath $Nextest).Path
if (Test-Path -LiteralPath $RecordDirectory)
{ throw 'RecordDirectory must be a fresh owned directory' 
}
$records = (New-Item -ItemType Directory -Path $RecordDirectory).FullName
$phases = (New-Item -ItemType Directory -Path (Join-Path $records 'phases')).FullName
$pins = Get-Content -Raw -LiteralPath (Join-Path $source '.cyril-u62f\source-pins.json') | ConvertFrom-Json
$baseline = 'd6f7937af9200a93b9d8997bf856b8fd80c7204e'
if ($pins.export_baseline -cne $baseline)
{ throw 'Source export is not the pinned failing checkout' 
}
foreach ($file in $pins.files)
{
  $actual = (Get-FileHash -LiteralPath (Join-Path $source $file.path) -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($actual -cne $file.sha256)
  { throw ('Instrumented source pin mismatch: ' + $file.path) 
  }
}
# Verify the immutable failing lockfile, not the different 4ef worktree lockfile.
$lockBytes = [IO.File]::ReadAllBytes((Join-Path $source 'Cargo.lock'))
$header = [Text.Encoding]::ASCII.GetBytes(('blob ' + $lockBytes.Length + [char]0))
$sha = [Security.Cryptography.SHA1]::Create()
try
{
  [void]$sha.TransformBlock($header, 0, $header.Length, $header, 0)
  [void]$sha.TransformFinalBlock($lockBytes, 0, $lockBytes.Length)
  $lockBlob = ([BitConverter]::ToString($sha.Hash)).Replace('-','').ToLowerInvariant()
} finally
{ $sha.Dispose() 
}
if ($lockBlob -cne $pins.export_lock_git_blob)
{ throw 'Cargo.lock does not match the immutable failing checkout' 
}
$expectedNextest = 'd9dd12a3d5dd67d5b8ca9315fa9f3c2384d1d47f4e324039ba634af36e2862d8'
if ((Get-FileHash -LiteralPath $nextestPath -Algorithm SHA256).Hash.ToLowerInvariant() -cne $expectedNextest)
{ throw 'Nextest executable pin mismatch' 
}
# WinRM starts a fresh process: never rely on a previous preflight/build shell's
# MSVC environment surviving into this invocation. Import existing tools only.
$vcvars = 'C:\BuildTools\VC\Auxiliary\Build\vcvars64.bat'
if (-not (Test-Path -LiteralPath $vcvars -PathType Leaf))
{
  throw 'The preflighted MSVC environment script is unavailable; do not install or substitute tools'
}
$msvcEnvironment = & $env:ComSpec /d /c "`"$vcvars`" >nul 2>&1 && set"
if ($LASTEXITCODE -ne 0)
{
  throw ('Existing MSVC environment import failed: ' + $LASTEXITCODE)
}
foreach ($line in $msvcEnvironment)
{
  if ($line -match '^([^=]+)=(.*)$')
  {
    Set-Item -Path ("env:" + $matches[1]) -Value $matches[2]
  }
}
$msvcLink = (Get-Command link.exe -ErrorAction Stop).Source
$msvcCompiler = (Get-Command cl.exe -ErrorAction Stop).Source
$env:Path = (Split-Path -Parent $nextestPath) + ";$env:USERPROFILE\.cargo\bin;$env:Path"
$env:CARGO_TERM_COLOR = 'always'
$env:RUST_BACKTRACE = '1'
$env:CARGO_INCREMENTAL = '0'
$env:CACHE_ON_FAILURE = 'false'
$env:CYRIL_MEMORY_PHASE_PROBE = '1'
$env:CYRIL_MEMORY_PHASE_DIRECTORY = $phases
$env:CYRIL_MEMORY_PROBE_SOURCE = $baseline
# The parent creates this fresh target directory during the separately authorized
# preparation build. Do not reuse global/shared artifacts or stage over old sources.
$target = Join-Path $source 'target-u62f-probe'
$env:CARGO_TARGET_DIR = $target
if (-not (Test-Path -LiteralPath $target -PathType Container))
{ throw 'Parent must complete the pinned preparation build first' 
}
if (Test-Path Env:NEXTEST_TEST_THREADS)
{ throw 'Remove inherited test-thread override before the default-scheduling probe' 
}
if (Test-Path Env:NEXTEST_PROFILE)
{ throw 'Remove inherited profile override before the default-profile probe' 
}
$stdout = Join-Path $records 'nextest.stdout.log'
$stderr = Join-Path $records 'nextest.stderr.log'
$observations = Join-Path $records 'process-observations.jsonl'
$script:observationCount = 0
function Record($value)
{
  if ($script:observationCount -ge 4096)
  { throw 'Diagnostic observation cap reached' 
  }
  $script:observationCount++
  $value | ConvertTo-Json -Compress -Depth 6 | Add-Content -LiteralPath $observations -Encoding UTF8
}
function Save-ObserverError($failure, $kind, $fileName)
{
  [ordered]@{
    kind=$kind
    observedUTC=[DateTime]::UtcNow.ToString('o')
    elapsedMs=$watch.ElapsedMilliseconds
    exceptionType=$failure.Exception.GetType().FullName
    message=$failure.Exception.Message
    fullyQualifiedErrorId=$failure.FullyQualifiedErrorId
    scriptPath=$failure.InvocationInfo.ScriptName
    scriptLine=$failure.InvocationInfo.ScriptLineNumber
    offsetInLine=$failure.InvocationInfo.OffsetInLine
    positionMessage=$failure.InvocationInfo.PositionMessage
    scriptStackTrace=$failure.ScriptStackTrace
  } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $records $fileName) -Encoding UTF8
}
Record ([ordered]@{ kind='process-local-msvc'; vcvars=$vcvars; linker=$msvcLink; compiler=$msvcCompiler; globalPATHChanged=$false })
$os = Get-CimInstance Win32_OperatingSystem
$system = Get-CimInstance Win32_ComputerSystem
Record ([ordered]@{ kind='probe-start'; utc=[DateTime]::UtcNow.ToString('o'); baseline=$baseline; instrumentationBase=$pins.instrumentation_edit_base; source=$source; target=$target; os=$os.Caption; osBuild=$os.BuildNumber; logicalCPU=$system.NumberOfLogicalProcessors; RAMBytes=$system.TotalPhysicalMemory; deadlineSeconds=$DeadlineSeconds; args='nextest run -p cyril --success-output immediate --failure-output immediate'; scheduling='default; no test-thread, no-capture, or fail-fast override'; phases=$phases })
$owned = @{}
$paths = @{}
$driver = $null
$driverExitCode = $null
$timedOut = $false
$watch = [Diagnostics.Stopwatch]::StartNew()
try
{
  # Direct invocation of the verified Cargo subcommand executable is equivalent
  # to `cargo nextest run -p cyril`; only output-retention flags are added.
  $driver = Start-Process -FilePath $nextestPath -WorkingDirectory $source -ArgumentList @('nextest','run','-p','cyril','--success-output','immediate','--failure-output','immediate') -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru
  [void]$driver.Handle
  Record ([ordered]@{ kind='driver-start'; pid=$driver.Id; executable=$nextestPath; utc=[DateTime]::UtcNow.ToString('o') })
  $finishedAt = $null
  while ($true)
  {
    foreach ($process in @(Get-Process -Name 'cyril-memory-runtime','memory_runtime-*' -ErrorAction SilentlyContinue))
    {
      try
      {
        $path = $process.Path
        if (-not $path.StartsWith(($target + '\'), [StringComparison]::OrdinalIgnoreCase))
        { continue 
        }
        $key = [string]$process.Id + ':' + $process.StartTime.ToUniversalTime().Ticks
        if (-not $owned.ContainsKey($key))
        {
          [void]$process.Handle
          $owned[$key] = @{ process=$process; exited=$false; path=$path }
          $paths[$path] = $true
          Record ([ordered]@{ kind='owned-process-observed'; pid=$process.Id; executable=$path; startUTC=$process.StartTime.ToUniversalTime().ToString('o'); observedUTC=[DateTime]::UtcNow.ToString('o') })
        }
      } catch
      { Record ([ordered]@{ kind='observation-error'; pid=$process.Id; error=$_.Exception.GetType().FullName; message=$_.Exception.Message }) 
      }
    }
    foreach ($entry in $owned.Values)
    {
      if (-not $entry.exited -and $entry.process.HasExited)
      {
        $entry.exited = $true
        Record ([ordered]@{ kind='owned-process-exit'; pid=$entry.process.Id; executable=$entry.path; hasExited=$true; observedUTC=[DateTime]::UtcNow.ToString('o'); exitUTC=$null; exitCode=$null; exitTimeObservation='unavailable-not-sampled'; exitCodeObservation='unavailable-not-sampled' })
      }
    }
    if ($driver.HasExited)
    {
      if ($null -eq $finishedAt)
      {
        $driverExitCode = $driver.ExitCode
        if ($null -eq $driverExitCode)
        { throw 'Required nextest driver exit status is unavailable; no outcome may be inferred'
        }
        $finishedAt = $watch.Elapsed.TotalSeconds
        Record ([ordered]@{ kind='driver-exit'; pid=$driver.Id; hasExited=$true; observedUTC=[DateTime]::UtcNow.ToString('o'); exitUTC=$null; exitTimeObservation='unavailable-not-sampled'; exitCode=$driverExitCode; elapsedMs=$watch.ElapsedMilliseconds })
      }
      if (@($owned.Values | Where-Object { -not $_.exited }).Count -eq 0 -or ($watch.Elapsed.TotalSeconds - $finishedAt) -ge 5)
      { break 
      }
    }
    if ($watch.Elapsed.TotalSeconds -ge $DeadlineSeconds)
    {
      $timedOut = $true
      Record ([ordered]@{ kind='outer-diagnostic-deadline'; elapsedMs=$watch.ElapsedMilliseconds; note='Not the fixture readiness or shutdown budget' })
      break
    }
    Start-Sleep -Milliseconds 50
  }
} catch
{
  # Save the original ErrorRecord projection before cleanup can throw another error.
  $primaryError = $_
  Save-ObserverError $primaryError 'observer-primary-error' 'observer-primary-error.json'
  throw
} finally
{
  try
  {
    # This is explicitly post-observation cleanup, never evidence of test teardown.
    if ($null -ne $driver -and -not $driver.HasExited)
    {
      Record ([ordered]@{ kind='observer-owned-driver-cleanup'; pid=$driver.Id })
      $driver.Kill()
      if (-not $driver.WaitForExit(5000))
      { Record ([ordered]@{ kind='observer-driver-reap-unobserved'; pid=$driver.Id })
      }
    }
    foreach ($entry in $owned.Values)
    {
      if (-not $entry.process.HasExited)
      {
        Record ([ordered]@{ kind='observer-owned-lingering-cleanup'; pid=$entry.process.Id; executable=$entry.path; note='Alive after captured test/driver boundary; cleanup is not a test success' })
        $entry.process.Kill()
        $reaped = $entry.process.WaitForExit(5000)
        Record ([ordered]@{ kind='observer-cleanup-reap'; pid=$entry.process.Id; observedExit=$reaped })
      }
    }
    foreach ($path in $paths.Keys)
    { Record ([ordered]@{ kind='observed-executable-pin'; executable=$path; sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() })
    }
  } catch
  {
    Save-ObserverError $_ 'observer-cleanup-error' 'observer-cleanup-error.json'
    throw
  }
}
[ordered]@{ kind='probe-receipt'; timedOut=$timedOut; driverExitCode=$driverExitCode; driverExitCodeObserved=($null -ne $driverExitCode); elapsedMs=$watch.ElapsedMilliseconds; records=$records; phases=$phases; stdout=$stdout; stderr=$stderr; observations=$observations } | ConvertTo-Json -Compress
if ($timedOut)
{ throw 'Bounded diagnostic run reached its outer deadline; inspect live per-process phase logs'
}
if ($null -eq $driverExitCode)
{ throw 'Required nextest driver exit status is unavailable; no outcome may be inferred'
}
if ($driverExitCode -ne 0)
{ throw ('Instrumented nextest lane failed: ' + $driverExitCode)
}

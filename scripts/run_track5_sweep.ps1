$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Binary = Join-Path $RepoRoot 'target\release\vortex-alfa.exe'
if (-not (Test-Path $Binary)) {
    throw "Release binary not found at $Binary. Run 'cargo build --release' first."
}

$DirMain = Join-Path $RepoRoot 'data_local\bybit_macro5y_1d'
$DirR1   = Join-Path $RepoRoot 'data_local\bybit_macro5y_r1'
$DirR2   = Join-Path $RepoRoot 'data_local\bybit_macro5y_r2'
foreach ($dir in @($DirMain, $DirR1, $DirR2)) {
    if (-not (Test-Path $dir)) {
        throw "Dataset directory not found: $dir"
    }
    foreach ($sub in @('linear', 'spot')) {
        $subPath = Join-Path $dir $sub
        if (-not (Test-Path $subPath)) {
            throw "Dataset subdirectory not found: $subPath"
        }
    }
}

$ReportsDir = Join-Path $RepoRoot 'data_local\reports_track5'
New-Item -ItemType Directory -Force -Path $ReportsDir | Out-Null

function Invoke-ScanMacro {
    param(
        [string]$Label,
        [string]$DatasetDir,
        [string]$SlippageBps,
        [string]$OutPath
    )
    $linearDir = Join-Path $DatasetDir 'linear'
    $spotDir   = Join-Path $DatasetDir 'spot'
    Write-Host "Running [$Label] slip=$SlippageBps -> $OutPath"
    $output = & $Binary scan-macro --linear-dir $linearDir --spot-dir $spotDir --slippage-bps $SlippageBps --out $OutPath 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Error "scan-macro failed for [$Label] with exit code $LASTEXITCODE`: $output"
    }
    return $output
}

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$runCount = 0

# 1. Main 5-year full-sample sweep (1.5 bps binding slippage) - print full ASCII table
$mainOut = Join-Path $ReportsDir 'main_5y_slip1p5.json'
$ascii = Invoke-ScanMacro -Label 'MAIN 5y slip=1.5bps' -DatasetDir $DirMain -SlippageBps '1.5' -OutPath $mainOut
$ascii | Out-Host
$runCount++

# 2-5. OOS Regime 1 / Regime 2 and slippage sensitivity sweeps
$otherSpecs = @(
    @{ Label = 'OOS R1 slip=1.5bps';  Dir = $DirR1;   Slip = '1.5'; File = 'oos_r1_slip1p5.json' },
    @{ Label = 'OOS R2 slip=1.5bps';  Dir = $DirR2;   Slip = '1.5'; File = 'oos_r2_slip1p5.json' },
    @{ Label = 'SENS 5y slip=0.5bps'; Dir = $DirMain; Slip = '0.5'; File = 'sens_5y_slip0p5.json' },
    @{ Label = 'SENS 5y slip=3.0bps'; Dir = $DirMain; Slip = '3.0'; File = 'sens_5y_slip3p0.json' }
)
foreach ($spec in $otherSpecs) {
    $outJson = Join-Path $ReportsDir $spec.File
    $null = Invoke-ScanMacro -Label $spec.Label -DatasetDir $spec.Dir -SlippageBps $spec.Slip -OutPath $outJson
    $runCount++
}

$sw.Stop()
Write-Host ("Completed {0} scan-macro runs in {1:N2}s." -f $runCount, $sw.Elapsed.TotalSeconds)

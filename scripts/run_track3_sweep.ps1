$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Binary = Join-Path $RepoRoot 'target\release\vortex-alfa.exe'
if (-not (Test-Path $Binary)) {
    throw "Release binary not found at $Binary. Run 'cargo build --release' first."
}

$DirMain = Join-Path $RepoRoot 'data_local\bybit_xs30_4h'
$DirY1   = Join-Path $RepoRoot 'data_local\bybit_xs30_4h_y1'
$DirY2   = Join-Path $RepoRoot 'data_local\bybit_xs30_4h_y2'
foreach ($dir in @($DirMain, $DirY1, $DirY2)) {
    if (-not (Test-Path $dir)) {
        throw "Dataset directory not found: $dir"
    }
}

$ReportsDir = Join-Path $RepoRoot 'data_local\reports_track3'
New-Item -ItemType Directory -Force -Path $ReportsDir | Out-Null

function Invoke-ScanXs {
    param(
        [string]$Label,
        [string]$InputPath,
        [string]$TopK,
        [string]$SlippageBps,
        [string]$OutPath
    )
    Write-Host "Running [$Label] top-k=$TopK slip=$SlippageBps -> $OutPath"
    $output = & $Binary scan-xs --format csv --input $InputPath --top-k $TopK --slippage-bps $SlippageBps --out $OutPath 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Error "scan-xs failed for [$Label] with exit code $LASTEXITCODE`: $output"
    }
    return $output
}

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$runCount = 0

# 1. Main 24m sweep (30 pairs, 4h, binding slippage 1.5 bps)
$mainSpecs = @(
    @{ TopK = '1';     Tag = 'k1';       Label = 'MAIN 24m k=1' },
    @{ TopK = '2';     Tag = 'k2';       Label = 'MAIN 24m k=2' },
    @{ TopK = '3';     Tag = 'k3';       Label = 'MAIN 24m k=3' },
    @{ TopK = '1,2,3'; Tag = 'global27'; Label = 'MAIN 24m global M=27' }
)
foreach ($spec in $mainSpecs) {
    $outJson = Join-Path $ReportsDir ("main_24m_{0}_slip1p5.json" -f $spec.Tag)
    $ascii = Invoke-ScanXs -Label $spec.Label -InputPath $DirMain -TopK $spec.TopK -SlippageBps '1.5' -OutPath $outJson
    $ascii | Out-Host
    $runCount++
}

# 2. OOS Year 1 (30 pairs, 4h, slippage 1.5 bps)
$oosY1Specs = @(
    @{ TopK = '1';     Tag = 'k1';       Label = 'OOS Y1 k=1' },
    @{ TopK = '2';     Tag = 'k2';       Label = 'OOS Y1 k=2' },
    @{ TopK = '3';     Tag = 'k3';       Label = 'OOS Y1 k=3' },
    @{ TopK = '1,2,3'; Tag = 'global27'; Label = 'OOS Y1 global M=27' }
)
foreach ($spec in $oosY1Specs) {
    $outJson = Join-Path $ReportsDir ("oos_y1_{0}_slip1p5.json" -f $spec.Tag)
    $null = Invoke-ScanXs -Label $spec.Label -InputPath $DirY1 -TopK $spec.TopK -SlippageBps '1.5' -OutPath $outJson
    $runCount++
}

# 3. OOS Year 2 (30 pairs, 4h, slippage 1.5 bps)
$oosY2Specs = @(
    @{ TopK = '1';     Tag = 'k1';       Label = 'OOS Y2 k=1' },
    @{ TopK = '2';     Tag = 'k2';       Label = 'OOS Y2 k=2' },
    @{ TopK = '3';     Tag = 'k3';       Label = 'OOS Y2 k=3' },
    @{ TopK = '1,2,3'; Tag = 'global27'; Label = 'OOS Y2 global M=27' }
)
foreach ($spec in $oosY2Specs) {
    $outJson = Join-Path $ReportsDir ("oos_y2_{0}_slip1p5.json" -f $spec.Tag)
    $null = Invoke-ScanXs -Label $spec.Label -InputPath $DirY2 -TopK $spec.TopK -SlippageBps '1.5' -OutPath $outJson
    $runCount++
}

# 4. Secondary slippage sensitivity (0.5 and 3.0 bps on full 24m, 30 pairs)
$sensSpecs = @(
    @{ TopK = '3';     Tag = 'k3';       Slip = '0.5'; SlipTag = '0p5'; Label = 'SENS 0.5bps k=3' },
    @{ TopK = '3';     Tag = 'k3';       Slip = '3.0'; SlipTag = '3p0'; Label = 'SENS 3.0bps k=3' },
    @{ TopK = '2';     Tag = 'k2';       Slip = '0.5'; SlipTag = '0p5'; Label = 'SENS 0.5bps k=2' },
    @{ TopK = '2';     Tag = 'k2';       Slip = '3.0'; SlipTag = '3p0'; Label = 'SENS 3.0bps k=2' },
    @{ TopK = '1,2,3'; Tag = 'global27'; Slip = '0.5'; SlipTag = '0p5'; Label = 'SENS 0.5bps global M=27' },
    @{ TopK = '1,2,3'; Tag = 'global27'; Slip = '3.0'; SlipTag = '3p0'; Label = 'SENS 3.0bps global M=27' }
)
foreach ($spec in $sensSpecs) {
    $outJson = Join-Path $ReportsDir ("sens_24m_{0}_slip{1}.json" -f $spec.Tag, $spec.SlipTag)
    $null = Invoke-ScanXs -Label $spec.Label -InputPath $DirMain -TopK $spec.TopK -SlippageBps $spec.Slip -OutPath $outJson
    $runCount++
}

$sw.Stop()
Write-Host ("Completed {0} scan-xs runs in {1:N2}s." -f $runCount, $sw.Elapsed.TotalSeconds)

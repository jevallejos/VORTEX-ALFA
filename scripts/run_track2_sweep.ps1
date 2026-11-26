$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Binary = Join-Path $RepoRoot 'target\release\vortex-alfa.exe'
if (-not (Test-Path $Binary)) {
    throw "Release binary not found at $Binary. Run 'cargo build --release' first."
}

$ReportsDir = Join-Path $RepoRoot 'data_local\reports'
$PerSymDir = Join-Path $ReportsDir 'per_symbol'
New-Item -ItemType Directory -Force -Path $ReportsDir | Out-Null
New-Item -ItemType Directory -Force -Path $PerSymDir | Out-Null

$Symbols = @(
    'BTCUSDT', 'ETHUSDT', 'SOLUSDT', 'AVAXUSDT',
    'TAOUSDT', 'SUIUSDT', 'ORDIUSDT', 'PENDLEUSDT',
    'WIFUSDT', 'LINKUSDT', 'NEARUSDT', 'LTCUSDT'
)
$Timeframes = @('15m', '1h', '4h')

function Invoke-ScanMeso {
    param(
        [string]$Label,
        [string]$InputPath,
        [string]$SlippageBps,
        [string]$OutPath
    )
    Write-Host "Running [$Label] slip=$SlippageBps -> $OutPath"
    $output = & $Binary scan-meso --format csv --input $InputPath --slippage-bps $SlippageBps --out $OutPath 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Error "scan-meso failed for [$Label] with exit code $LASTEXITCODE`: $output"
    }
    return $output
}

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$runCount = 0

# 1. Main sweep (24m, 12 pairs, binding slippage 1.5 bps)
foreach ($tf in $Timeframes) {
    $inDir = Join-Path $RepoRoot "data_local\bybit_24m\$tf"
    $outJson = Join-Path $ReportsDir "main_24m_${tf}_slip1p5.json"
    $ascii = Invoke-ScanMeso -Label "MAIN 24m $tf" -InputPath $inDir -SlippageBps '1.5' -OutPath $outJson
    $ascii | Out-Host
    $runCount++
}

# 2. OOS Temporal Stability: Year 1 and Year 2 (12 pairs, slippage 1.5 bps)
foreach ($tf in $Timeframes) {
    $inY1 = Join-Path $RepoRoot "data_local\bybit_24m_y1\$tf"
    $outY1 = Join-Path $ReportsDir "oos_y1_${tf}_slip1p5.json"
    $null = Invoke-ScanMeso -Label "OOS Y1 $tf" -InputPath $inY1 -SlippageBps '1.5' -OutPath $outY1
    $runCount++

    $inY2 = Join-Path $RepoRoot "data_local\bybit_24m_y2\$tf"
    $outY2 = Join-Path $ReportsDir "oos_y2_${tf}_slip1p5.json"
    $null = Invoke-ScanMeso -Label "OOS Y2 $tf" -InputPath $inY2 -SlippageBps '1.5' -OutPath $outY2
    $runCount++
}

# 3. $64 USDT viability (Ex-BTC, 11 pairs, slippage 1.5 bps)
foreach ($tf in $Timeframes) {
    $inEx = Join-Path $RepoRoot "data_local\bybit_24m_exbtc\$tf"
    $outEx = Join-Path $ReportsDir "exbtc_24m_${tf}_slip1p5.json"
    $null = Invoke-ScanMeso -Label "EX-BTC 24m $tf" -InputPath $inEx -SlippageBps '1.5' -OutPath $outEx
    $runCount++
}

# 4. Secondary slippage sensitivity (0.5 and 3.0 bps on full 24m, 12 pairs)
foreach ($tf in $Timeframes) {
    $inDir = Join-Path $RepoRoot "data_local\bybit_24m\$tf"
    $out05 = Join-Path $ReportsDir "sens_24m_${tf}_slip0p5.json"
    $null = Invoke-ScanMeso -Label "SENS 0.5bps $tf" -InputPath $inDir -SlippageBps '0.5' -OutPath $out05
    $runCount++

    $out30 = Join-Path $ReportsDir "sens_24m_${tf}_slip3p0.json"
    $null = Invoke-ScanMeso -Label "SENS 3.0bps $tf" -InputPath $inDir -SlippageBps '3.0' -OutPath $out30
    $runCount++
}

# 5. Per-symbol breakdown across all 3 timeframes and 12 pairs
foreach ($tf in $Timeframes) {
    foreach ($sym in $Symbols) {
        $inCsv = Join-Path $RepoRoot "data_local\bybit_24m\$tf\$sym.csv"
        $outSym = Join-Path $PerSymDir "${tf}_${sym}_slip1p5.json"
        $null = Invoke-ScanMeso -Label "SYM $tf $sym" -InputPath $inCsv -SlippageBps '1.5' -OutPath $outSym
        $runCount++
    }
}

$sw.Stop()
Write-Host ("Completed {0} scan-meso runs in {1:N2}s." -f $runCount, $sw.Elapsed.TotalSeconds)

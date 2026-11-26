$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Binary = Join-Path $RepoRoot 'target\release\vortex-alfa.exe'
if (-not (Test-Path $Binary)) {
    throw "Release binary not found at $Binary. Run 'cargo build --release' first."
}

$Dir24m = Join-Path $RepoRoot 'data_local\bybit_hl_xvenue_24m'
$DirY1  = Join-Path $RepoRoot 'data_local\bybit_hl_xvenue_y1'
$DirY2  = Join-Path $RepoRoot 'data_local\bybit_hl_xvenue_y2'

foreach ($dir in @($Dir24m, $DirY1, $DirY2)) {
    if (-not (Test-Path $dir)) {
        throw "Dataset directory not found: $dir"
    }
    foreach ($sub in @('bybit', 'hyperliquid')) {
        $subPath = Join-Path $dir $sub
        if (-not (Test-Path $subPath)) {
            throw "Dataset subdirectory not found: $subPath"
        }
    }
}

$ReportsDir = Join-Path $RepoRoot 'data_local\reports_track7'
New-Item -ItemType Directory -Force -Path $ReportsDir | Out-Null

function Invoke-ScanXvenue {
    param(
        [string]$Label,
        [string]$DatasetDir,
        [string]$CapitalUsdt,
        [string]$SlippageBps,
        [string]$BridgeFeeUsdt,
        [string]$OutPath
    )
    $bybitDir = Join-Path $DatasetDir 'bybit'
    $hlDir    = Join-Path $DatasetDir 'hyperliquid'
    Write-Host "Running [$Label] cap=$CapitalUsdt slip=${SlippageBps}bps bridge=$BridgeFeeUsdt -> $OutPath"
    $output = & $Binary scan-xvenue `
        --bybit-dir $bybitDir `
        --hl-dir $hlDir `
        --capital-usdt $CapitalUsdt `
        --slippage-bps $SlippageBps `
        --bridge-fee-usdt $BridgeFeeUsdt `
        --out $OutPath 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Error "scan-xvenue failed for [$Label] with exit code $LASTEXITCODE`: $output"
    }
    return $output
}

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$runCount = 0

$scenarios = @(
    @{ Label = 'MAIN 24m cap=2000 slip=1.5 bridge=1';  Dir = $Dir24m; Cap = '2000'; Slip = '1.5'; Bridge = '1'; File = 'main_24m_cap2000_s1p5_b1.json'; Print = $true  },
    @{ Label = 'OOS Y1 cap=2000 slip=1.5 bridge=1';    Dir = $DirY1;  Cap = '2000'; Slip = '1.5'; Bridge = '1'; File = 'oos_y1_cap2000_s1p5_b1.json';   Print = $true  },
    @{ Label = 'OOS Y2 cap=2000 slip=1.5 bridge=1';    Dir = $DirY2;  Cap = '2000'; Slip = '1.5'; Bridge = '1'; File = 'oos_y2_cap2000_s1p5_b1.json';   Print = $true  },
    @{ Label = 'SENS 24m cap=1000 slip=1.5 bridge=1';  Dir = $Dir24m; Cap = '1000'; Slip = '1.5'; Bridge = '1'; File = 'sens_24m_cap1000_s1p5_b1.json'; Print = $true  },
    @{ Label = 'SENS Y1 cap=1000 slip=1.5 bridge=1';   Dir = $DirY1;  Cap = '1000'; Slip = '1.5'; Bridge = '1'; File = 'sens_y1_cap1000_s1p5_b1.json';  Print = $false },
    @{ Label = 'SENS Y2 cap=1000 slip=1.5 bridge=1';   Dir = $DirY2;  Cap = '1000'; Slip = '1.5'; Bridge = '1'; File = 'sens_y2_cap1000_s1p5_b1.json';  Print = $false },
    @{ Label = 'SENS 24m cap=2000 slip=0.5 bridge=1';  Dir = $Dir24m; Cap = '2000'; Slip = '0.5'; Bridge = '1'; File = 'sens_24m_cap2000_s0p5_b1.json'; Print = $false },
    @{ Label = 'SENS 24m cap=2000 slip=3.0 bridge=1';  Dir = $Dir24m; Cap = '2000'; Slip = '3.0'; Bridge = '1'; File = 'sens_24m_cap2000_s3p0_b1.json'; Print = $false },
    @{ Label = 'SENS 24m cap=2000 slip=1.5 bridge=0';  Dir = $Dir24m; Cap = '2000'; Slip = '1.5'; Bridge = '0'; File = 'sens_24m_cap2000_s1p5_b0.json'; Print = $false },
    @{ Label = 'SENS 24m cap=2000 slip=1.5 bridge=5';  Dir = $Dir24m; Cap = '2000'; Slip = '1.5'; Bridge = '5'; File = 'sens_24m_cap2000_s1p5_b5.json'; Print = $false }
)

foreach ($spec in $scenarios) {
    $outJson = Join-Path $ReportsDir $spec.File
    $ascii = Invoke-ScanXvenue `
        -Label $spec.Label `
        -DatasetDir $spec.Dir `
        -CapitalUsdt $spec.Cap `
        -SlippageBps $spec.Slip `
        -BridgeFeeUsdt $spec.Bridge `
        -OutPath $outJson
    if ($spec.Print) {
        $ascii | Out-Host
    }
    $runCount++
}

$sw.Stop()
Write-Host ("Completed {0} scan-xvenue runs in {1:N2}s." -f $runCount, $sw.Elapsed.TotalSeconds)

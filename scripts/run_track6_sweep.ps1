$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Binary = Join-Path $RepoRoot 'target\release\vortex-alfa.exe'
if (-not (Test-Path $Binary)) {
    throw "Release binary not found at $Binary. Run 'cargo build --release' first."
}

$DirCarry30_5y  = Join-Path $RepoRoot 'data_local\bybit_carry30_5y'
$DirCarry30_R1  = Join-Path $RepoRoot 'data_local\bybit_carry30_r1'
$DirCarry30_R2  = Join-Path $RepoRoot 'data_local\bybit_carry30_r2'
$DirCarry30_24m = Join-Path $RepoRoot 'data_local\bybit_carry30_24m'
$DirMacro12_5y  = Join-Path $RepoRoot 'data_local\bybit_macro5y_1d'
$DirMacro12_R1  = Join-Path $RepoRoot 'data_local\bybit_macro5y_r1'
$DirMacro12_R2  = Join-Path $RepoRoot 'data_local\bybit_macro5y_r2'

foreach ($dir in @(
    $DirCarry30_5y,
    $DirCarry30_R1,
    $DirCarry30_R2,
    $DirCarry30_24m,
    $DirMacro12_5y,
    $DirMacro12_R1,
    $DirMacro12_R2
)) {
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

$ReportsDir = Join-Path $RepoRoot 'data_local\reports_track6'
New-Item -ItemType Directory -Force -Path $ReportsDir | Out-Null

function Invoke-ScanCarryUta {
    param(
        [string]$Label,
        [string]$DatasetDir,
        [string]$SlippageBps,
        [string]$BorrowAprPct,
        [string]$OutPath
    )
    $linearDir = Join-Path $DatasetDir 'linear'
    $spotDir   = Join-Path $DatasetDir 'spot'
    Write-Host "Running [$Label] slip=${SlippageBps}bps borrow=${BorrowAprPct}% -> $OutPath"
    $output = & $Binary scan-carry-uta --linear-dir $linearDir --spot-dir $spotDir --slippage-bps $SlippageBps --borrow-apr-pct $BorrowAprPct --out $OutPath 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Error "scan-carry-uta failed for [$Label] with exit code $LASTEXITCODE`: $output"
    }
    return $output
}

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$runCount = 0

$scenarios = @(
    @{ Label = 'MAIN 30p 5y s=1.5 b=15.0';     Dir = $DirCarry30_5y;  Slip = '1.5'; Borrow = '15.0'; File = 'main_30p_5y_s1p5_b15.json';     Print = $true  },
    @{ Label = 'OOS 30p R1 s=1.5 b=15.0';      Dir = $DirCarry30_R1;  Slip = '1.5'; Borrow = '15.0'; File = 'oos_30p_r1_s1p5_b15.json';      Print = $false },
    @{ Label = 'OOS 30p R2 s=1.5 b=15.0';      Dir = $DirCarry30_R2;  Slip = '1.5'; Borrow = '15.0'; File = 'oos_30p_r2_s1p5_b15.json';      Print = $true  },
    @{ Label = 'OOS 30p 24m s=1.5 b=15.0';     Dir = $DirCarry30_24m; Slip = '1.5'; Borrow = '15.0'; File = 'oos_30p_24m_s1p5_b15.json';     Print = $true  },
    @{ Label = 'CTRL 12p 5y s=1.5 b=15.0';     Dir = $DirMacro12_5y;  Slip = '1.5'; Borrow = '15.0'; File = 'ctrl_12p_5y_s1p5_b15.json';     Print = $false },
    @{ Label = 'CTRL 12p R1 s=1.5 b=15.0';     Dir = $DirMacro12_R1;  Slip = '1.5'; Borrow = '15.0'; File = 'ctrl_12p_r1_s1p5_b15.json';     Print = $false },
    @{ Label = 'CTRL 12p R2 s=1.5 b=15.0';     Dir = $DirMacro12_R2;  Slip = '1.5'; Borrow = '15.0'; File = 'ctrl_12p_r2_s1p5_b15.json';     Print = $false },
    @{ Label = 'SENS 30p 5y s=1.5 b=0.0';      Dir = $DirCarry30_5y;  Slip = '1.5'; Borrow = '0.0';  File = 'sens_30p_5y_s1p5_b0.json';      Print = $false },
    @{ Label = 'SENS 30p 5y s=1.5 b=25.0';     Dir = $DirCarry30_5y;  Slip = '1.5'; Borrow = '25.0'; File = 'sens_30p_5y_s1p5_b25.json';     Print = $false },
    @{ Label = 'SENS 30p 5y s=0.5 b=15.0';     Dir = $DirCarry30_5y;  Slip = '0.5'; Borrow = '15.0'; File = 'sens_30p_5y_s0p5_b15.json';     Print = $false },
    @{ Label = 'SENS 30p 5y s=3.0 b=15.0';     Dir = $DirCarry30_5y;  Slip = '3.0'; Borrow = '15.0'; File = 'sens_30p_5y_s3p0_b15.json';     Print = $false },
    @{ Label = 'SENS 30p 24m s=1.5 b=0.0';     Dir = $DirCarry30_24m; Slip = '1.5'; Borrow = '0.0';  File = 'sens_30p_24m_s1p5_b0.json';     Print = $false },
    @{ Label = 'SENS 30p 24m s=1.5 b=25.0';    Dir = $DirCarry30_24m; Slip = '1.5'; Borrow = '25.0'; File = 'sens_30p_24m_s1p5_b25.json';    Print = $false },
    @{ Label = 'SENS 30p 24m s=0.5 b=15.0';    Dir = $DirCarry30_24m; Slip = '0.5'; Borrow = '15.0'; File = 'sens_30p_24m_s0p5_b15.json';    Print = $false },
    @{ Label = 'SENS 30p 24m s=3.0 b=15.0';    Dir = $DirCarry30_24m; Slip = '3.0'; Borrow = '15.0'; File = 'sens_30p_24m_s3p0_b15.json';    Print = $false }
)

foreach ($spec in $scenarios) {
    $outJson = Join-Path $ReportsDir $spec.File
    $ascii = Invoke-ScanCarryUta -Label $spec.Label -DatasetDir $spec.Dir -SlippageBps $spec.Slip -BorrowAprPct $spec.Borrow -OutPath $outJson
    if ($spec.Print) {
        $ascii | Out-Host
    }
    $runCount++
}

$sw.Stop()
Write-Host ("Completed {0} scan-carry-uta runs in {1:N2}s." -f $runCount, $sw.Elapsed.TotalSeconds)

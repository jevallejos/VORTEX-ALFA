# VORTEX-ALFA — Phase 6 Track 7 Cross-Venue Perp–Perp Funding Arbitrage (`Bybit Linear` vs. `Hyperliquid Perp`, `4h`, `24` Meses) Forensic Report

> **Veredicto Empírico Global**: **`NO — DESCARTADO PARA DESPLIEGUE REAL EN CAPITAL PEQUEÑO ($1,000–$2,000 USDT)`**. **`0 / 9`** configuraciones aprueban en **`24m`**, **`Y1`** o **`Y2`** tanto bajo el corte local (`M = 9`, `t_crit = 3.5000`) como bajo el corte acumulado del proyecto (`M_cum = 116`, `t_crit_cum116 = 3.9265`) o la compuerta económica (**`CAGR >= 8.0%` en `24m` y `>= 6.0%` en `Y1` e `Y2`**). La mejor celda nominal en `24m` (`$2,000` USDT, `1.5 bps` slippage, puente Arbitrum `$1.00`), **`Lev1x_Slow30d_Top2`**, alcanza apenas `NetVIP0 = +0.0656 bps/4h`, **`CAGR = +1.43%`** (`AnnSR = 0.92`, `MaxDD = 0.51%`, `t_NW = 3.003`, `DSR = 0.2337`, `DSR_116 = 0.0167`, `InsufficientSample`), es decir `$28.60/año` brutos operacionales sobre `$2,000` (`+1.17%` = `$11.70/año` sobre `$1,000`), por debajo de las tasas USD libres de riesgo (`~4%–5%`) y a `~5.6×` de distancia del umbral `>= 8.0%`. Todas las variantes apalancadas `Lev2x` (`<= +0.47%`) y `Lev3x` (`-4.58%` a `-7.52%`) destruyen capital por recentrados bilaterales frecuentes (`75–137` eventos a `18%`), puentes USDC frecuentes (`40–82`) y tarifas round-trip de **`26.00 bps`** frente a un spread medio real de solo **`+0.1939 bps/4h`**. Incluso bajo el diagnóstico no vinculante **`XVenue-MakerDiag`** (`3.50 bps/turn_L`, `0 bps` slippage), **`0 / 9`** celdas alcanzan `Pass` estadístico o `8.0%` CAGR en `24m` (máximo `5.70%` en `Lev3x_Fast7d_Top4`, `DSR = 0.8556`, `InsufficientSample`).
> **Engine**: `target\release\vortex-alfa.exe scan-xvenue` ([`src/application/falsifier_xvenue_carry.rs`](../src/application/falsifier_xvenue_carry.rs), [`src/interfaces/cli_xvenue_carry.rs`](../src/interfaces/cli_xvenue_carry.rs))
> **Universo & Timeframe**: `16` pares solapados `Bybit V5 Linear` vs. `Hyperliquid Perp` (`0:BTCUSDT/BTC .. 15:INJUSDT/INJ`), velas cerradas **`4h`** (`interval = 240m`): **`16 / 16`** activos al **`100.0%`** sin huecos ni precios sintéticos (`70,080` velas Bybit + `70,080` velas HL = `140,160` eventos únicos en `4,380` slices `4h`).
> **Ventana Histórica UTC**: `2024-11-25T00:00:00Z` (`1732492800000`) a `2026-11-25T00:00:00Z` (`1795564800000`), **`24` meses exactos** (`4,380` barras `4h`; última apertura `2026-11-24T20:00:00Z`). Particiones OOS: **`Y1`** `2024-11-25T00:00:00Z`–`2025-11-25T00:00:00Z` (`2,190` barras `4h`) | **`Y2`** `2025-11-25T00:00:00Z`–`2026-11-25T00:00:00Z` (`2,190` barras `4h`).
> **Fricción Vinculante (`Bybit VIP-0` + `HL Tier 0` + `1.5 bps` slip/pata + Puente Arbitrum `$1.00`) vs. `XVenue-MakerDiag`**: `c_xv = 5.50 + 4.50 + 2 * 1.50 = 13.00 bps`/`turn_L` (`26.00 bps` RT); diagnóstico `XVenue-MakerDiag = 2.00 + 1.50 = 3.50 bps`/`turn_L` (`7.00 bps` RT, `is_binding = false`). Peaje puente `$1.00 USDC` (`5.0 bps` sobre `$2,000` / `10.0 bps` sobre `$1,000`) al alcanzar desbalance entre venues `|imb| >= 15%`.
> **Compuertas de Código**: `6 / 6` compuertas PS en verde (`52/52` tests `ok`: `47` previos + `5` en [`tests/xvenue_carry_tests.rs`](../tests/xvenue_carry_tests.rs); `50/50` archivos `src/**/*.rs <= 350` líneas, máximo global `348` en `stats.rs`, módulos Track 7 `259 / 208 / 261 / 250 / 138`; `0` warnings Clippy; `0` patrones prohibidos; `58/58` `UTF-8 without BOM` + `LF-only`).
> **Index**: [`DOCS/INDEX.md`](./INDEX.md) | [`DOCS/PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md) | [`DOCS/PHASE_1A_TRACK2_FALSIFICATION_REPORT.md`](./PHASE_1A_TRACK2_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_1B_TRACK1_FALSIFICATION_REPORT.md`](./PHASE_1B_TRACK1_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md`](./PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_3A_TRACK4_MM_DESIGN_REPORT.md`](./PHASE_3A_TRACK4_MM_DESIGN_REPORT.md) | [`DOCS/PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md`](./PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_4_TRACK5_MACRO_FALSIFICATION_REPORT.md`](./PHASE_4_TRACK5_MACRO_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_5_TRACK6_CARRY_UTA_REPORT.md`](./PHASE_5_TRACK6_CARRY_UTA_REPORT.md)

---

## 0. Conformidad con las 4 Leyes Inviolables ("Las 4 D") en Fase 6

| Ley | Regla | Evidencia Verificada en Código Fuente y Auditoría | Estado |
|---|---|---|:---:|
| **Ley 1 — Cero Duplicación (SSOT / DRY)** | Reutilizar `domain::types`, `FeeSchedule`, `stats`, `compute_equity_metrics(2190.0)`, `weight_turnover`, `payback_apr_hurdle_pct` y `CsvCandleReader`; cero cálculos estadísticos o financieros en Python/PS | [`xvenue_carry.rs:7, 94`](../src/domain/signals/xvenue_carry.rs#L7) reutiliza [`payback_apr_hurdle_pct`](../src/domain/signals/carry_uta.rs#L49); [`xvenue_carry_accounting.rs:3-10, 106, 232-243`](../src/application/xvenue_carry_accounting.rs#L3-L243) reutiliza `CostPerTurnover`, [`compute_equity_metrics`](../src/application/macro_baselines.rs#L132) (`bars_per_year = 2190.0`), [`weight_turnover`](../src/application/macro_baselines.rs#L334) y [`welford_mean`](../src/domain/stats.rs#L28); [`falsifier_xvenue_carry.rs:124-126, 176-195`](../src/application/falsifier_xvenue_carry.rs#L124-L195) reutiliza [`FeeSchedule::bybit_vip0`](../src/domain/fees.rs), [`evaluate_hypothesis_batch`](../src/domain/stats.rs#L277) y [`t_crit_holm`](../src/domain/stats.rs#L131); [`cli_xvenue_carry.rs:7-8, 64-68`](../src/interfaces/cli_xvenue_carry.rs#L7-L68) reutiliza [`CsvCandleReader`](../src/infrastructure/csv_candle_reader.rs) y `cli.rs` (`fmt_opt`, `take_arg`, `write_json_pretty`). [`fetch_bybit_hl_xvenue_24m.py`](../scripts/fetch_bybit_hl_xvenue_24m.py), [`audit_xvenue_datasets.py`](../scripts/audit_xvenue_datasets.py) y [`run_track7_sweep.ps1`](../scripts/run_track7_sweep.ps1) solo descargan/auditan y disparan el binario Rust. | **PASS** |
| **Ley 2 — Cero Desviación (Strict Scope Lock)** | Prohibido modificar Tracks 1–6, `stats.rs`, `fees.rs`, `ledger.rs`, `types.rs`, `Cargo.toml` o los `47` tests existentes | `git diff --name-only` (pre-docs) confirma únicamente los `4` archivos de registro mínimo: [`src/application/mod.rs`](../src/application/mod.rs), [`src/domain/signals/mod.rs`](../src/domain/signals/mod.rs), [`src/interfaces/cli.rs`](../src/interfaces/cli.rs) (`scan-xvenue` en [`L56, L66`](../src/interfaces/cli.rs#L56-L66)) y [`src/interfaces/mod.rs`](../src/interfaces/mod.rs). Track 7 es 100% aditivo (`5` módulos SRP nuevos + [`tests/xvenue_carry_tests.rs`](../tests/xvenue_carry_tests.rs), `5` tests nuevos; `52/52` tests totales `ok`). | **PASS** |
| **Ley 3 — Cero Diluvio (`<= 350` líneas, `3` deps `Cargo.toml`)** | `5` módulos `<= 270`, todos `src/**/*.rs <= 350` tras `cargo fmt --all`, `Cargo.toml` con `3` deps, Python stdlib puro | Todos los `50/50` archivos en `src/**/*.rs` cumplen `<= 350` líneas físicas (`ReadAllLines`, máximo global `348` en [`stats.rs`](../src/domain/stats.rs)): [`xvenue_carry.rs`](../src/domain/signals/xvenue_carry.rs) (`259 <= 260`), [`xvenue_carry_types.rs`](../src/application/xvenue_carry_types.rs) (`208 <= 210`), [`xvenue_carry_accounting.rs`](../src/application/xvenue_carry_accounting.rs) (`261 <= 270`), [`falsifier_xvenue_carry.rs`](../src/application/falsifier_xvenue_carry.rs) (`250 <= 250`), [`cli_xvenue_carry.rs`](../src/interfaces/cli_xvenue_carry.rs) (`138 <= 170`) y [`cli.rs`](../src/interfaces/cli.rs) (`330 <= 335`). Cero tuplas anónimas `.0`/`.1` en Track 7. [`Cargo.toml`](../Cargo.toml) conserva exactamente `3` dependencias (`thiserror`, `serde`, `serde_json`); scripts Python `0` `pip install`. | **PASS** |
| **Ley 4 — Cero Disfraz (Zero Disguise)** | Tarifas reales Bybit VIP-0 + HL Tier 0, basis real sin precios sintéticos, margen dividido + puente `$1.00` Arbitrum, y reporte íntegro de `9` celdas frente a `Cum116` y Kill-Gate | [`XVenueFeeProfiles::new`](../src/application/xvenue_carry_types.rs#L71-L101) fija `13.00 bps/turn_L` (`5.50 + 4.50 + 2*1.5`) vinculante y `3.50 bps` `MakerDiag` (`is_binding = false`); [`settle_xvenue_bar`](../src/application/xvenue_carry_accounting.rs#L156-L166) liquida basis real de velas `4h` de ambos exchanges y peaje puente Arbitrum `$1.00`; se reportan íntegramente las `9` celdas en `24m`, `Y1`, `Y2` y los `7` escenarios de sensibilidad junto al veredicto frío **`NO`** (`0 / 9`). | **PASS** |

---

## 1. Manifiesto de los 16 Pares Solapados (`4h`, `24` Meses) y Auditoría de Integridad

Descarga resumible ejecutada mediante [`scripts/fetch_bybit_hl_xvenue_24m.py`](../scripts/fetch_bybit_hl_xvenue_24m.py) ([`download_manifest.json`](../data_local/bybit_hl_xvenue_24m/download_manifest.json)) respetando control estricto de peso HL (`<= 860` weight / `61s`) en `4` lotes secuenciales de `4` monedas, y auditada con aritmética exacta `Decimal` por [`scripts/audit_xvenue_datasets.py`](../scripts/audit_xvenue_datasets.py) ([`audit_report.json`](../data_local/bybit_hl_xvenue_24m/audit_report.json)). Los **`16 / 16`** pares completaron exactamente **`4,380 / 4,380`** barras `4h` solapadas (`2,190` en `Y1` + `2,190` en `Y2`), con **`0`** barras exclusivas de un solo exchange (`bybit_only_bars = 0`, `hl_only_bars = 0`) y `0` precios sintéticos.

| ID | Par Bybit / HL | `szDec` · `maxLev` HL | Fuente Bybit (`4h`) | Cobertura Dual (`24m / Y1 / Y2` · Gaps) | Intervalos Funding (`BB` / `HL`) | Raw Fund `BB` (`Sub-4h`) | Raw Fund `HL` (`Sub-4h`) | Raw/CSV Sum `BB` (bps) | Raw/CSV Sum `HL` (bps) | `Max Jitter HL` (`>60s`) | `diff_bps` (`BB`/`HL`) |
|---:|---|---:|---|---|---|---:|---:|---:|---:|---:|---|
| 0 | `BTCUSDT` / `BTC` | `5` · `40x` | `local_xvenue_remapped` | `4,380` (`2,190 / 2,190`) · `0` | `8.0h` / `1.0h` | 2,190 (`0`) | 17,520 (`13,140`) | `911.9298` | `1848.104095` | `887,645 ms` (`2`) | `0` / `0` |
| 1 | `ETHUSDT` / `ETH` | `4` · `25x` | `local_xvenue_remapped` | `4,380` (`2,190 / 2,190`) · `0` | `8.0h` / `1.0h` | 2,190 (`0`) | 17,520 (`13,140`) | `913.7089` | `1695.991985` | `887,645 ms` (`2`) | `0` / `0` |
| 2 | `SOLUSDT` / `SOL` | `2` · `20x` | `reused_bybit_xs30_4h` | `4,380` (`2,190 / 2,190`) · `0` | `8.0h` / `1.0h` | 2,190 (`0`) | 17,520 (`13,140`) | `411.5407` | `891.122052` | `887,645 ms` (`2`) | `0` / `0` |
| 3 | `AVAXUSDT` / `AVAX` | `2` · `10x` | `local_xvenue_remapped` | `4,380` (`2,190 / 2,190`) · `0` | `8.0h` / `1.0h` | 2,190 (`0`) | 17,520 (`13,140`) | `552.459` | `1286.786003` | `887,645 ms` (`2`) | `0` / `0` |
| 4 | `TAOUSDT` / `TAO` | `3` · `5x` | `reused_bybit_xs30_4h` | `4,380` (`2,190 / 2,190`) · `0` | `4.0, 8.0h` / `1.0h` | 4,226 (`0`) | 17,520 (`13,140`) | `1050.2966` | `2162.852867` | `887,645 ms` (`2`) | `0` / `0` |
| 5 | `SUIUSDT` / `SUI` | `1` · `10x` | `reused_bybit_xs30_4h` | `4,380` (`2,190 / 2,190`) · `0` | `8.0h` / `1.0h` | 2,190 (`0`) | 17,520 (`13,140`) | `1061.7912` | `1391.764626` | `887,645 ms` (`2`) | `0` / `0` |
| 6 | `ORDIUSDT` / `ORDI` | `2` · `3x` | `reused_bybit_xs30_4h` | `4,380` (`2,190 / 2,190`) · `0` | `4.0, 8.0h` / `1.0h` | 4,226 (`0`) | 17,520 (`13,140`) | `1244.6355` | `2110.093725` | `887,645 ms` (`2`) | `0` / `0` |
| 7 | `PENDLEUSDT` / `PENDLE` | `0` · `5x` | `reused_bybit_xs30_4h` | `4,380` (`2,190 / 2,190`) · `0` | `8.0h` / `1.0h` | 2,190 (`0`) | 17,520 (`13,140`) | `1170.2135` | `2545.828551` | `887,645 ms` (`2`) | `0` / `0` |
| 8 | `WIFUSDT` / `WIF` | `0` · `5x` | `reused_bybit_xs30_4h` | `4,380` (`2,190 / 2,190`) · `0` | `4.0, 8.0h` / `1.0h` | 4,226 (`0`) | 17,520 (`13,140`) | `510.827` | `1734.885987` | `887,645 ms` (`2`) | `0` / `0` |
| 9 | `LINKUSDT` / `LINK` | `1` · `10x` | `reused_bybit_xs30_4h` | `4,380` (`2,190 / 2,190`) · `0` | `8.0h` / `1.0h` | 2,190 (`0`) | 17,520 (`13,140`) | `1033.3196` | `2436.949425` | `887,645 ms` (`2`) | `0` / `0` |
| 10 | `BNBUSDT` / `BNB` | `3` · `10x` | `reused_bybit_xs30_4h` | `4,380` (`2,190 / 2,190`) · `0` | `8.0h` / `1.0h` | 2,190 (`0`) | 17,520 (`13,140`) | `605.968` | `1055.796692` | `887,645 ms` (`2`) | `0` / `0` |
| 11 | `XRPUSDT` / `XRP` | `0` · `20x` | `reused_bybit_xs30_4h` | `4,380` (`2,190 / 2,190`) · `0` | `8.0h` / `1.0h` | 2,190 (`0`) | 17,520 (`13,140`) | `761.9967` | `1392.699219` | `887,645 ms` (`2`) | `0` / `0` |
| 12 | `DOGEUSDT` / `DOGE` | `0` · `10x` | `local_xvenue_remapped` | `4,380` (`2,190 / 2,190`) · `0` | `8.0h` / `1.0h` | 2,190 (`0`) | 17,520 (`13,140`) | `951.6266` | `1761.317199` | `887,645 ms` (`2`) | `0` / `0` |
| 13 | `AAVEUSDT` / `AAVE` | `2` · `10x` | `local_xvenue_remapped` | `4,380` (`2,190 / 2,190`) · `0` | `8.0h` / `1.0h` | 2,190 (`0`) | 17,520 (`13,140`) | `949.1709` | `2525.095385` | `887,645 ms` (`2`) | `0` / `0` |
| 14 | `ARBUSDT` / `ARB` | `1` · `10x` | `reused_bybit_xs30_4h` | `4,380` (`2,190 / 2,190`) · `0` | `8.0h` / `1.0h` | 2,190 (`0`) | 17,520 (`13,140`) | `987.2377` | `1364.769877` | `887,645 ms` (`2`) | `0` / `0` |
| 15 | `INJUSDT` / `INJ` | `1` · `5x` | `local_xvenue_remapped` | `4,380` (`2,190 / 2,190`) · `0` | `1.0, 6.0, 8.0h` / `1.0h` | 2,339 (`128`) | 17,520 (`13,140`) | `-240.5598` | `259.742674` | `887,645 ms` (`2`) | `0` / `0` |

### 1.1 Auditoría de Integridad & Spread Descriptivo de Funding (`HL_bps - Bybit_bps`)

- **Resultado de Auditoría ([`audit_report.json`](../data_local/bybit_hl_xvenue_24m/audit_report.json)) — Estado `PASS`**:
  - `manifest_failures = 0`, `raw_bybit_funding_checked = 16` (`failures = 0`), `raw_hl_funding_checked = 16` (`failures = 0`), `csv_files_checked = 96`, `rows_checked = 280,320`, `missing_files_total = 0`, `empty_files_total = 0`.
  - Integridad numérica y temporal impecable (`0` en los `29` contadores de fallo): `nan_inf_total = 0`, `non_pos_price_total = 0`, `invalid_ohlc_total = 0`, `neg_vol_total = 0`, `duplicate_ts_total = 0`, `backward_ts_total = 0`, `wrong_sym_id_total = 0`, `out_of_window_total = 0`, `misaligned_ts_total = 0`, `header_failures = 0`, `encoding_failures = 0`, `gap_files_total = 0`, `gap_count_total = 0`, `missing_bars_total = 0`, `incomplete_coverage_files = 0`, `split_mismatch_total = 0`, `cross_venue_alignment_failures = 0`, `per_bar_funding_mismatch_total = 0`, `unmatched_funding_events_total = 0`, `funding_reconciliation_failures = 0`.
- **Estadística descriptiva del spread bruto de funding `HL_bps - Bybit_bps` (`4h`) por moneda**:
  - Media equiponderada del universo: **`+0.1939 bps/4h`** en `24m` (mediana `0.1817`), cayendo **`-30.5%`** de **`+0.2288 bps/4h`** en `Y1` a **`+0.1590 bps/4h`** en `Y2`. Acumulado `24m` total: HL **`+26,463.8004 bps`** vs. Bybit **`+12,876.1619 bps`** (`SumDiff = +13,587.6385 bps`).

| ID | Coin | `24m` Mean (`Std`) | `24m` Med (`P10` / `P90`) | `24m` Sum Diff | `24m` `HL>BB%` | `Y1` Mean (`Std`) | `Y1` Sum Diff (`HL>BB%`) | `Y2` Mean (`Std`) | `Y2` Sum Diff (`HL>BB%`) |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 0 | `BTC` | `+0.2137` (`0.5827`) | `+0.2791` (`-0.5000` / `+0.5895`) | `+936.17` | `68.74%` | `+0.2984` (`0.7187`) | `+653.40` (`70.82%`) | `+0.1291` (`0.3850`) | `+282.77` (`66.67%`) |
| 1 | `ETH` | `+0.1786` (`0.6282`) | `+0.2548` (`-0.5000` / `+0.6111`) | `+782.28` | `65.09%` | `+0.1906` (`0.7881`) | `+417.33` (`61.05%`) | `+0.1666` (`0.4099`) | `+364.95` (`69.13%`) |
| 2 | `SOL` | `+0.1095` (`0.8313`) | `+0.1902` (`-0.7136` / `+0.8091`) | `+479.58` | `58.97%` | `+0.1791` (`0.9823`) | `+392.28` (`60.64%`) | `+0.0399` (`0.6386`) | `+87.30` (`57.31%`) |
| 3 | `AVAX` | `+0.1677` (`1.3252`) | `+0.2193` (`-0.6346` / `+0.8194`) | `+734.33` | `58.97%` | `+0.2525` (`1.7618`) | `+552.88` (`60.91%`) | `+0.0829` (`0.6282`) | `+181.44` (`57.03%`) |
| 4 | `TAO` | `+0.2540` (`1.6132`) | `+0.0000` (`-0.3483` / `+1.2046`) | `+1,112.56` | `35.02%` | `+0.2713` (`1.9538`) | `+594.25` (`38.26%`) | `+0.2367` (`1.1782`) | `+518.31` (`31.78%`) |
| 5 | `SUI` | `+0.0753` (`1.1337`) | `+0.1245` (`-0.7376` / `+0.6386`) | `+329.97` | `55.39%` | `+0.0992` (`1.4765`) | `+217.32` (`54.43%`) | `+0.0514` (`0.6244`) | `+112.65` (`56.35%`) |
| 6 | `ORDI` | `+0.1976` (`0.7792`) | `+0.0000` (`-0.3720` / `+1.0147`) | `+865.46` | `30.75%` | `+0.2312` (`0.8243`) | `+506.23` (`37.21%`) | `+0.1640` (`0.7300`) | `+359.23` (`24.29%`) |
| 7 | `PENDLE` | `+0.3141` (`0.9645`) | `+0.5000` (`-0.5000` / `+1.1265`) | `+1,375.62` | `65.11%` | `+0.3693` (`1.1252`) | `+808.78` (`64.06%`) | `+0.2588` (`0.7673`) | `+566.84` (`66.16%`) |
| 8 | `WIF` | `+0.2795` (`1.1825`) | `+0.0000` (`-0.4518` / `+1.3373`) | `+1,224.06` | `38.17%` | `+0.2755` (`1.2797`) | `+603.39` (`39.50%`) | `+0.2834` (`1.0768`) | `+620.67` (`36.85%`) |
| 9 | `LINK` | `+0.3205` (`1.6587`) | `+0.4509` (`-0.5000` / `+0.8446`) | `+1,403.63` | `69.68%` | `+0.3688` (`2.2798`) | `+807.68` (`67.44%`) | `+0.2721` (`0.5491`) | `+595.95` (`71.92%`) |
| 10 | `BNB` | `+0.1027` (`0.8580`) | `+0.2077` (`-0.6804` / `+0.8200`) | `+449.83` | `57.63%` | `+0.0380` (`1.0377`) | `+83.20` (`53.15%`) | `+0.1674` (`0.6225`) | `+366.63` (`62.10%`) |
| 11 | `XRP` | `+0.1440` (`1.0139`) | `+0.2142` (`-0.5724` / `+0.7501`) | `+630.70` | `59.95%` | `+0.2012` (`1.3233`) | `+440.61` (`61.10%`) | `+0.0868` (`0.5468`) | `+190.10` (`58.81%`) |
| 12 | `DOGE` | `+0.1849` (`1.0074`) | `+0.2310` (`-0.5226` / `+0.6842`) | `+809.69` | `61.53%` | `+0.2225` (`1.3185`) | `+487.20` (`60.50%`) | `+0.1473` (`0.5375`) | `+322.49` (`62.56%`) |
| 13 | `AAVE` | `+0.3598` (`1.2318`) | `+0.4903` (`-0.5000` / `+0.9069`) | `+1,575.92` | `72.40%` | `+0.4605` (`1.6380`) | `+1,008.59` (`72.56%`) | `+0.2591` (`0.5761`) | `+567.33` (`72.24%`) |
| 14 | `ARB` | `+0.0862` (`1.0223`) | `+0.1533` (`-0.8582` / `+0.8106`) | `+377.53` | `55.25%` | `+0.1335` (`1.2567`) | `+292.36` (`55.21%`) | `+0.0389` (`0.7120`) | `+85.18` (`55.30%`) |
| 15 | `INJ` | `+0.1142` (`2.0409`) | `+0.1850` (`-0.9512` / `+0.9877`) | `+500.30` | `55.71%` | `+0.0693` (`1.0865`) | `+151.86` (`53.01%`) | `+0.1591` (`2.6735`) | `+348.44` (`58.40%`) |

> [!WARNING]
> **Divulgaciones Metodológicas y de Mercado Real (`16` pares `4h`, `Bybit` vs. `Hyperliquid`)**:
> 1. **Anomalía real de jitter en `fundingHistory` de Hyperliquid (julio 2025)**: Los `16 / 16` activos de Hyperliquid registran exactamente `2` retrasos de liquidación horaria superiores a `60s` en el endpoint público: `2025-07-19T10:00:00Z` (`raw_time_ms = 1752920087645`, snapped `1752919200000`, jitter `+887,645 ms ≈ 14m 48s`) y `2025-07-27T12:00:00Z` (`raw_time_ms = 1753617710963`, snapped `1753617600000`, jitter `+110,963 ms ≈ 1m 51s`). Se encajan por diagrama de Voronoi a la hora exacta más cercana (`MAX_JITTER_MS = 1_799_999` en [`fetch_bybit_hl_xvenue_24m.py`](../scripts/fetch_bybit_hl_xvenue_24m.py) y [`audit_xvenue_datasets.py`](../scripts/audit_xvenue_datasets.py)), logrando **`0`** colisiones (`duplicate_snapped_count = 0`), **`0`** eventos sin emparejar y **`diff_bps = 0`** en los `16` pares.
> 2. **Sesgo residual de supervivencia**: El panel `0:BTC..15:INJ` exige activos líquidos listados en ambos exchanges hasta `2026-11-25`, omitiendo pares delistados o ilíquidos. Pese a esta ventaja ex-post, ninguna de las `9` configuraciones supera la falsación ni el Kill-Gate económico.
> 3. **Simplificaciones conservadoras del modelo de margen dividido (`$1.00` Arbitrum) vs. fricciones de producción**: El motor asume re-cierre/re-apertura bilateral en barra `4h` sin *leg-risk* asíncrono ni latencia de puente Arbitrum (`~15–30 min` reales en congestión donde la pata ganadora podría moverse antes de acreditar USDC), y sin límites por tramos de *maintenance margin* de HL ni tasas negativas de préstamo/retiro.

---

## 2. Trazabilidad `Ecuación (LaTeX) -> archivo:línea` (Track 7 Cross-Venue `4h`)

| Concepto / Ecuación (LaTeX) | Ubicación Exacta (`archivo:línea`) | Análisis Dimensional | Guardas de Dominio y Numéricas |
|---|---|---|---|
| **Spread de funding e historial causal (`4h`)**:<br>$\Delta f_{i,t} = f^{\text{HL}}_{i,t} - f^{\text{Byb}}_{i,t}$,<br>$\text{APR}^{\text{signed}}_{i,t} = \frac{2190}{100 \cdot W}\sum_{j=0}^{W-1}\Delta f_{i,t-j}\;[\%]$ | [`src/domain/signals/xvenue_carry.rs:8, 129-141, 253-259`](../src/domain/signals/xvenue_carry.rs#L129-L141) (`window_signed_apr_pct` `L253-259`) | $f$ en $[\text{bps/4h}]$; `BARS_4H_PER_YEAR = 2190.0`; $\text{APR}^{\text{signed}}$ en $[\%]$ | Exige presencia simultánea en `bybit` y `hl`, volumen finito `>= 0`, `funding_rate_bps` finito y timestamps monótonos en ambos venues (`check_venue` `L123-124, L227-249`). |
| **Dirección bidireccional (`+1` vs `-1`) y amortización**:<br>$d_{i,t} = \begin{cases}+1\;(\text{Long Byb / Short HL}) & \text{APR}^{\text{signed}} \ge 0 \\ -1\;(\text{Long HL / Short Byb}) & \text{APR}^{\text{signed}} < 0\end{cases}$,<br>$\Delta\text{APR}_{\text{hurdle}} = \frac{2 \cdot c_{\text{xv,VIP0}} \cdot 365}{100 \cdot D_{\text{payback}}}$ | [`src/domain/signals/xvenue_carry.rs:13-31, 91-102, 163-218`](../src/domain/signals/xvenue_carry.rs#L163-L218) $\to$ [`carry_uta.rs:49`](../src/domain/signals/carry_uta.rs#L49) | $c_{\text{xv,VIP0}} = 13.00\text{ bps/turn}_L$ (`1.5 bps` slip); $D \in \{7,14,30\}\text{d} \implies \Delta\text{APR} \in \{13.56\%, 6.78\%, 3.16\%\}$ | Cambio de signo exige `held >= min_hold_bars` y `-held_apr >= max(entry_apr, payback_hurdle_pct)`; si sale por `held_apr <= exit_apr_pct` queda bloqueado de reversión instantánea (`blocked_flip` `L166-177`). Desempate `SymbolId` ascendente (`L250-252`). |
| **Sizing de grilla (`L_i`) y Utilización (`AvgUtil%`)**:<br>$L_i = \frac{L_{\text{tot}}}{\max(\lvert H_t \rvert,\, \min(\text{min\_div}, K))}$,<br>$\text{Util}\% = \frac{\sum_i \lvert L_i \rvert}{L_{\text{tot}}}\cdot 100$ | [`src/domain/signals/xvenue_carry.rs:211-218`](../src/domain/signals/xvenue_carry.rs#L211-L218); grilla `9` specs [`xvenue_carry_types.rs:18-50`](../src/application/xvenue_carry_types.rs#L18-L50); `util_pct` [`xvenue_carry_accounting.rs:107`](../src/application/xvenue_carry_accounting.rs#L107) | $L_{\text{tot}} \in \{0.50, 1.00, 1.50\}$ nocional por pata (`1x / 2x / 3x`); `Util%` $\in [0, 100\%]$ junto a `avg_gross_l` | En `Lev3x` ($L_{\text{tot}} = 1.50$), `avg_gross_l` llega a `1.4969` (`Util% = 99.79%`) cuando todos los slots se llenan y baja a `84.39%` en `Fast7d` por filtro `entry >= 15%`. |
| **Precios medios armónicos y recentrado bilateral**:<br>$\bar{P}'_v = \frac{L_{\text{new}}}{\frac{L_{\text{old}}}{\bar{P}_v}+\frac{L_{\text{new}}-L_{\text{old}}}{P_{v,\text{open}}}}$,<br>Si $\max\!\left(\lvert r^{\text{Byb}}_{\text{drift}} \rvert,\, \lvert r^{\text{HL}}_{\text{drift}} \rvert\right) \ge \theta_{\text{rec}} \implies \text{Turn}_{\text{rec}} \mathrel{+}= 2\lvert L_i \rvert$ | [`src/application/xvenue_carry_accounting.rs:82-97`](../src/application/xvenue_carry_accounting.rs#L82-L97) | $\theta_{\text{rec}} \in \{0.60, 0.30, 0.18\}$; $\text{Turn}_{\text{rec}}$ suma `2.0 * abs(w)` | Cierra y reabre simultáneamente ambas patas (`Bybit` + `HL`), pagando `2 * abs(w) * c_xv` (`26 bps/turn_L` efectivos en round-trip sobre nocional de pata) y reseteando $\bar{P}_{\text{Byb}}, \bar{P}_{\text{HL}}$ (`L96`). |
| **Desbalance entre venues y peaje Arbitrum USDC**:<br>$\text{imb}_t = \text{imb}_{t-1} + \sum_i w_i\!\left(r^{\text{Byb}}_i - \frac{f^{\text{Byb}}_i}{10^4}\right)$,<br>Si $\lvert \text{imb}_t \rvert \ge 0.15 \implies \text{Bridge}_t = \frac{\text{fee}_{\text{bridge}}}{\text{Cap}}\cdot 10^4,\; \text{imb}_t \leftarrow 0$ | [`src/application/xvenue_carry_accounting.rs:158-166`](../src/application/xvenue_carry_accounting.rs#L158-L166); `BridgeConfig` [`xvenue_carry_types.rs:57-62`](../src/application/xvenue_carry_types.rs#L57-L62) | `Bridge_t` en $[\text{bps/4h}]$ sobre capital (`5.0 bps` a `$1 / $2,000`, `10.0 bps` a `$1 / $1,000`) | Exige `capital_usdt > 0`, `imbalance_trigger > 0`, `bridge_fee_usdt >= 0` y valores finitos (`L139-145`). |
| **Basis real, `BarOpenSettlement`, EOF y Tarifas**:<br>$R_{\text{net},t} = \sum_i w_i(r^{\text{Byb}}_i - r^{\text{HL}}_i)\cdot 10^4 + \sum_i w_i(f^{\text{HL}}_{i,\text{exit}} - f^{\text{Byb}}_{i,\text{exit}}) - \text{Bridge}_t - \text{Turn}_{L,t}\cdot c_{\text{xv}}$ | [`src/application/xvenue_carry_accounting.rs:148-174`](../src/application/xvenue_carry_accounting.rs#L148-L174); [`falsifier_xvenue_carry.rs:86-115`](../src/application/falsifier_xvenue_carry.rs#L86-L115); [`xvenue_carry_types.rs:71-101`](../src/application/xvenue_carry_types.rs#L71-L101) | $c_{\text{VIP0}} = 13.00\text{ bps}$, $c_{\text{MakerDiag}} = 3.50\text{ bps}$ por `turn_L` | Señal al cierre `t`, apertura en `Open(t+1)`; `BarOpenSettlement` lee funding de `Open(t+2)` e ignora funding en EOF `Close(T)` cerrando `eof_close_turn_l = sum |w_i|`. |
| **Métricas `4h`, Holms Local (`M=9`) vs `Cum116` (`M=116`)**:<br>$\text{CAGR}\% = (E_T^{2190/N}-1)\cdot 100,\;\text{AnnSR}=\frac{\mu}{\sigma}\sqrt{2190}$,<br>$t_{\text{crit},9} = 3.5000,\; t_{\text{crit},116} = \Phi^{-1}(1 - \frac{0.05}{2\cdot 116}) = 3.9265$ | [`src/application/xvenue_carry_accounting.rs:242-243`](../src/application/xvenue_carry_accounting.rs#L242-L243) $\to$ [`macro_baselines.rs:132`](../src/application/macro_baselines.rs#L132); [`falsifier_xvenue_carry.rs:124-195`](../src/application/falsifier_xvenue_carry.rs#L124-L195) $\to$ [`stats.rs:131, 277`](../src/domain/stats.rs#L131) | `bars_per_year = 2190.0`; `t_crit_cum116 = 3.92645532` | `build_diag_cell` evalúa `n_min_cum116`, `dsr_cum116` y `verdict_cum116` usando `t_crit_cum116`; `MakerDiag` fija `is_binding = false` (`falsifier_xvenue_carry.rs:125`) y nunca aprueba. |

---

## 3. Tabla Completa del Barrido Principal `24m` (`cap = $2,000`, `slip = 1.5 bps`, `bridge = $1.00`)

Ejecutado con [`scripts/run_track7_sweep.ps1`](../scripts/run_track7_sweep.ps1) sobre [`data_local/bybit_hl_xvenue_24m`](../data_local/bybit_hl_xvenue_24m). Cabecera observada ([`main_24m_cap2000_s1p5_b1.json`](../data_local/reports_track7/main_24m_cap2000_s1p5_b1.json)): `track = Track7-XVenueCarry` | `slices = 4,380` | `bybit = 70,080` | `hl = 70,080` | `events = 140,160` | `t_crit_local = 3.5000` | `t_crit_cum116 = 3.9265` | `sr_std(VIP0) = 0.020338` | `sr_std(MakerDiag) = 0.010782`.

### 3.1 Resultados Vinculantes `XVenue-VIP0` (`13.00 bps/turn_L`, `22` métricas por celda)

| Strategy | N | `n_min` (`Cum116`) | Basis | FundSpd | Bridge | CostVIP0 | TurnL | GrossL | Util% | RecCnt | BrdgCnt | NetVIP0 | CAGR% | AnnSR | MaxDD% | Calmar | `t_NW` | DSR | Verdict | DSR116 | Cum116 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---|
| `Lev1x_Fast7d_Top4` | 4362 | `—` (`—`) | +0.0151 | +0.1692 | 0.0080 | 0.1982 | 0.015245 | 0.4219 | 84.39% | 1 | 7 | -0.0218 | -0.49% | -0.37 | 1.25% | -0.39 | -1.060 | 0.0053 | `Fail` | 0.0000 | `Fail` |
| `Lev1x_Mid14d_Top4` | 4338 | 63,206 (79,547) | +0.0064 | +0.1551 | 0.0069 | 0.1169 | 0.008990 | 0.4990 | 99.79% | 6 | 6 | +0.0377 | +0.82% | 0.65 | 0.74% | 1.11 | 1.953 | 0.1308 | `InsufficientSample` | 0.0054 | `InsufficientSample` |
| `Lev1x_Slow30d_Top2` | 4200 | 31,491 (39,633) | -0.0035 | +0.1305 | 0.0119 | 0.0495 | 0.003810 | 0.4950 | 99.00% | 1 | 10 | **+0.0656** | **+1.43%** | 0.92 | 0.51% | 2.81 | 3.003 | 0.2337 | `InsufficientSample` | 0.0167 | `InsufficientSample` |
| `Lev2x_Fast7d_Top4` | 4362 | `—` (`—`) | +0.0303 | +0.3385 | 0.0287 | 0.4287 | 0.032974 | 0.8439 | 84.39% | 19 | 25 | -0.0886 | -1.95% | -0.75 | 4.30% | -0.45 | -2.129 | 0.0010 | `Fail` | 0.0000 | `Fail` |
| `Lev2x_Mid14d_Top4` | 4338 | `—` (`—`) | +0.0127 | +0.3102 | 0.0473 | 0.2867 | 0.022053 | 0.9979 | 99.79% | 40 | 41 | -0.0110 | -0.27% | -0.09 | 2.42% | -0.11 | -0.280 | 0.0150 | `Fail` | 0.0002 | `Fail` |
| `Lev2x_Slow30d_Top2` | 4200 | 996,246 (1,253,810) | -0.0069 | +0.2609 | 0.0476 | 0.1826 | 0.014048 | 0.9900 | 99.00% | 28 | 40 | +0.0237 | +0.47% | 0.16 | 1.74% | 0.27 | 0.503 | 0.0378 | `InsufficientSample` | 0.0007 | `InsufficientSample` |
| `Lev3x_Fast7d_Top4` | 4362 | `—` (`—`) | +0.0454 | +0.5077 | 0.0722 | 0.8345 | 0.064191 | 1.2658 | 84.39% | 89 | 63 | -0.3536 | -7.52% | -1.95 | 14.80% | -0.51 | -5.280 | 0.0000 | `Fail` | 0.0000 | `Fail` |
| `Lev3x_Mid14d_Top4` | 4338 | `—` (`—`) | +0.0191 | +0.4654 | 0.0888 | 0.6608 | 0.050830 | 1.4969 | 99.79% | 137 | 77 | -0.2651 | -5.71% | -1.48 | 11.58% | -0.49 | -3.927 | 0.0000 | `Fail` | 0.0000 | `Fail` |
| `Lev3x_Slow30d_Top2` | 4200 | `—` (`—`) | -0.0104 | +0.3914 | 0.0976 | 0.4921 | 0.037857 | 1.4850 | 99.00% | 75 | 82 | -0.2088 | -4.58% | -0.94 | 9.67% | -0.47 | -2.611 | 0.0005 | `Fail` | 0.0000 | `Fail` |

*(Nota: `Util% = avg_gross_l / target_l * 100` con `avg_gross_l` mostrado explícitamente junto a `TurnL`.)*

### 3.2 Diagnóstico No Vinculante `XVenue-MakerDiag` (`3.50 bps/turn_L`, `0 bps` slippage, `is_binding = false`)

| Strategy | NetMaker (bps/4h) | CAGR% | AnnSR | MaxDD% | `t_NW` | DSR | Verdict (`is_binding=false`) |
|---|---:|---:|---:|---:|---:|---:|---|
| `Lev1x_Fast7d_Top4` | +0.1230 | +2.72% | 2.19 | 0.50% | 6.915 | 0.9781 | `InsufficientSample` |
| `Lev1x_Mid14d_Top4` | +0.1231 | +2.72% | 2.19 | 0.47% | 7.165 | 0.9774 | `InsufficientSample` |
| `Lev1x_Slow30d_Top2` | +0.1017 | +2.24% | 1.45 | 0.39% | 4.983 | 0.8278 | `InsufficientSample` |
| `Lev2x_Fast7d_Top4` | +0.2247 | +5.01% | 2.00 | 0.99% | 6.385 | 0.9601 | `InsufficientSample` |
| `Lev2x_Mid14d_Top4` | +0.1985 | +4.41% | 1.78 | 0.94% | 5.839 | 0.9217 | `InsufficientSample` |
| `Lev2x_Slow30d_Top2` | +0.1572 | +3.45% | 1.12 | 0.78% | 3.833 | 0.6855 | `InsufficientSample` |
| `Lev3x_Fast7d_Top4` | **+0.2562** | **+5.70%** | 1.51 | 1.49% | 4.873 | 0.8556 | `InsufficientSample` |
| `Lev3x_Mid14d_Top4` | +0.2178 | +4.81% | 1.29 | 1.43% | 4.213 | 0.7704 | `InsufficientSample` |
| `Lev3x_Slow30d_Top2` | +0.1508 | +3.25% | 0.71 | 1.71% | 2.414 | 0.4689 | `InsufficientSample` |

> [!IMPORTANT]
> Ni siquiera suponiendo ejecución **100% pasiva simultánea sin slippage** en ambos exchanges (`XVenue-MakerDiag`, `7.00 bps` RT en lugar de `26.00 bps` RT), **ninguna (`0 / 9`)** configuración alcanza la compuerta económica `CAGR >= 8.0%` en `24m` (techo `+5.70%` en `Lev3x_Fast7d_Top4`) ni logra `Pass` estadístico (`N < n_min` y bloqueo estructural `is_binding = false`).

---

## 4. Particiones OOS (`Y1` y `Y2`) y Verificación Explícita del Kill-Gate Económico

### 4.1 Año 1 (`Y1`: `2024-11-25` a `2025-11-25`, `2,190` slices, `sr_std(VIP0) = 0.021542`)

Fuente: [`oos_y1_cap2000_s1p5_b1.json`](../data_local/reports_track7/oos_y1_cap2000_s1p5_b1.json).

| Strategy | N | Basis | FundSpd | Bridge | CostVIP0 | Rec/Brdg | Util% | NetVIP0 | CAGR% | AnnSR | MaxDD% | `t_NW` | DSR | Verdict | Cum116 (`DSR116`) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| `Lev1x_Fast7d_Top4` | 2172 | +0.0172 | +0.2152 | 0.0092 | 0.2434 | 1 / 4 | 91.44% | -0.0203 | -0.45% | -0.31 | 1.16% | -0.606 | 0.0337 | `Fail` | `Fail` (`0.0019`) |
| `Lev1x_Mid14d_Top4` | 2148 | +0.0116 | +0.1744 | 0.0023 | 0.1377 | 3 / 1 | 100.00% | +0.0460 | +1.00% | 0.70 | 0.74% | 1.434 | 0.2046 | `InsufficientSample` | `InsufficientSample` (`0.0295`) |
| `Lev1x_Slow30d_Top2` | 2010 | -0.0026 | +0.1324 | 0.0124 | 0.0647 | 1 / 5 | 100.00% | **+0.0528** | **+1.15%** | 0.71 | 0.51% | 1.431 | 0.2145 | `InsufficientSample` | `InsufficientSample` (`0.0347`) |
| `Lev2x_Fast7d_Top4` | 2172 | +0.0343 | +0.4304 | 0.0391 | 0.5287 | 14 / 17 | 91.44% | -0.1031 | -2.27% | -0.80 | 3.12% | -1.539 | 0.0110 | `Fail` | `Fail` (`0.0004`) |
| `Lev2x_Mid14d_Top4` | 2148 | +0.0233 | +0.3487 | 0.0582 | 0.3409 | 24 / 25 | 100.00% | -0.0271 | -0.63% | -0.21 | 2.42% | -0.412 | 0.0427 | `Fail` | `Fail` (`0.0027`) |
| `Lev2x_Slow30d_Top2` | 2010 | -0.0051 | +0.2649 | 0.0597 | 0.2264 | 16 / 24 | 100.00% | -0.0263 | -0.63% | -0.17 | 1.74% | -0.336 | 0.0512 | `Fail` | `Fail` (`0.0039`) |
| `Lev3x_Fast7d_Top4` | 2172 | +0.0515 | +0.6456 | 0.0806 | 1.0010 | 54 / 35 | 91.44% | -0.3846 | -8.16% | -1.93 | 8.50% | -3.671 | 0.0004 | `Fail` | `Fail` (`0.0000`) |
| `Lev3x_Mid14d_Top4` | 2148 | +0.0349 | +0.5231 | 0.1210 | 0.8080 | 86 / 52 | 100.00% | -0.3710 | -7.90% | -1.81 | 8.35% | -3.314 | 0.0005 | `Fail` | `Fail` (`0.0000`) |
| `Lev3x_Slow30d_Top2` | 2010 | -0.0077 | +0.3973 | 0.1219 | 0.6500 | 48 / 49 | 100.00% | -0.3823 | -8.16% | -1.60 | 8.21% | -2.839 | 0.0013 | `Fail` | `Fail` (`0.0000`) |

### 4.2 Año 2 (`Y2`: `2025-11-25` a `2026-11-25`, `2,190` slices, `sr_std(VIP0) = 0.020573`)

Fuente: [`oos_y2_cap2000_s1p5_b1.json`](../data_local/reports_track7/oos_y2_cap2000_s1p5_b1.json).

| Strategy | N | Basis | FundSpd | Bridge | CostVIP0 | Rec/Brdg | Util% | NetVIP0 | CAGR% | AnnSR | MaxDD% | `t_NW` | DSR | Verdict | Cum116 (`DSR116`) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| `Lev1x_Fast7d_Top4` | 2172 | +0.0126 | +0.1216 | 0.0069 | 0.1571 | 0 / 3 | 74.45% | -0.0298 | -0.66% | -0.56 | 0.94% | -1.108 | 0.0221 | `Fail` | `Fail` (`0.0012`) |
| `Lev1x_Mid14d_Top4` | 2148 | -0.0011 | +0.1167 | 0.0047 | 0.1120 | 3 / 2 | 91.20% | -0.0011 | -0.03% | -0.02 | 0.66% | -0.042 | 0.0706 | `Fail` | `Fail` (`0.0065`) |
| `Lev1x_Slow30d_Top2` | 2010 | -0.0025 | +0.1273 | 0.0124 | 0.0420 | 0 / 5 | 100.00% | **+0.0703** | **+1.54%** | 1.02 | 0.25% | 2.254 | 0.3362 | `InsufficientSample` | `InsufficientSample` (`0.0805`) |
| `Lev2x_Fast7d_Top4` | 2172 | +0.0252 | +0.2432 | 0.0184 | 0.3392 | 5 / 8 | 74.45% | -0.0892 | -1.96% | -0.83 | 2.16% | -1.641 | 0.0111 | `Fail` | `Fail` (`0.0005`) |
| `Lev2x_Mid14d_Top4` | 2148 | -0.0023 | +0.2334 | 0.0233 | 0.2613 | 14 / 10 | 91.20% | -0.0534 | -1.19% | -0.54 | 1.63% | -1.079 | 0.0235 | `Fail` | `Fail` (`0.0014`) |
| `Lev2x_Slow30d_Top2` | 2010 | -0.0050 | +0.2546 | 0.0398 | 0.1552 | 11 / 16 | 100.00% | +0.0545 | +1.15% | 0.39 | 0.64% | 0.832 | 0.1524 | `InsufficientSample` | `InsufficientSample` (`0.0225`) |
| `Lev3x_Fast7d_Top4` | 2172 | +0.0378 | +0.3648 | 0.0645 | 0.6928 | 35 / 28 | 74.45% | -0.3547 | -7.53% | -2.16 | 7.71% | -3.875 | 0.0001 | `Fail` | `Fail` (`0.0000`) |
| `Lev3x_Mid14d_Top4` | 2148 | -0.0034 | +0.3501 | 0.0559 | 0.5341 | 41 / 24 | 91.20% | -0.2433 | -5.24% | -1.61 | 5.24% | -2.867 | 0.0011 | `Fail` | `Fail` (`0.0000`) |
| `Lev3x_Slow30d_Top2` | 2010 | -0.0076 | +0.3819 | 0.0846 | 0.3978 | 28 / 34 | 100.00% | -0.1080 | -2.44% | -0.51 | 2.55% | -1.002 | 0.0293 | `Fail` | `Fail` (`0.0020`) |

### 4.3 Verificación del Kill-Gate (`Cum116 == Pass` AND `CAGR_24m >= 8.0%` AND `CAGR_Y1 >= 6.0%` AND `CAGR_Y2 >= 6.0%`)

- **`0 / 9`** configuraciones cumplen el criterio estadístico (`Pass` en local `M = 9` o `Cum116` en `M = 116`): el máximo `t_NW` bajo costos vinculantes en `24m` es `3.003` (`Lev1x_Slow30d_Top2`, frente a `3.5000` local y `3.9265` acumulado), con `N = 4,200 < n_min = 31,491` (`n_min_cum116 = 39,633`, equivalente a `18.1` años de historial `4h`) y `DSR = 0.2337` (`DSR_116 = 0.0167 < 0.95`).
- **`0 / 9`** cumplen la compuerta económica en ninguno de los tres tramos: techo `24m = +1.43%` (requisito `>= 8.0%`), techo `Y1 = +1.15%` (requisito `>= 6.0%`), techo `Y2 = +1.54%` (requisito `>= 6.0%`). **Kill-Gate: `0 / 9 PASS` (`REJECT`)**.

---

## 5. Análisis de Sensibilidad y Anatomía del Fracaso

### 5.1 Sensibilidad al Capital (`$1,000` vs. `$2,000` USDT) en `24m`, `Y1` e `Y2`

Fuentes: `sens_{24m,y1,y2}_cap1000_s1p5_b1.json` vs. `{main_24m,oos_y1,oos_y2}_cap2000_s1p5_b1.json`. Reducir capital de `$2,000` a `$1,000` **duplica el costo en bps por puente Arbitrum** (`5.0 -> 10.0 bps` por evento):

| Strategy | `24m`: `$2k -> $1k` (`Net` / `BrdgBps` / `CAGR` / `t_NW` / `Cum116`) | `Y1`: `$2k -> $1k` (`CAGR` / `t_NW` / `Cum116`) | `Y2`: `$2k -> $1k` (`CAGR` / `t_NW` / `Cum116`) |
|---|---|---|---|
| `Lev1x_Fast7d_Top4` | `-0.0218 -> -0.0299` \| `0.0080->0.0160` \| `-0.49% -> -0.66%` \| `-1.060 -> -1.412` \| `F -> F` | `-0.45% -> -0.65%` \| `-0.606 -> -0.862` \| `F` | `-0.66% -> -0.81%` \| `-1.108 -> -1.326` \| `F` |
| `Lev1x_Mid14d_Top4` | `+0.0377 -> +0.0308` \| `0.0069->0.0138` \| `+0.82% -> +0.67%` \| `1.953 -> 1.531` \| `I -> I` | `+1.00% -> +0.95%` \| `1.434 -> 1.343` \| `I` | `-0.03% -> -0.13%` \| `-0.042 -> -0.222` \| `F` |
| `Lev1x_Slow30d_Top2` | `+0.0656 -> +0.0537` \| `0.0119->0.0238` \| **`+1.43% -> +1.17%`** \| `3.003 -> 2.374` \| `I -> I` | `+1.15% -> +0.87%` \| `1.431 -> 1.069` \| `I` | `+1.54% -> +1.26%` \| `2.254 -> 1.781` \| `I` |
| `Lev2x_Fast7d_Top4` | `-0.0886 -> -0.1172` \| `0.0287->0.0573` \| `-1.95% -> -2.57%` \| `-2.129 -> -2.746` \| `F -> F` | `-2.27% -> -3.11%` \| `-1.539 -> -2.075` \| `F` | `-1.96% -> -2.36%` \| `-1.641 -> -1.935` \| `F` |
| `Lev2x_Mid14d_Top4` | `-0.0110 -> -0.0582` \| `0.0473->0.0945` \| `-0.27% -> -1.30%` \| `-0.280 -> -1.396` \| `F -> F` | `-0.63% -> -1.89%` \| `-0.412 -> -1.229` \| `F` | `-1.19% -> -1.69%` \| `-1.079 -> -1.519` \| `F` |
| `Lev2x_Slow30d_Top2` | `+0.0237 -> -0.0239` \| `0.0476->0.0952` \| **`+0.47% -> -0.57%`** \| `0.503 -> -0.487` \| `I -> F` | `-0.63% -> -1.92%` \| `-0.336 -> -1.067` \| `F` | `+1.15% -> +0.28%` \| `0.832 -> 0.217` \| `I` |
| `Lev3x_Fast7d_Top4` | `-0.3536 -> -0.4258` \| `0.0722->0.1444` \| `-7.52% -> -8.98%` \| `-5.280 -> -6.128` \| `F -> F` | `-8.16% -> -9.77%` \| `-3.671 -> -4.315` \| `F` | `-7.53% -> -8.83%` \| `-3.875 -> -4.397` \| `F` |
| `Lev3x_Mid14d_Top4` | `-0.2651 -> -0.3538` \| `0.0888->0.1775` \| `-5.71% -> -7.53%` \| `-3.927 -> -4.932` \| `F -> F` | `-7.90% -> -10.31%` \| `-3.314 -> -4.148` \| `F` | `-5.24% -> -6.39%` \| `-2.867 -> -3.389` \| `F` |
| `Lev3x_Slow30d_Top2` | `-0.2088 -> -0.3064` \| `0.0976->0.1952` \| `-4.58% -> -6.60%` \| `-2.611 -> -3.617` \| `F -> F` | `-8.16% -> -10.58%` \| `-2.839 -> -3.551` \| `F` | `-2.44% -> -4.24%` \| `-1.002 -> -1.706` \| `F` |

### 5.2 Sensibilidad a Slippage (`0.5 / 1.5* / 3.0 bps`) y Peaje Puente (`$0 / $1* / $5`) en `24m` (`$2,000`)

Fuentes: `sens_24m_cap2000_{s0p5_b1, s3p0_b1, s1p5_b0, s1p5_b5}.json` y [`main_24m_cap2000_s1p5_b1.json`](../data_local/reports_track7/main_24m_cap2000_s1p5_b1.json).

| Strategy | Slip `0.5` (`11 bps`) CAGR / `t_NW` / Verd | Slip `1.5*` (`13 bps`) CAGR / `t_NW` / Verd | Slip `3.0` (`16 bps`) CAGR / `t_NW` / Verd | Bridge `$0` (`0 bps`) CAGR / `t_NW` / `DSR116` | Bridge `$5` (`25 bps`) CAGR / `t_NW` / Verd |
|---|---|---|---|---|---|
| `Lev1x_Fast7d_Top4` | +0.07% / 0.176 / `I` | -0.49% / -1.060 / `F` | -1.36% / -2.930 / `F` | -0.31% / -0.674 / `0.0003` (`F`) | -1.18% / -2.160 / `F` |
| `Lev1x_Mid14d_Top4` | +1.03% / 2.536 / `I` | +0.82% / 1.953 / `I` | +0.42% / 0.955 / `I` | +0.97% / 2.363 / `0.0202` (`I`) | +0.21% / 0.412 / `I` |
| `Lev1x_Slow30d_Top2` | **+1.61%** / 3.374 / `I` | **+1.43%** / 3.003 / `I` | +1.30% / 2.571 / `I` | **+1.70%** / 3.573 / `0.0574` (`I`) | +0.38% / 0.643 / `I` |
| `Lev2x_Fast7d_Top4` | -0.58% / -0.641 / `F` | -1.95% / -2.129 / `F` | -3.84% / -4.073 / `F` | -1.34% / -1.451 / `0.0001` (`F`) | -4.39% / -4.067 / `F` |
| `Lev2x_Mid14d_Top4` | +0.31% / 0.402 / `I` | -0.27% / -0.280 / `F` | -1.48% / -1.563 / `F` | +0.77% / 0.953 / `0.0035` (`I`) | -4.32% / -3.619 / `F` |
| `Lev2x_Slow30d_Top2` | +1.13% / 1.173 / `I` | +0.47% / 0.503 / `I` | -0.28% / -0.199 / `F` | +1.52% / 1.537 / `0.0082` (`I`) | -3.64% / -2.776 / `F` |
| `Lev3x_Fast7d_Top4` | -5.29% / -3.813 / `F` | -7.52% / -5.280 / `F` | -11.56% / -7.548 / `F` | -6.05% / -4.290 / `0.0000` (`F`) | -13.20% / -7.781 / `F` |
| `Lev3x_Mid14d_Top4` | -4.08% / -2.969 / `F` | -5.71% / -3.927 / `F` | -8.18% / -5.133 / `F` | -3.86% / -2.732 / `0.0000` (`F`) | -12.78% / -6.887 / `F` |
| `Lev3x_Slow30d_Top2` | -3.02% / -1.794 / `F` | -4.58% / -2.611 / `F` | -6.74% / -3.545 / `F` | -2.52% / -1.452 / `0.0001` (`F`) | -12.42% / -5.719 / `F` |

*(Leyenda: `I` = `InsufficientSample`, `F` = `Fail`; en todos los `10` escenarios del barrido los veredictos `M=9` y `Cum116` arrojan **`0 / 9 Pass`**.)*

### 5.3 Anatomía Causal del Fracaso & Viabilidad Operativa (`$1,000–$2,000 USDT`)

1. **Spread estrecho y altamente correlacionado entre venues vs. `26.00 bps` RT**: Ambas plataformas siguen al mismo mercado global de perpetuos (`BTC`..`INJ`); la brecha media de funding es de apenas **`+0.1939 bps/4h`** (`~4.25%` bruto sin apalancar) y comprimió **`-30.5%`** entre `Y1` (`+0.2288`) y `Y2` (`+0.1590`). Pagar **`26.00 bps`** round-trip (`5.50 + 4.50 + 3.0` por lado) requiere **`~134` barras (`22.3` días)** de spread medio tan solo para recuperar la comisión de entrada y salida.
2. **Explosión cuadrática de recentrado y puentes al apalancar (`Lev1x -> Lev2x -> Lev3x`)**: Estrechar el gatillo de recentrado bilateral de `60%` (`Lev1x`) a `30%` (`Lev2x`) y `18%` (`Lev3x`) multiplica los recentrados de `1–6` eventos (`Lev1x`) a `19–40` (`Lev2x`) y **`75–137` (`Lev3x`)**, además de disparar los puentes Arbitrum de `6–10` (`Lev1x`) a **`63–82` (`Lev3x`)**. En `Lev3x_Fast7d_Top4` (`$2,000`), `CostVIP0` (`0.8345 bps/4h`) + `Bridge` (`0.0722`) = **`0.9067 bps/4h`**, consumiendo el **`178.6%`** del spread bruto capturado (`+0.5077 bps/4h`).
3. **Resolución de tamaño mínimo y margen real con `$1,000–$2,000 USDT`**:
   - En `Lev1x_Slow30d_Top2` (`0.50` por pata, `2` pares -> `$250/pata` en `$1,000`, `$500/pata` en `$2,000`), todas las órdenes superan los mínimos de Bybit (`$5`) y Hyperliquid (`$10`). En **`[06] ORDI` (`maxLev = 3x` en HL)** o los pares de **`maxLev = 5x`** (`TAO`, `PENDLE`, `WIF`, `INJ`), asignar `Lev3x` (`$1,500/pata` con capital dividido `$1,000` por venue en cuenta de `$2,000`) ya trabaja al `1.5x` por activo (consumiendo `50%` del margen máximo en `ORDI`), dejando holgura crítica frente a mechas intrabarra y redondeo entero (`szDecimals = 0` en `PENDLE`, `WIF`, `XRP`, `DOGE`).
   - En términos económicos absolutos, la mejor configuración (`Lev1x_Slow30d_Top2`, **`+1.43%` CAGR**) produce apenas **`$28.60/año`** sobre `$2,000` (`$11.70/año` con `$1,000`), muy por debajo de tasas libres de riesgo USD (`~4%–5%`).

---

## 6. Catálogo de Tests + Salida Cruda Sin Editar de las 6 Compuertas PowerShell + `git status --short`

### 6.1 Catálogo Resumen de Pruebas de Integración ([`tests/xvenue_carry_tests.rs`](../tests/xvenue_carry_tests.rs), `5 / 5 ok`; `52 / 52` total)

| # | Test (`archivo:línea`) | Alcance y Vectores Verificados (`input -> expected -> actual`) | Estado |
|---:|---|---|---|
| 1 | `test_xvenue_bidirectional_basis_funding_and_sign_flip_turnover` ([`L166-400`](../tests/xvenue_carry_tests.rs#L166-L400)) | `XVenueFeeProfiles` (`13.00` vs `3.50 bps`), `payback_apr_hurdle_pct` (`7/14/30d`), bloqueo `min_hold` + reversión `+1 -> -1` (`1.00 = 2L` turnover cuando `-held_apr >= max(entry, hurdle)`), e idempotencia exacta en `basis` (`+150 / +250 / -100 bps`) y `fund` (`+1.5 / +1.5 / -1.0 bps`) en ambas direcciones. | **PASS** |
| 2 | `test_xvenue_split_margin_recenter_and_bridge_toll` ([`L402-655`](../tests/xvenue_carry_tests.rs#L402-L655)) | Recentrados bilaterales en umbrales exactos `60% / 30% / 18%` (`recenter_events = 1`, `recenter_turn_l = 2*w`), acumulación `byb_pnl` y disparo de puente Arbitrum en `|imb| >= 0.15` (`bridge_bps = 5.0` en `$2k` vs `10.0` en `$1k`, `imb -> 0.0`), con invariancia de `RecCnt = 2/2/3` y `BrdgCnt = 2`. | **PASS** |
| 3 | `test_xvenue_bar_open_settlement_eof_maker_diag_and_cum116` ([`L657-936`](../tests/xvenue_carry_tests.rs#L657-L936)) | `BarOpenSettlement` causal (funding `t+2`, `0.0` en EOF `r2`), liquidación EOF (`eof_close_turn_l = 0.80`), garantía doble de que `MakerDiag` (`is_binding = false`) jamás emite `GateVerdict::Pass` ni con `t_nw >= 3.5` y `DSR >= 0.95`, y separación `t_crit_cum116 = 3.9265` (`M=116`) donde una celda con `t_nw` en `(3.50, 3.9265)` aprueba local y reprueba `Cum116`. | **PASS** |
| 4 | `test_xvenue_zero_lookahead_bit_for_bit_and_late_hl_listing` ([`L938-1167`](../tests/xvenue_carry_tests.rs#L938-L1167)) | Corrupción `10×` precios / `-500 bps` funding tras `t > 80`: igualdad bit-a-bit (`to_bits()`) de los `XVenueBarRecord` previos al corte en las `9` estrategias; y exclusión causal de activo Hyperliquid con listado tardío hasta su primera vela real sin síntesis de precios. | **PASS** |
| 5 | `test_xvenue_cli_end_to_end_json_roundtrip_and_rejections` ([`L1169-1343`](../tests/xvenue_carry_tests.rs#L1169-L1343)) | Ejecución CLI `scan-xvenue` end-to-end, columnas ASCII (`FundSpd`, `Basis`, `Bridge`, `RecCnt`, `AvgUtil%`, `Cum116`), roundtrip `serde_json` (`m_cumulative = 116`, `slices = 216`, `candles = 864/864`, `9` celdas), y rechazo tipado ante `--capital-usdt <= 0`, `--bridge-fee-usdt < 0`, `NaN` o flags faltantes. | **PASS** |

### 6.2 Salida Cruda Sin Editar de las 6 Compuertas PowerShell + `git status` y `git diff --name-only`

> Salida literal sin editar capturada en PowerShell 5.1 (`D:\Desarrollo\VORTEX-ALFA`, tras `cargo fmt --all`, verificación física `[System.IO.File]::ReadAllLines` de `50/50` archivos `src/` y suite de `52/52` tests). Al final se anexa el `git status --short` post-documentación confirmando cambios exclusivamente en `DOCS/`.

```text
5.1.26100.9818
=== GATE 1: cargo fmt ===
FMT_EXIT=0
=== GATE 2: cargo clippy ===
    Checking vortex-alfa v0.1.0 (D:\Desarrollo\VORTEX-ALFA)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.99s
CLIPPY_EXIT=0
=== GATE 3: cargo test ===
   Compiling vortex-alfa v0.1.0 (D:\Desarrollo\VORTEX-ALFA)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 15.71s
     Running unittests src\lib.rs (target\debug\deps\vortex_alfa-a216a8d1bd8b90ff.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\vortex_alfa-e53f0a43f8dc11a2.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\adversarial_math_tests.rs (target\debug\deps\adversarial_math_tests-1e7b3596f2b1b423.exe)

running 9 tests
test test_moments4_closed_form_exact ... ok
test test_acklam_normal_inv_cdf_accuracy ... ok
test test_garman_klass_closed_form_and_flat_bar ... ok
test test_hawkes_o1_three_step_analytical ... ok
test test_holm_bonferroni_order_and_monotonicity ... ok
test test_newey_west_closed_form_n8 ... ok
test test_dsr_cross_sectional_std_and_diag_never_passes ... ok
test test_vip0_ceil_fee_e8_and_book_roundtrip_asymmetry ... ok
test test_exact_funding_boundary_bar_open_vs_bar_close ... ok

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\carry_uta_tests.rs (target\debug\deps\carry_uta_tests-9b3f8b32f1f47eca.exe)

running 6 tests
test uta_cross_margin_borrow_and_recenter_physics ... ok
test payback_apr_hurdle_and_cost_amortized_rotation ... ok
test real_basis_bar_open_settlement_eof_and_maker_diag_non_binding ... ok
test cash50_blind30d_replicates_phase4_topcarry30d ... ok
test zero_lookahead_bit_exact_and_late_spot_listing ... ok
test cli_scan_carry_uta_end_to_end_and_rejections ... ok

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

     Running tests\cross_sectional_tests.rs (target\debug\deps\cross_sectional_tests-a086dcf4759e346e.exe)

running 7 tests
test test_xs_momentum_vol_adj_skip_and_gk_analytical ... ok
test test_xs_edge_cases_zero_var_flat_bar_ties_insufficient_symbols_and_non_finite ... ok
test test_xs_residual_reversion_ols_beta_and_clamp_analytical ... ok
test test_xs_funding_carry_closed_form_and_ranking ... ok
test test_exact_turnover_funding_bar_open_eof_and_maker_never_passes ... ok
test test_zero_lookahead_end_to_end_track3_xs ... ok
test test_cli_scan_xs_csv_and_global27_report ... ok

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s

     Running tests\edge_cases_and_drift_tests.rs (target\debug\deps\edge_cases_and_drift_tests-2a9b90ac347a6026.exe)

running 8 tests
test test_dust_notional_crossed_book_and_invalid_ohlc_rejected ... ok
test test_flat_candle_garman_klass_zero ... ok
test test_zero_variance_never_passes_or_nans ... ok
test test_flash_crash_minus_30_percent_bar_and_tick ... ok
test test_equity_ledger_10000_trades_zero_drift ... ok
test test_duplicate_and_backward_timestamps_rejected ... ok
test test_corrupted_binary_formats_rejected ... ok
test test_reject_non_finite_across_all_components ... ok

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests\macro_track5_tests.rs (target\debug\deps\macro_track5_tests-ad877cc04813ebaa.exe)

running 6 tests
test test_equity_metrics_and_passive_baselines_closed_form ... ok
test test_macro_trend_and_donchian_closed_form_vectors ... ok
test test_spot_perp_carry_real_basis_vip0_fees_and_eof ... ok
test test_edge_cases_late_spot_flat_candles_and_invalid_inputs ... ok
test test_zero_lookahead_bit_for_bit_and_symbol_isolation ... ok
test test_cli_scan_macro_end_to_end_and_json_roundtrip ... ok

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s

     Running tests\market_making_tests.rs (target\debug\deps\market_making_tests-c773f527f66d75db.exe)

running 6 tests
test test_mm_fifo_queue_conservation_and_5ms_adverse_latency ... ok
test test_mm_stoikov_ofi_tfi_as_and_postonly_analytical ... ok
test test_mm_markout_lvr_eof_liquidation_breakeven_and_zero_drift ... ok
test test_mm_diagnostic_frontier_never_passes_binding_gate ... ok
test test_mm_zero_lookahead_and_strict_symbol_isolation ... ok
test test_cli_scan_mm_multifile_par2_and_json_report ... ok

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests\xvenue_carry_tests.rs (target\debug\deps\xvenue_carry_tests-70b21d8dcfcd6210.exe)

running 5 tests
test test_xvenue_bidirectional_basis_funding_and_sign_flip_turnover ... ok
test test_xvenue_split_margin_recenter_and_bridge_toll ... ok
test test_xvenue_bar_open_settlement_eof_maker_diag_and_cum116 ... ok
test test_xvenue_cli_end_to_end_json_roundtrip_and_rejections ... ok
test test_xvenue_zero_lookahead_bit_for_bit_and_late_hl_listing ... ok

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s

     Running tests\zero_lookahead_tests.rs (target\debug\deps\zero_lookahead_tests-68c127dd0d00bc59.exe)

running 5 tests
test test_zero_lookahead_end_to_end_track2_meso ... ok
test test_zero_lookahead_end_to_end_track1_micro ... ok
test test_strict_symbol_isolation_track1_micro ... ok
test test_zero_lookahead_end_to_end_track2_meso ... ok
test test_multifile_par2_vpz1_vkl1_streaming_and_cli ... ok

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

TEST_EXIT=0
=== GATE 4: line count <= 350 ===

File                                                                 Lines Pass
----                                                                 ----- ----
D:\Desarrollo\VORTEX-ALFA\src\application\carry_uta_accounting.rs      273 True
D:\Desarrollo\VORTEX-ALFA\src\application\carry_uta_types.rs           190 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_carry_uta.rs       212 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_macro.rs           259 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_meso.rs            330 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_micro.rs           280 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_mm.rs              337 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_xs.rs              334 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_xvenue_carry.rs    250 True
D:\Desarrollo\VORTEX-ALFA\src\application\macro_accounting.rs          281 True
D:\Desarrollo\VORTEX-ALFA\src\application\macro_baselines.rs           345 True
D:\Desarrollo\VORTEX-ALFA\src\application\mm_accounting.rs             345 True
D:\Desarrollo\VORTEX-ALFA\src\application\mm_fifo_sim.rs               346 True
D:\Desarrollo\VORTEX-ALFA\src\application\mm_markout.rs                158 True
D:\Desarrollo\VORTEX-ALFA\src\application\mod.rs                        16 True
D:\Desarrollo\VORTEX-ALFA\src\application\xvenue_carry_accounting.rs   261 True
D:\Desarrollo\VORTEX-ALFA\src\application\xvenue_carry_types.rs        208 True
D:\Desarrollo\VORTEX-ALFA\src\domain\error.rs                           45 True
D:\Desarrollo\VORTEX-ALFA\src\domain\fees.rs                           179 True
D:\Desarrollo\VORTEX-ALFA\src\domain\ledger.rs                         113 True
D:\Desarrollo\VORTEX-ALFA\src\domain\mod.rs                              8 True
D:\Desarrollo\VORTEX-ALFA\src\domain\normal_dist.rs                    217 True
D:\Desarrollo\VORTEX-ALFA\src\domain\ports.rs                           18 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\carry_uta.rs              260 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\cross_sectional.rs        306 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\macro_donchian.rs         192 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\macro_trend.rs            296 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\macro_vol.rs              151 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\market_making.rs          310 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\meso_energy.rs            331 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\micro_detectors.rs        226 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\micro_tail.rs             330 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\mod.rs                     11 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\spot_perp_carry.rs        241 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\xvenue_carry.rs           259 True
D:\Desarrollo\VORTEX-ALFA\src\domain\stats.rs                          348 True
D:\Desarrollo\VORTEX-ALFA\src\domain\types.rs                          331 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\binary_codec.rs           342 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\csv_candle_reader.rs      319 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\mod.rs                      4 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\tape_reader.rs            325 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\vkl1_reader.rs            154 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli.rs                        330 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli_carry_uta.rs              128 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli_macro.rs                  158 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli_mm.rs                     136 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli_xvenue_carry.rs           138 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\mod.rs                          5 True
D:\Desarrollo\VORTEX-ALFA\src\lib.rs                                     7 True
D:\Desarrollo\VORTEX-ALFA\src\main.rs                                   17 True


=== GATE 5: banned patterns ===
BANNED_RECURSIVE_COUNT=0
=== GATE 6: encoding hygiene ===

File                          UTF8_NoBOM LF_Only
----                          ---------- -------
carry_uta_accounting.rs             True    True
carry_uta_types.rs                  True    True
falsifier_carry_uta.rs              True    True
falsifier_macro.rs                  True    True
falsifier_meso.rs                   True    True
falsifier_micro.rs                  True    True
falsifier_mm.rs                     True    True
falsifier_xs.rs                     True    True
falsifier_xvenue_carry.rs           True    True
macro_accounting.rs                 True    True
macro_baselines.rs                  True    True
mm_accounting.rs                    True    True
mm_fifo_sim.rs                      True    True
mm_markout.rs                       True    True
mod.rs                              True    True
xvenue_carry_accounting.rs          True    True
xvenue_carry_types.rs               True    True
error.rs                            True    True
fees.rs                             True    True
ledger.rs                           True    True
mod.rs                              True    True
normal_dist.rs                      True    True
ports.rs                            True    True
carry_uta.rs                        True    True
cross_sectional.rs                  True    True
macro_donchian.rs                   True    True
macro_trend.rs                      True    True
macro_vol.rs                        True    True
market_making.rs                    True    True
meso_energy.rs                      True    True
micro_detectors.rs                  True    True
micro_tail.rs                       True    True
mod.rs                              True    True
spot_perp_carry.rs                  True    True
xvenue_carry.rs                     True    True
stats.rs                            True    True
types.rs                            True    True
binary_codec.rs                     True    True
csv_candle_reader.rs                True    True
mod.rs                              True    True
tape_reader.rs                      True    True
vkl1_reader.rs                      True    True
cli.rs                              True    True
cli_carry_uta.rs                    True    True
cli_macro.rs                        True    True
cli_mm.rs                           True    True
cli_xvenue_carry.rs                 True    True
mod.rs                              True    True
lib.rs                              True    True
main.rs                             True    True
adversarial_math_tests.rs           True    True
carry_uta_tests.rs                  True    True
cross_sectional_tests.rs            True    True
edge_cases_and_drift_tests.rs       True    True
macro_track5_tests.rs               True    True
market_making_tests.rs              True    True
xvenue_carry_tests.rs               True    True
zero_lookahead_tests.rs             True    True


=== GIT STATUS & DIFF ===
 M src/application/mod.rs
 M src/domain/signals/mod.rs
 M src/interfaces/cli.rs
 M src/interfaces/mod.rs
?? scripts/audit_xvenue_datasets.py
?? scripts/fetch_bybit_hl_xvenue_24m.py
?? scripts/run_track7_sweep.ps1
?? src/application/falsifier_xvenue_carry.rs
?? src/application/xvenue_carry_accounting.rs
?? src/application/xvenue_carry_types.rs
?? src/domain/signals/xvenue_carry.rs
?? src/interfaces/cli_xvenue_carry.rs
?? tests/xvenue_carry_tests.rs
--- git diff --name-only ---
src/application/mod.rs
src/domain/signals/mod.rs
src/interfaces/cli.rs
src/interfaces/mod.rs
```

*(Nota previa al cierre: tras redactar [`DOCS/PHASE_6_TRACK7_XVENUE_BYBIT_HL_REPORT.md`](./PHASE_6_TRACK7_XVENUE_BYBIT_HL_REPORT.md) y actualizar [`DOCS/INDEX.md`](./INDEX.md), se re-verificó `git status --short` confirmando exclusivamente ` M DOCS/INDEX.md`, `?? DOCS/PHASE_6_TRACK7_XVENUE_BYBIT_HL_REPORT.md` y exclusión limpia de `data_local/`.)*

---

## 7. Veredicto Frío Binario de Fase 6 y Balance Acumulado `M_cum = 116`

- **Veredicto Binario de Despliegue**: **NO — DESCARTADO PARA DESPLIEGUE REAL EN CAPITAL PEQUEÑO (`$1,000–$2,000 USDT`)**.
- **Balance Acumulado VORTEX-ALFA (`M_cum = 116` hipótesis probadas en datos reales `2021–2026`)**:
  - **Track 1 (`scan-micro`, `20.52M` eventos L1)**: `0 / 20 Pass` (falsado).
  - **Track 2 (`scan-meso`, `15m/1h/4h`, `12` pares, `24m`)**: `0 / 36 Pass` (falsado).
  - **Track 3 (`scan-xs`, `4h`, `30` pares, `24m`)**: `0 / 27 Pass` (falsado).
  - **Track 4 (`scan-mm`, `20.52M` eventos L1)**: `0 / 6 Pass` (`0 / 30` frontera) (falsado).
  - **Track 5 (`scan-macro`, `1d`, `12` pares, `5y`)**: `1 / 9 Pass` (`Carry_SpotPerp_TopCarry30d`, `CAGR = 3.00%` en `5y`, `0 / 9` en `R2`).
  - **Track 6 (`scan-carry-uta`, `1d`, `30` pares, `5y`)**: `9 / 9 Pass` en `5y` / `5 / 9 Pass` en `R2` (máx `Pass` `3.81%` CAGR) / `0 / 9 Pass` en `24m` (máx `2.53%`), **`0 / 9 >= 6.0%`** -> **NO / Descartado**.
  - **Track 7 (`scan-xvenue`, `4h`, `16` pares Bybit vs. HL, `24m`)**: **`0 / 9 Pass`** local, **`0 / 9 Pass`** acumulado (`t_crit_cum116 = 3.9265`), **`0 / 9`** bajo Kill-Gate económico (`>= 8.0%` en `24m`, `>= 6.0%` en `Y1`/`Y2`; máx `1.43%` CAGR a `$2k` y `1.17%` a `$1k`), e incluso **`0 / 9`** bajo `MakerDiag` (`<= 5.70%` CAGR) -> **NO / Descartado**.

# VORTEX-ALFA — Phase 5 Track 6 Carry UTA (`1d`, `30` Pares) Empirical Falsification Report

> **Veredicto Empírico Global**: **`9 / 9 Pass`** estadístico en el barrido principal de `5` años sobre `30` pares (`VIP-0 + 1.5 bps` slip, `borrow = 15.0%`, `M = 9`, `t_crit = 3.5000`; `CAGR = 2.90%–6.12%`, `t_NW = 4.733–10.177`, `DSR = 0.9710–1.0000`), pero **`NO — DESCARTADO PARA DESPLIEGUE REAL EN CAPITAL PEQUEÑO ($1,000–$2,000 USDT)`** al exigir la barrera económica de régimen reciente (**`>= 6.0% CAGR` en `R2` y `24m`**): en **`R2`** (`2024-05-25..2026-11-25`, **`5 / 9 Pass`**), la rotación amortizada rescata la significancia estadística frente a `Blind30d` (`0 / 3 Pass`), pero el máximo `CAGR%` entre celdas `Pass` es **`3.81%`** (`Uta75_Amort30d_Top2`; `Uta85_Amort30d_Top4` `3.41%`; `Uta85_Amort30d_Top2` `4.05%` `Fail` con `DSR = 0.9227`), y en **`24m`** (`2024-11-25..2026-11-25`, **`0 / 9 Pass`**, todos `InsufficientSample`) el máximo `CAGR%` cae a **`2.53%`** (`Uta85_Amort30d_Top2`), por debajo de tasas USD libres de riesgo (`~4%–5%`) y lejos de `>= 6.0%`.
> **Engine**: `target\release\vortex-alfa.exe scan-carry-uta` ([`src/application/falsifier_carry_uta.rs`](../src/application/falsifier_carry_uta.rs), [`src/interfaces/cli_carry_uta.rs`](../src/interfaces/cli_carry_uta.rs))
> **Universo & Timeframe**: `30` pares Bybit V5 (`0:BTCUSDT .. 29:ICPUSDT`), velas cerradas **`1d`** (`interval = D`): **`29` pares duales `linear` + `spot`** + **`[04] TAOUSDT` perp-only** (`0` barras `spot`, excluido de Carry por Ley 4); `48,950` velas `linear` + `47,247` velas `spot` = `96,197` eventos únicos en `1,826` slices diarios.
> **Ventana Histórica UTC**: `2021-11-25T00:00:00Z` (`1637798400000`) a `2026-11-25T00:00:00Z` (`1795564800000`), **`5` años exactos** (`1,826` barras esperadas por serie; última apertura `2026-11-24T00:00:00Z`).
> **Particiones OOS**: **`R1`** `2021-11-25T00:00:00Z`–`2024-05-25T00:00:00Z` (`912` días) | **`R2`** `2024-05-25T00:00:00Z`–`2026-11-25T00:00:00Z` (`914` días) | **Cohorte Completa `24m`** `2024-11-25T00:00:00Z`–`2026-11-25T00:00:00Z` (`730` días, `29 / 29` duales al `100.0%`).
> **Fricción Vinculante (`VIP-0 + 1.5 bps` slip + UTA Borrow `15.0% APR`) vs. `VIP0-MakerDiag`**: Dual Spot–Perp Carry: `(10.00 + 1.50) + (5.50 + 1.50) = 18.50 bps`/`turn_L` (`MakerDiag: 10.00 + 2.00 = 12.00 bps`/`turn_L`, `is_binding = false`); interés de préstamo UTA `15.0% APR` (`1,500 / 365 = 4.109589 bps/día` sobre `borrowed_usdt_frac`).
> **Compuertas de Código**: `6 / 6` compuertas PS en verde (`47/47` tests `ok`: `41` previos + `6` en [`tests/carry_uta_tests.rs`](../tests/carry_uta_tests.rs); `45/45` archivos `src/**/*.rs <= 350` líneas, máximo `348` en `stats.rs`; `0` warnings Clippy; `0` patrones prohibidos; `52/52` `UTF-8 without BOM` + `LF-only`).
> **Index**: [`DOCS/INDEX.md`](./INDEX.md) | [`DOCS/PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md) | [`DOCS/PHASE_1A_TRACK2_FALSIFICATION_REPORT.md`](./PHASE_1A_TRACK2_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_1B_TRACK1_FALSIFICATION_REPORT.md`](./PHASE_1B_TRACK1_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md`](./PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_3A_TRACK4_MM_DESIGN_REPORT.md`](./PHASE_3A_TRACK4_MM_DESIGN_REPORT.md) | [`DOCS/PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md`](./PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_4_TRACK5_MACRO_FALSIFICATION_REPORT.md`](./PHASE_4_TRACK5_MACRO_FALSIFICATION_REPORT.md)

---

## 0. Conformidad con las 4 Leyes Inviolables ("Las 4 D") en Fase 5

| Ley | Regla | Evidencia Verificada en Código Fuente y Auditoría | Estado |
|---|---|---|:---:|
| **Ley 1 — Cero Duplicación (SSOT / DRY)** | Reutilizar primitivas del dominio (`types`, `fees`, `stats`), contabilidad macro (`MacroFeeProfiles`, `compute_equity_metrics`, `weight_turnover`) y lector `CsvCandleReader`; cero cálculos estadísticos o financieros en Python/PS | [`carry_uta_accounting.rs:6-9`](../src/application/carry_uta_accounting.rs#L6-L9) reutiliza `CostPerTurnover`, `compute_equity_metrics` ([`macro_baselines.rs:132-167`](../src/application/macro_baselines.rs#L132-L167)), `weight_turnover` ([`macro_baselines.rs:334-345`](../src/application/macro_baselines.rs#L334-L345)) y `MacroDailySlice` (`L140, 270-271`), además de [`welford_mean`](../src/domain/stats.rs#L28-L40) (`L261-269`); [`falsifier_carry_uta.rs:60-61, 125-126`](../src/application/falsifier_carry_uta.rs#L60-L126) reutiliza [`MacroFeeProfiles::new`](../src/application/macro_accounting.rs#L30-L69) y [`evaluate_hypothesis_batch`](../src/domain/stats.rs#L277-L348); [`cli_carry_uta.rs:8-9, 53-57`](../src/interfaces/cli_carry_uta.rs#L8-L57) reutiliza [`CsvCandleReader`](../src/infrastructure/csv_candle_reader.rs) y `fmt_opt`, `take_arg`, `write_json_pretty` ([`cli.rs:20-28, 69-78, 324-329`](../src/interfaces/cli.rs#L20-L28)). [`fetch_bybit_carry30_5y.py`](../scripts/fetch_bybit_carry30_5y.py), [`audit_carry30_datasets.py`](../scripts/audit_carry30_datasets.py) y [`run_track6_sweep.ps1`](../scripts/run_track6_sweep.ps1) no calculan señales, rotación, fees, interés UTA, Newey-West ni DSR. | **PASS** |
| **Ley 2 — Cero Desviación (Strict Scope Lock)** | Prohibido modificar Tracks 1–5, `stats.rs`, `fees.rs`, `ledger.rs`, `types.rs`, `Cargo.toml` o los `41` tests previos | `git diff --name-only` confirma únicamente los `4` archivos de registro mínimo: [`src/application/mod.rs`](../src/application/mod.rs) (`L1-3`), [`src/domain/signals/mod.rs`](../src/domain/signals/mod.rs) (`L1`), [`src/interfaces/cli.rs`](../src/interfaces/cli.rs) (`scan-carry-uta` en [`L55, L65`](../src/interfaces/cli.rs#L55-L65)) y [`src/interfaces/mod.rs`](../src/interfaces/mod.rs) (`L2`). Track 6 agrega `5` módulos nuevos en `src/` y [`tests/carry_uta_tests.rs`](../tests/carry_uta_tests.rs) (`6` tests nuevos; `47/47` tests totales `ok`). | **PASS** |
| **Ley 3 — Cero Diluvio (Zero Bloat `<= 350` líneas)** | Todos los `src/**/*.rs <= 350` líneas, [`Cargo.toml`](../Cargo.toml) con `3` deps y scripts Python en stdlib puro | Los `45/45` archivos en `src/**/*.rs` cumplen `<= 350` líneas (máximo global `348` en [`stats.rs`](../src/domain/stats.rs)): `carry_uta.rs` (`260`), `carry_uta_types.rs` (`190`), `carry_uta_accounting.rs` (`273`), `falsifier_carry_uta.rs` (`212`), `cli_carry_uta.rs` (`128`), `cli.rs` (`329`). [`Cargo.toml`](../Cargo.toml) conserva exactamente `3` dependencias (`thiserror`, `serde`, `serde_json`); scripts Python en stdlib puro (`0` `pip install`). | **PASS** |
| **Ley 4 — Cero Disfraz (Zero Disguise)** | Costos duales reales Spot (`10.00 bps`) + Perp (`5.50 bps`) + slippage, basis real Spot vs. Perp, fechas reales sin síntesis de precios ni padding y reporte íntegro (`15` escenarios) | [`CarryUtaEngine::on_slice`](../src/domain/signals/carry_uta.rs#L151-L155) y [`open_carry_uta_day`](../src/application/carry_uta_accounting.rs#L93-L96) excluyen `[04] TAOUSDT` por ausencia real de `spot` (`0` filas `header-only`, `0` precios sintéticos); [`fetch_bybit_carry30_5y.py:594-638`](../scripts/fetch_bybit_carry30_5y.py#L594-L638) filtra `17` filas pre-listado de `UNIUSDT`; `VIP0-MakerDiag` (`12.00 bps/turn_L`, `is_binding = false` vía `fee_profiles.perp_maker_diag` en [`falsifier_carry_uta.rs:126`](../src/application/falsifier_carry_uta.rs#L126)) jamás emite `Pass`; se reportan íntegramente las `9` celdas en todos los `15` escenarios (`5y`, `R1`, `R2`, `24m`, `ctrl_12p`, `sens_0/25%_borrow`, `sens_0.5/3.0_slip`), exponiendo el fracaso `0 / 9 Pass` en `24m` y el incumplimiento `0 / 9` de `>= 6.0% CAGR` en `R2`/`24m`. | **PASS** |

---

## 1. Manifiesto de los 30 Pares (`5` Años `1d`: `2021-11-25` a `2026-11-25`) y Auditoría de Integridad

Descarga ejecutada vía [`scripts/fetch_bybit_carry30_5y.py`](../scripts/fetch_bybit_carry30_5y.py) (`elapsed_sec = 113.3` s, [`download_manifest.json`](../data_local/bybit_carry30_5y/download_manifest.json)) y auditada con precisión `Decimal` por [`scripts/audit_carry30_datasets.py`](../scripts/audit_carry30_datasets.py) ([`audit_report.json`](../data_local/bybit_carry30_5y/audit_report.json)). De los `30` símbolos, `17` completaron `1,826` barras duales desde `2021-11-25`, `12` iniciaron posteriormente según su listado real en Bybit (`SUI`, `ORDI`, `PENDLE`, `WIF`, `NEAR`, `BNB`, `UNI`, `ARB`, `OP`, `APT`, `INJ`, `TIA`, `SEI`, `ETC`: `1` con inicio tardío en `linear` solo — `UNIUSDT` `2021-11-30` — y `11` con listados tardíos en `linear` y/o `spot`), y **`[04] TAOUSDT`** carece de mercado Spot en Bybit V5 (`spot_listed_on_bybit = false`, `0` velas `spot`, `1,030` velas `linear`). En la ventana reciente **`24m`** (`2024-11-25` a `2026-11-25`), los **`29 / 29`** pares duales operaron al **`100.0%`** (`730 / 730` barras cada uno).

| ID | Símbolo | `Linear` (Min..Max UTC · N · Cob% · Gaps) | `Spot` (Min..Max UTC · N · Cob% · LeadMiss · Gaps) | Dual Overlap (`Full / R1 / R2 / 24m`) | Intervalos Funding | Raw Events | Sub-24h Agg. | Raw Sum (bps) | CSV Sum (bps) | `diff_bps` |
|---:|---|---|---|---|---|---:|---:|---:|---:|---|
| 0 | `BTCUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 3302.1228 | 3302.1228 | `0` |
| 1 | `ETHUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 3129.3518 | 3129.3518 | `0` |
| 2 | `SOLUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `2.0h, 8.0h` | 5,838 | 4,012 | -1438.935 | -1438.935 | `0` |
| 3 | `AVAXUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 2320.8208 | 2320.8208 | `0` |
| 4 | `TAOUSDT` | `2024-01-30..2026-11-24` · 1,030 · 56.4074% · 0 | *(Perp-Only)* · 0 · 0.0000% · 1,826 · 0 | `0 (0 / 0 / 0)` | `4.0h, 8.0h` | 5,120 | 4,091 | 4308.9801 | 4308.9801 | `0` |
| 5 | `SUIUSDT` | `2023-05-03..2026-11-24` · 1,302 · 71.3034% · 0 | `2023-05-03..2026-11-24` · 1,302 · 71.3034% · 524 · 0 | `1,302 (388 / 914 / 730)` | `8.0h` | 3,903 | 2,602 | 2986.7103 | 2986.7103 | `0` |
| 6 | `ORDIUSDT` | `2023-05-22..2026-11-24` · 1,283 · 70.2629% · 0 | `2023-08-17..2026-11-24` · 1,196 · 65.4984% · 630 · 0 | `1,196 (282 / 914 / 730)` | `4.0h, 8.0h` | 5,880 | 4,597 | 3434.8773 | 3434.8773 | `0` |
| 7 | `PENDLEUSDT` | `2023-07-05..2026-11-24` · 1,239 · 67.8532% · 0 | `2023-07-03..2026-11-24` · 1,241 · 67.9628% · 585 · 0 | `1,239 (325 / 914 / 730)` | `8.0h` | 3,714 | 2,476 | 2914.9431 | 2914.9431 | `0` |
| 8 | `WIFUSDT` | `2024-01-11..2026-11-24` · 1,049 · 57.4480% · 0 | `2024-04-29..2026-11-24` · 940 · 51.4786% · 886 · 0 | `940 (26 / 914 / 730)` | `4.0h, 8.0h` | 5,177 | 4,129 | 3668.9133 | 3668.9133 | `0` |
| 9 | `LINKUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 4124.4648 | 4124.4648 | `0` |
| 10 | `NEARUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2022-03-11..2026-11-24` · 1,720 · 94.1950% · 106 · 0 | `1,720 (806 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 3500.8266 | 3500.8266 | `0` |
| 11 | `LTCUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 3860.7875 | 3860.7875 | `0` |
| 12 | `BNBUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2022-03-10..2026-11-24` · 1,721 · 94.2497% · 105 · 0 | `1,721 (807 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | -1453.4353 | -1453.4353 | `0` |
| 13 | `XRPUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 3580.0192 | 3580.0192 | `0` |
| 14 | `DOGEUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 4386.3871 | 4386.3871 | `0` |
| 15 | `ADAUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 3278.9392 | 3278.9392 | `0` |
| 16 | `DOTUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 1409.5901 | 1409.5901 | `0` |
| 17 | `ATOMUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 627.9238 | 627.9238 | `0` |
| 18 | `UNIUSDT` | `2021-11-30..2026-11-24` · 1,821 · 99.7262% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,821 (907 / 914 / 730)` | `8.0h` | 5,461 *(+17 pre)* | 3,640 | 3778.1898 | 3778.1898 | `0` |
| 19 | `AAVEUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 3858.8278 | 3858.8278 | `0` |
| 20 | `ARBUSDT` | `2023-03-23..2026-11-24` · 1,343 · 73.5487% · 0 | `2023-03-23..2026-11-24` · 1,343 · 73.5487% · 483 · 0 | `1,343 (429 / 914 / 730)` | `8.0h` | 4,026 | 2,684 | 3437.1092 | 3437.1092 | `0` |
| 21 | `OPUSDT` | `2022-06-01..2026-11-24` · 1,638 · 89.7043% · 0 | `2022-05-31..2026-11-24` · 1,639 · 89.7590% · 187 · 0 | `1,638 (724 / 914 / 730)` | `8.0h` | 4,910 | 3,273 | 3561.5561 | 3561.5561 | `0` |
| 22 | `APTUSDT` | `2022-10-19..2026-11-24` · 1,498 · 82.0372% · 0 | `2022-10-19..2026-11-24` · 1,498 · 82.0372% · 328 · 0 | `1,498 (584 / 914 / 730)` | `8.0h` | 4,492 | 2,994 | 289.6585 | 289.6585 | `0` |
| 23 | `INJUSDT` | `2022-08-17..2026-11-24` · 1,561 · 85.4874% · 0 | `2022-09-16..2026-11-24` · 1,531 · 83.8445% · 295 · 0 | `1,531 (617 / 914 / 730)` | `1.0h, 6.0h, 8.0h` | 4,830 | 3,269 | 490.9751 | 490.9751 | `0` |
| 24 | `TIAUSDT` | `2023-10-31..2026-11-24` · 1,121 · 61.3910% · 0 | `2023-10-31..2026-11-24` · 1,121 · 61.3910% · 705 · 0 | `1,121 (207 / 914 / 730)` | `4.0h, 8.0h` | 5,392 | 4,272 | 622.9501 | 622.9501 | `0` |
| 25 | `SEIUSDT` | `2023-08-16..2026-11-24` · 1,197 · 65.5531% · 0 | `2023-08-15..2026-11-24` · 1,198 · 65.6079% · 628 · 0 | `1,197 (283 / 914 / 730)` | `8.0h` | 3,589 | 2,392 | -712.4773 | -712.4773 | `0` |
| 26 | `FILUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 4198.0154 | 4198.0154 | `0` |
| 27 | `BCHUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | -204.7362 | -204.7362 | `0` |
| 28 | `ETCUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2022-07-28..2026-11-24` · 1,581 · 86.5827% · 245 · 0 | `1,581 (667 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 2561.9697 | 2561.9697 | `0` |
| 29 | `ICPUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0000% · 0 · 0 | `1,826 (912 / 914 / 730)` | `8.0h` | 5,478 | 3,652 | 3451.2242 | 3451.2242 | `0` |

### Auditoría de Integridad & Reconciliación Causal de Funding (`BarOpenSettlement`)

- **Auditor (`scripts/audit_carry30_datasets.py` -> [`audit_report.json`](../data_local/bybit_carry30_5y/audit_report.json)) — Estado `PASS`**:
  - `30` JSONs crudos de funding (`raw_funding_failures = 0`, `manifest_failures = 0`), `240` archivos CSV auditados a través de `bybit_carry30_5y`, `bybit_carry30_r1`, `bybit_carry30_r2` y `bybit_carry30_24m` (`missing_files_total = 0`, `empty_non_r1_files_total = 0`, `spot_absent_files_total = 4` correspondientes a los CSVs `header-only` de `TAOUSDT` Spot, `spot_absence_mismatch_total = 0`, **`rows_checked = 235,464`**; `48,950` filas `linear` + `47,247` filas `spot` = `96,197` velas únicas en `5y`).
  - Integridad numérica y temporal impecable: `nan_inf_total = 0`, `non_pos_price_total = 0`, `invalid_ohlc_total = 0`, `neg_vol_total = 0`, `nonzero_spot_funding_total = 0`, `duplicate_ts_total = 0`, `backward_ts_total = 0`, `wrong_sym_id_total = 0`, `out_of_window_total = 0`, `misaligned_ts_total = 0`, `header_failures = 0`, `encoding_failures = 0`, `gap_files_total = 0`, `gap_count_total = 0`, `missing_bars_total = 0`, `trailing_missing_bars_total = 0`, `header_only_r1_files = 0`, `split_mismatch_total = 0`, `cohort24_mismatch_total = 0`, `per_bar_funding_mismatch_total = 0`.
  - Los `50` archivos con `leading_incomplete_files` corresponden exactamente a las `12` series `linear` de listado tardío (`TAO`, `SUI`, `ORDI`, `PENDLE`, `WIF`, `UNI`, `ARB`, `OP`, `APT`, `INJ`, `TIA`, `SEI`) más las `13` series `spot` duales de listado tardío (`SUI`, `ORDI`, `PENDLE`, `WIF`, `NEAR`, `BNB`, `ARB`, `OP`, `APT`, `INJ`, `TIA`, `SEI`, `ETC`), repetidas en `5y` y `R1` (`25 + 25 = 50`; `0` en `R2` y `0` en `24m`).
- **Agregación causal en cubetas semiabiertas `(bar_open_ts - 24h, bar_open_ts]` y acotación de vida contractual**: Se consolidaron **`106,515`** eventos sub-24h (`subbar_aggregated_events_total = 106,515`) sobre `155,458` liquidaciones retenidas (incluyendo `2.0h/8.0h` en `SOLUSDT`, `4.0h/8.0h` en `TAOUSDT`/`ORDIUSDT`/`WIFUSDT`/`TIAUSDT`, y `1.0h/6.0h/8.0h` en `INJUSDT`) con **`unmatched_funding_events_total = 0`**, **`funding_contract_life_mismatch_total = 0`**, **`funding_reconciliation_failures = 0`** y **`diff_bps = 0` en los `30 / 30` pares**.
- **Resumen `cohort24_summary` (`2024-11-25` a `2026-11-25`, `730` días)**: `symbols_total = 30`, `dual_listed_symbols_count = 29`, `perp_only_symbols = ["TAOUSDT"]`, `expected_24m_bars = 730`, `linear_full_730_count = 30`, `spot_dual_full_730_count = 29`, `dual_overlap_full_730_count = 29`, `active_dual_pairs_24m_100pct = true`.

> [!WARNING]
> **Divulgaciones Metodológicas y de Mercado Real (`30` pares, `5` años)**:
> 1. **`[04] TAOUSDT` Perp-Only en Bybit V5 (`0` barras Spot)**: Confirmado mediante `/v5/market/instruments-info` (`spot_listed_on_bybit = false`). Sus `4` CSVs Spot contienen únicamente la cabecera (`0` filas, `0` precios sintéticos). En acatamiento estricto de la Ley 4, [`CarryUtaEngine::on_slice`](../src/domain/signals/carry_uta.rs#L151-L155) y [`open_carry_uta_day`](../src/application/carry_uta_accounting.rs#L93-L96) excluyen a `TAOUSDT` de toda asignación Carry, operando sobre `29` pares duales elegibles.
> 2. **`[18] UNIUSDT` y `17` registros pre-listado de funding API**: `UNIUSDT` Linear abre su primera vela real el `2021-11-30T00:00:00Z` (`1,821 / 1,826` barras, `5` barras iniciales ausentes), mientras el endpoint `/v5/market/funding/history` de Bybit devuelve `17` liquidaciones *placeholder* de `0.0001` (`1 bps` c/u, suma `17 bps`) entre `2021-11-24T08:00:00Z` y `2021-11-29T16:00:00Z`. [`fetch_bybit_carry30_5y.py:594-638`](../scripts/fetch_bybit_carry30_5y.py#L594-L638) y [`audit_carry30_datasets.py:119-160`](../scripts/audit_carry30_datasets.py#L119-L160) acotan causalmente los eventos a `effective_funding_start_utc = 2021-11-30T00:00:00Z`, excluyendo las `17` filas pre-listado y preservando `5,461` liquidaciones reales (`3,778.1898 bps`, `diff_bps = 0`).
> 3. **Sesgo residual de supervivencia**: El panel `0:BTCUSDT..29:ICPUSDT` está condicionado a activos que permanecieron activos y líquidos en Bybit V5 hasta `2026-11-25`, excluyendo tokens colapsados o deslistados en 2021–2026. Los rendimientos positivos observados representan por tanto una **cota superior optimista** frente a un universo ex-ante sin sesgo de supervivencia; la reprobación de la barrera económica en `R2` y `24m` ocurre incluso bajo esta ventaja de selección.
> 4. **Elegibilidad real de colateral UTA y fricciones de Bybit Cross-Margin**: Nuestro modelo asume paridad uniforme `1:1` de colateral entre USDT y todos los activos Spot hasta `L_tot = coverage_ratio` (`0.50 / 0.75 / 0.85`), con tasa plana `borrow_apr_pct = 15.0%` aplicada sobre `borrowed_usdt_frac`. En la Cuenta de Trading Unificada (UTA) real de Bybit, las altcoins Spot sufren **ratios de valor de colateral (*haircuts*) escalonados** (p. ej., mid-caps/memes como `WIF`, `ORDI`, `TIA`, `SEI`, `PENDLE` cargan *collateral ratios* reducidos o elegibilidad nula frente a `BTC`/`ETH`/`SOL`), las tasas horarias de préstamo USDT presentan picos bruscos en fases alcistas, y las reglas de margen de mantenimiento / auto-liquidación / ADL operan antes de la paridad teórica `1:1`. En consecuencia, la capacidad efectiva de apalancamiento UTA en cola altcoin es más restrictiva que lo modelado.

---

## 2. Trazabilidad `Ecuación (LaTeX) -> archivo:línea` (Track 6 Cash-and-Carry UTA)

| Concepto / Ecuación (LaTeX) | Ubicación Exacta (`archivo:línea`) | Análisis Dimensional | Guardas de Dominio y Numéricas |
|---|---|---|---|
| **APR rolling (`window_days = 30`)**:<br>$\text{APR}_i(t) = \frac{365}{100 \cdot W}\sum_{j=0}^{W-1} f_{i,t-j}\;[\%]$ | [`src/domain/signals/carry_uta.rs:9-10, 144-156, 256-260`](../src/domain/signals/carry_uta.rs#L144-L156) (`window_apr_pct` `L256-260`) | $f_{i}$ en $[\text{bps/día}]$; $\text{APR}_i$ en $[\%]$ con `DAYS_PER_YEAR = 365.0` | Exige `linear.contains_key && spot.contains_key` (`L151`), `window.len() == window_days` y `apr.is_finite()`; rechaza timestamps no monótonos y `funding` o `volume` inválidos (`L125-142`). |
| **Barrera de amortización (`payback_apr_hurdle_pct`)**:<br>$\Delta\text{APR}_{\text{hurdle}} = \frac{2 \cdot c_{\text{carry,VIP0}} \cdot 365}{100 \cdot D_{\text{payback}}}\;[\%]$ | [`src/domain/signals/carry_uta.rs:49-65, 92-94`](../src/domain/signals/carry_uta.rs#L49-L65); invocación en [`falsifier_carry_uta.rs:61, 68, 203`](../src/application/falsifier_carry_uta.rs#L60-L68) | $c_{\text{carry,VIP0}}$ en $[\text{bps/turn}_L]$ (`18.50` a `1.5 bps` slip); $D_{\text{payback}}=30\text{d} \implies 4.501\overline{6}\%$ (`4.015%` a `0.5`, `5.231\overline{6}%` a `3.0 bps`) | Rechaza `carry_cost_vip0_bps < 0` o no finito (`InvalidFeeBps`), `payback_days == 0` (`InvalidStatParam`) y `hurdle` no finito. |
| **Selección (`BlindPeriodic` vs. `CostAmortized`) y Sizing `L_i`**:<br>$L_i = \frac{\text{coverage\_ratio}}{\max(\lvert H_t \rvert,\, \min(\text{min\_div}, \text{max\_pairs}))}$ | [`src/domain/signals/carry_uta.rs:12-47, 161-242, 250-255`](../src/domain/signals/carry_uta.rs#L161-L241) (`select_holdings` `L184-241`, `best_cmp` `L250-255`); grilla de `9` specs en [`carry_uta_types.rs:15-91`](../src/application/carry_uta_types.rs#L15-L91) | $L_i$ fracción nocional por pata ($\sum L_i \le \text{coverage\_ratio} \in \{0.50, 0.75, 0.85\}$) | `BlindPeriodic` (`cadence=30d`, `entry=8%`, `exit=-2%`) re-rankea todo el universo; `CostAmortized` (`cadence=7d`, `min_hold=14d`, `entry=6%/8%`, `exit=0%`) expulsa solo si `apr <= exit_apr_pct` o si `days >= 14` y `chal.apr - inc.apr >= payback_hurdle_pct` (`L219-237`). Desempate determinista `SymbolId` ascendente (`L250-255`). |
| **Precio de entrada armónico ante aumento de nocional (`new_l > pos.covered_l`)**:<br>$\bar{P}_{\text{entry}}^{\prime} = \frac{L_{\text{new}}}{\frac{L_{\text{old}}}{\bar{P}_{\text{old}}} + \frac{L_{\text{new}} - L_{\text{old}}}{P_{\text{open}}}}$ | [`src/application/carry_uta_accounting.rs:102-112`](../src/application/carry_uta_accounting.rs#L102-L112) (`UtaOpenPosition` `L17-21`) | $\bar{P}_{\text{entry}}$ en $[\text{USDT}]$ tanto en pata `perp` como en `spot` | Al reducir nocional (`new_l <= pos.covered_l`), preserva el precio medio de entrada; al cerrar completamente (`positions.retain`, `L139`), borra la posición. |
| **Física Cross-Margin UTA (`borrowed_usdt_frac`, `borrow_bps`)**:<br>$U_{\text{short}} = \sum_i L_i \!\left(\frac{P_{i,\text{lin,open}}-\bar{P}_{i,\text{perp}}}{\bar{P}_{i,\text{perp}}}\right)^+$,<br>$B_{\text{frac}} = \left(U_{\text{short}} - (1 - L_{\text{tot}})^+\right)^+$, $\text{borrow\_bps} = B_{\text{frac}}\cdot\frac{\text{borrow\_apr}\cdot 100}{365}$ | [`src/application/carry_uta_accounting.rs:113-115, 141-148`](../src/application/carry_uta_accounting.rs#L113-L148); constantes default en [`carry_uta_types.rs:11-13`](../src/application/carry_uta_types.rs#L11-L13) | $U_{\text{short}}, B_{\text{frac}}$ fracción del capital; $\text{borrow\_bps}$ en $[\text{bps/día}]$ (`4.109589 bps/d` por unidad prestada a `15% APR`) | Exige `recenter_trigger_pct > 0`, `borrow_apr_pct >= 0`, `days_per_year > 0` y valores finitos (`L78-86, 143-148`). |
| **Recenter Trigger (`recenter_trigger_pct`) y peaje (`2 * L_i * c_carry`)**:<br>Si $\frac{P_{i,\text{lin,open}}-\bar{P}_{i,\text{perp}}}{\bar{P}_{i,\text{perp}}} \ge \theta_{\text{rec}} \implies \text{Turn}_{\text{rec}} \mathrel{+}= 2L_i$ | [`src/application/carry_uta_accounting.rs:116-121`](../src/application/carry_uta_accounting.rs#L116-L121); triggers $\theta_{\text{rec}} \in \{0.80, 0.35, 0.20\}$ en [`carry_uta_types.rs:79-91`](../src/application/carry_uta_types.rs#L79-L91) | $\theta_{\text{rec}}$ adimensional; peaje de rotación `2.0 * pos.covered_l` por cierre+reapertura simultánea | Tras dispararse, resetea `pos.entry_perp_px = lin_open` y `pos.entry_spot_px = spot_open` (`L119-120`), descargando el déficit acumulado antes de los días sucesivos. |
| **Basis Real, `BarOpenSettlement`, EOF y Tarifas Duales**:<br>$R_{\text{net},t} = \sum_i L_i(r_{i,\text{spot}}-r_{i,\text{lin}})\cdot 10^4 + \sum_i L_i f_{i,\text{exit}} - \text{borrow\_bps}_t - \text{Turn}_{L,t}\cdot c_{\text{carry}}$ | [`src/application/carry_uta_accounting.rs:167-248`](../src/application/carry_uta_accounting.rs#L167-L248) (`settle_carry_uta_day`); [`falsifier_carry_uta.rs:60-103`](../src/application/falsifier_carry_uta.rs#L60-L103); [`macro_accounting.rs:62-68`](../src/application/macro_accounting.rs#L62-L68) | $[\text{bps/día}]$ sobre capital; $c_{\text{VIP0}} = 18.50\text{ bps}$ (`1.5 bps` slip), $c_{\text{MakerDiag}} = 12.00\text{ bps}$ | Decisión al cierre `t`, apertura en `Open(t+1)`; `BarOpenSettlement` acredita funding de `(t, t+1]` en `Open(t+2)` e ignora funding en EOF `Close(T)` sumando `eof_close_turn_l = sum |L_i|` (`L180-201`). |
| **Métricas de Capital y Batch Estadístico (`M = 9`)**:<br>$E_t = \max(0, E_{t-1}(1 + R_{\text{net},t}/10^4))$, $\text{CAGR\%} = (E_T^{365/N}-1)\cdot 100$, $\text{AnnSR} = \frac{\mu}{\sigma}\sqrt{365}$ | [`src/application/carry_uta_accounting.rs:250-273`](../src/application/carry_uta_accounting.rs#L250-L273) $\to$ [`macro_baselines.rs:132-167`](../src/application/macro_baselines.rs#L132-L167); batch en [`falsifier_carry_uta.rs:125-126`](../src/application/falsifier_carry_uta.rs#L125-L126) $\to$ [`stats.rs:277-348`](../src/domain/stats.rs#L277-L348) | $E_t$ adimensional (`E_0 = 1.0`); `CAGR%`, `MaxDD%` en $[\%]$; `t_nw`, `DSR`, `n_min` adimensionales | `t_crit = max(3.50, normal_inv_cdf(...)) = 3.5000`; `DSR` usa `sr_cross_std` real de las `9` celdas; `MakerDiag` usa `is_binding = false` y jamás emite `Pass`. |

---

## 3. Tabla Completa del Barrido Principal (`30` Pares, `5` Años, `slip = 1.5 bps`, `borrow = 15.0%`, `M = 9`, `t_crit = 3.5000`)

Ejecutado mediante [`scripts/run_track6_sweep.ps1`](../scripts/run_track6_sweep.ps1) sobre [`data_local/bybit_carry30_5y`](../data_local/bybit_carry30_5y) (`15` escenarios totales en el barrido):

- **Cabecera de lote observada ([`main_30p_5y_s1p5_b15.json`](../data_local/reports_track6/main_30p_5y_s1p5_b15.json))**: `track = Track6-CarryUTA` | `slices_processed = 1,826` | `linear_candles_processed = 48,950` | `spot_candles_processed = 47,247` | `events_processed = 96,197` | `slippage_bps = 1.5` | `borrow_apr_pct = 15.0` | `days_per_year = 365.0` | `m_hypotheses = 9` | `t_crit = 3.5000` | `sr_cross_std_vip0 = 0.030397` (`0.0303965891`) | `sr_cross_std_diag = 0.029783` (`0.0297827178`).

| Strategy | N_days | n_min | BasisBps | FundBps | BorrowBps | CostVIP0 | AvgTurnL | AvgUtil% | RecCnt | NetVIP0 | CAGR% | AnnSR | MaxDD% | Calmar | t_NW | DSR | Verdict |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `Cash50_Blind30d` | 1,796 | 737 | +0.003 | +1.211 | 0.0000 | 0.427 | 0.0231 | 50.00% | 16 | +0.786 | 2.90% | 2.46 | 1.08% | 2.68 | 6.582 | 0.9994 | `Pass` |
| `Cash50_Amort30d_Top4` | 1,796 | 413 | -0.003 | +1.263 | 0.0000 | 0.200 | 0.0108 | 50.00% | 13 | +1.060 | 3.94% | 3.29 | 1.01% | 3.89 | 10.177 | 1.0000 | `Pass` |
| `Cash50_Amort30d_Top2` | 1,796 | 1,050 | -0.008 | +1.288 | 0.0000 | 0.206 | 0.0111 | 46.63% | 7 | +1.074 | 3.98% | 2.06 | 2.13% | 1.87 | 7.786 | 0.9950 | `Pass` |
| `Uta75_Blind30d` | 1,796 | 957 | +0.004 | +1.816 | 0.0001 | 0.792 | 0.0428 | 75.00% | 55 | +1.028 | 3.81% | 2.16 | 1.76% | 2.16 | 5.756 | 0.9969 | `Pass` |
| `Uta75_Amort30d_Top4` | 1,796 | 450 | -0.005 | +1.894 | 0.0000 | 0.375 | 0.0203 | 75.00% | 32 | +1.515 | 5.67% | 3.15 | 1.58% | 3.58 | 9.647 | 1.0000 | `Pass` |
| `Uta75_Amort30d_Top2` | 1,796 | 1,117 | -0.012 | +1.933 | 0.0008 | 0.379 | 0.0205 | 69.95% | 16 | +1.541 | 5.74% | 2.00 | 3.20% | 1.80 | 7.511 | 0.9940 | `Pass` |
| `Uta85_Blind30d` | 1,796 | 1,442 | +0.005 | +2.058 | 0.0009 | 1.094 | 0.0592 | 85.00% | 100 | +0.967 | 3.57% | 1.76 | 2.08% | 1.72 | 4.733 | 0.9710 | `Pass` |
| `Uta85_Amort30d_Top4` | 1,796 | 511 | -0.006 | +2.147 | 0.0002 | 0.531 | 0.0287 | 85.00% | 56 | +1.610 | 6.03% | 2.96 | 1.80% | 3.35 | 9.108 | 1.0000 | `Pass` |
| `Uta85_Amort30d_Top2` | 1,796 | 1,282 | -0.014 | +2.190 | 0.0019 | 0.534 | 0.0289 | 79.27% | 28 | +1.640 | 6.12% | 1.87 | 3.78% | 1.62 | 6.919 | 0.9869 | `Pass` |

- **Referencia diagnóstica no vinculante `VIP0-MakerDiag` (`12.00 bps/turn_L`, `is_binding = false`)**: Las `9` celdas alcanzan `NetMakerDiag` entre **`+0.9360 bps/día`** (`Cash50_Blind30d`, `CAGR = 3.47%`, `AnnSR = 3.05`, `t_NW = 8.208`, `DSR = 1.0000`) y **`+1.8279 bps/día`** (`Uta85_Amort30d_Top2`, `CAGR = 6.84%`, `AnnSR = 2.09`, `t_NW = 7.808`, `DSR = 0.9967`), con `0 / 9 Pass` por restricción `is_binding = false`.
- **Lectura del barrido completo de `5` años**: En la muestra histórica completa (impulsada por los elevados *funding rates* del régimen alcista 2021–2024 `R1`), las **`9 / 9`** variantes aprueban `t_NW >= 3.5000`, `N >= n_min` y `DSR >= 0.95`. Únicamente `Uta85_Amort30d_Top4` (`6.03%` CAGR, `AnnSR = 2.96`, `MaxDD = 1.80%`, `Calmar = 3.35`) y `Uta85_Amort30d_Top2` (`6.12%` CAGR, `AnnSR = 1.87`, `MaxDD = 3.78%`, `Calmar = 1.62`) superan el `6.0%` CAGR sobre el ciclo completo de `5` años — pero como muestra §4, esa media de `5` años enmascara la caída estructural del funding en los últimos `24`–`30` meses.

---

## 4. Matriz OOS (`R1` vs. `R2` vs. Cohorte Completa `24m`) & Verificación de Barrera Económica (`>= 6.0% CAGR`)

### 4.1 Régimen 1 (`R1`: `2021-11-25` a `2024-05-25`, `912` slices, `21,530` linear / `20,741` spot, `sr_std(VIP0) = 0.029767`)

Fuente: [`oos_30p_r1_s1p5_b15.json`](../data_local/reports_track6/oos_30p_r1_s1p5_b15.json).

| Strategy | N | n_min | Basis | Fund | Borrow | Cost | TurnL | Util% | RecCnt | NetVIP0 | CAGR% | AnnSR | MaxDD% | t_NW | DSR | Verdict |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `Cash50_Blind30d` | 882 | 502 | +0.028 | +1.608 | 0.0000 | 0.425 | 0.0230 | 50.00% | 6 | +1.211 | 4.51% | 2.98 | 1.08% | 6.071 | 0.9989 | `Pass` |
| `Cash50_Amort30d_Top4` | 882 | 404 | +0.007 | +1.676 | 0.0000 | 0.283 | 0.0153 | 50.00% | 11 | +1.400 | 5.23% | 3.33 | 1.01% | 7.689 | 1.0000 | `Pass` |
| `Cash50_Amort30d_Top2` | 882 | 1,080 | +0.010 | +1.775 | 0.0000 | 0.346 | 0.0187 | 50.00% | 6 | +1.438 | 5.36% | 2.04 | 2.13% | 5.497 | 0.9699 | `InsufficientSample` |
| `Uta75_Blind30d` | 882 | 590 | +0.042 | +2.412 | 0.0002 | 0.794 | 0.0429 | 75.00% | 26 | +1.659 | 6.22% | 2.76 | 1.76% | 5.520 | 0.9977 | `Pass` |
| `Uta75_Amort30d_Top4` | 882 | 440 | +0.011 | +2.514 | 0.0000 | 0.527 | 0.0285 | 75.00% | 24 | +1.998 | 7.54% | 3.19 | 1.58% | 7.275 | 0.9999 | `Pass` |
| `Uta75_Amort30d_Top2` | 882 | 1,182 | +0.015 | +2.662 | 0.0009 | 0.645 | 0.0349 | 75.00% | 14 | +2.031 | 7.62% | 1.95 | 3.20% | 5.204 | 0.9621 | `InsufficientSample` |
| `Uta85_Blind30d` | 882 | 761 | +0.047 | +2.734 | 0.0008 | 1.088 | 0.0588 | 85.00% | 47 | +1.693 | 6.34% | 2.42 | 2.08% | 4.999 | 0.9902 | `Pass` |
| `Uta85_Amort30d_Top4` | 882 | 508 | +0.012 | +2.849 | 0.0004 | 0.758 | 0.0410 | 85.00% | 42 | +2.103 | 7.94% | 2.97 | 1.80% | 6.759 | 0.9997 | `Pass` |
| `Uta85_Amort30d_Top2` | 882 | 1,361 | +0.016 | +3.017 | 0.0022 | 0.874 | 0.0472 | 85.00% | 22 | +2.158 | 8.09% | 1.81 | 3.78% | 4.759 | 0.9400 | `InsufficientSample` |

### 4.2 Régimen 2 (`R2`: `2024-05-25` a `2026-11-25`, `914` slices, `27,420` linear / `26,506` spot, `sr_std(VIP0) = 0.054827`)

Fuente: [`oos_30p_r2_s1p5_b15.json`](../data_local/reports_track6/oos_30p_r2_s1p5_b15.json).

| Strategy | N | n_min | Basis | Fund | Borrow | Cost | TurnL | Util% | RecCnt | NetVIP0 | CAGR% | AnnSR | MaxDD% | t_NW | DSR | Verdict |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `Cash50_Blind30d` | 884 | 1,710 | -0.014 | +0.746 | 0.0000 | 0.408 | 0.0221 | 50.00% | 4 | +0.324 | 1.19% | 1.62 | 0.65% | 2.628 | 0.5133 | `InsufficientSample` |
| `Cash50_Amort30d_Top4` | 884 | 327 | -0.015 | +0.799 | 0.0000 | 0.146 | 0.0079 | 50.00% | 5 | +0.638 | 2.35% | 3.70 | 0.29% | 6.452 | 0.9979 | `Pass` |
| `Cash50_Amort30d_Top2` | 884 | 542 | -0.027 | +0.826 | 0.0000 | 0.084 | 0.0045 | 46.97% | 1 | +0.716 | 2.64% | 2.87 | 0.39% | 6.360 | 0.9690 | `Pass` |
| `Uta75_Blind30d` | 884 | 3,281 | -0.021 | +1.120 | 0.0000 | 0.746 | 0.0403 | 75.00% | 21 | +0.353 | 1.29% | 1.17 | 0.98% | 1.914 | 0.2716 | `InsufficientSample` |
| `Uta75_Amort30d_Top4` | 884 | 387 | -0.023 | +1.199 | 0.0000 | 0.285 | 0.0154 | 75.00% | 13 | +0.891 | 3.30% | 3.40 | 0.44% | 5.957 | 0.9938 | `Pass` |
| `Uta75_Amort30d_Top2` | 884 | 602 | -0.040 | +1.240 | 0.0001 | 0.173 | 0.0093 | 70.46% | 4 | +1.027 | 3.81% | 2.73 | 0.59% | 5.944 | 0.9517 | `Pass` |
| `Uta85_Blind30d` | 884 | 10,950 | -0.024 | +1.269 | 0.0006 | 1.023 | 0.0553 | 85.00% | 41 | +0.221 | 0.80% | 0.64 | 1.35% | 1.069 | 0.0782 | `InsufficientSample` |
| `Uta85_Amort30d_Top4` | 884 | 464 | -0.026 | +1.359 | 0.0000 | 0.412 | 0.0223 | 85.00% | 23 | +0.921 | 3.41% | 3.11 | 0.49% | 5.614 | 0.9829 | `Pass` |
| `Uta85_Amort30d_Top2` | 884 | 684 | -0.046 | +1.405 | 0.0008 | 0.267 | 0.0144 | 79.86% | 8 | +1.092 | 4.05% | 2.56 | 0.66% | 5.616 | 0.9227 | `Fail` |

### 4.3 Cohorte Completa `24m` (`2024-11-25` a `2026-11-25`, `730` slices, `21,900` linear / `21,170` spot, `sr_std(VIP0) = 0.038932`)

Fuente: [`oos_30p_24m_s1p5_b15.json`](../data_local/reports_track6/oos_30p_24m_s1p5_b15.json) (`29 / 29` duales activos al `100.0%` sin ingresos tardíos).

| Strategy | N | n_min | Basis | Fund | Borrow | Cost | TurnL | Util% | RecCnt | NetVIP0 | CAGR% | AnnSR | MaxDD% | t_NW | DSR | Verdict |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `Cash50_Blind30d` | 700 | 4,139 | -0.002 | +0.612 | 0.0000 | 0.410 | 0.0221 | 50.00% | 4 | +0.200 | 0.73% | 1.04 | 0.31% | 2.170 | 0.4524 | `InsufficientSample` |
| `Cash50_Amort30d_Top4` | 700 | 793 | -0.007 | +0.582 | 0.0000 | 0.148 | 0.0080 | 49.25% | 2 | +0.427 | 1.57% | 2.38 | 0.33% | 5.119 | 0.9352 | `InsufficientSample` |
| `Cash50_Amort30d_Top2` | 700 | 1,196 | -0.009 | +0.518 | 0.0000 | 0.079 | 0.0043 | 42.00% | 0 | +0.430 | 1.58% | 1.93 | 0.30% | 5.019 | 0.8588 | `InsufficientSample` |
| `Uta75_Blind30d` | 700 | 11,484 | -0.003 | +0.918 | 0.0000 | 0.733 | 0.0396 | 75.00% | 16 | +0.181 | 0.66% | 0.62 | 0.58% | 1.304 | 0.2488 | `InsufficientSample` |
| `Uta75_Amort30d_Top4` | 700 | 911 | -0.011 | +0.873 | 0.0011 | 0.261 | 0.0141 | 73.88% | 6 | +0.600 | 2.21% | 2.22 | 0.49% | 4.728 | 0.9094 | `InsufficientSample` |
| `Uta75_Amort30d_Top2` | 700 | 1,196 | -0.013 | +0.778 | 0.0000 | 0.119 | 0.0064 | 63.00% | 0 | +0.646 | 2.38% | 1.93 | 0.45% | 5.019 | 0.8588 | `InsufficientSample` |
| `Uta85_Blind30d` | 700 | 58,371 | -0.004 | +1.040 | 0.0004 | 0.944 | 0.0510 | 85.00% | 26 | +0.093 | 0.33% | 0.28 | 0.82% | 0.571 | 0.1220 | `InsufficientSample` |
| `Uta85_Amort30d_Top4` | 700 | 1,110 | -0.013 | +0.989 | 0.0020 | 0.352 | 0.0190 | 83.73% | 11 | +0.623 | 2.29% | 2.01 | 0.64% | 4.179 | 0.8635 | `InsufficientSample` |
| `Uta85_Amort30d_Top2` | 700 | 1,374 | -0.015 | +0.881 | 0.0001 | 0.180 | 0.0097 | 71.40% | 2 | +0.687 | 2.53% | 1.80 | 0.51% | 4.623 | 0.8168 | `InsufficientSample` |

### 4.4 Verificación Explícita de la Barrera Económica (`>= 6.0% CAGR` en `R2` y `24m`)

1. **Rescate estadístico parcial en `R2` (`5 / 9 Pass`)**: La rotación amortizada (`Amort30d`) evita el colapso de las `3` reglas `Blind30d` (`InsufficientSample`, `t_NW = 1.069..2.628`, `DSR = 0.0782..0.5133`) y aprueba **`5 / 6`** celdas `Amort30d` (`Cash50_Amort30d_Top4/Top2`, `Uta75_Amort30d_Top4/Top2`, `Uta85_Amort30d_Top4`; `Uta85_Amort30d_Top2` queda en `Fail` por `DSR = 0.9227 < 0.95` pese a `t_NW = 5.616` y `N = 884 >= 684`).
2. **Incumplimiento absoluto (`0 / 9`) de la barrera `>= 6.0% CAGR` en `R2`**: Entre las `5` celdas `Pass` de `R2`, el mayor rendimiento compuesto es **`3.81%` CAGR** (`Uta75_Amort30d_Top2`, seguido por `3.41%` en `Uta85_Amort30d_Top4` y `3.30%` en `Uta75_Amort30d_Top4`); incluso la celda no aprobada `Uta85_Amort30d_Top2` (`Fail`) solo alcanza **`4.05%`** CAGR (`4.41%` bajo `MakerDiag` no vinculante). Ninguna configuración se aproxima al umbral del **`6.0%`**.
3. **Colapso en la cohorte limpia `24m` (`0 / 9 Pass`, máx `2.53%` CAGR)**: Al excluir la cola de *funding* elevado de mayo–noviembre de 2024 (incluida todavía al inicio de `R2`), `24m` exhibe `FundBps` comprimido (`+0.518..+1.040 bps/día`), dejando las `9 / 9` celdas en `InsufficientSample` (`N = 700 < n_min = 793..58,371`; `DSR <= 0.9352 < 0.95`), con máximo `CAGR = 2.53%` en `Uta85_Amort30d_Top2` y `1.57%–2.38%` en las demás variantes `Amort30d` (incluso bajo `MakerDiag` no vinculante, el máximo en `24m` es `2.76%` CAGR).

---

## 5. Ablación Limpia (Descomposición Causal) y Análisis de Sensibilidad

### 5.1 (a) Universo `12` vs. `30` Pares (`5y`, `R1`, `R2`) y Paridad Forense con Track 5

Comparación controlada entre el universo de `12` mayores ([`ctrl_12p_5y_s1p5_b15.json`](../data_local/reports_track6/ctrl_12p_5y_s1p5_b15.json), [`ctrl_12p_r1_s1p5_b15.json`](../data_local/reports_track6/ctrl_12p_r1_s1p5_b15.json), [`ctrl_12p_r2_s1p5_b15.json`](../data_local/reports_track6/ctrl_12p_r2_s1p5_b15.json)) y el universo completo de `30` pares (`29` duales):

| Strategy | 5y (`12p -> 30p`) Net / CAGR / AnnSR / Verdict | R1 (`12p -> 30p`) Net / CAGR / AnnSR / Verdict | R2 (`12p -> 30p`) Net / CAGR / AnnSR / Verdict |
|---|---|---|---|
| `Cash50_Blind30d` | `+0.793 -> +0.786` \| `2.93% -> 2.90%` \| `4.65 -> 2.46` \| `Pass -> Pass` | `+1.094 -> +1.211` \| `4.07% -> 4.51%` \| `6.09 -> 2.98` \| `Pass -> Pass` | `+0.351 -> +0.324` \| `1.29% -> 1.19%` \| `1.92 -> 1.62` \| `Insuff -> Insuff` |
| `Cash50_Amort30d_Top4` | `+0.890 -> +1.060` \| `3.30% -> 3.94%` \| `5.98 -> 3.29` \| `Pass -> Pass` | `+1.275 -> +1.400` \| `4.76% -> 5.23%` \| `7.73 -> 3.33` \| `Pass -> Pass` | `+0.413 -> +0.638` \| `1.52% -> 2.35%` \| `2.81 -> 3.70` \| `Pass -> Pass` |
| `Cash50_Amort30d_Top2` | `+0.962 -> +1.074` \| `3.57% -> 3.98%` \| `6.11 -> 2.06` \| `Pass -> Pass` | `+1.370 -> +1.438` \| `5.13% -> 5.36%` \| `7.04 -> 2.04` \| `Pass -> Insuff` | `+0.356 -> +0.716` \| `1.31% -> 2.64%` \| `2.53 -> 2.87` \| `Pass -> Pass` |
| `Uta75_Blind30d` | `+1.085 -> +1.028` \| `4.04% -> 3.81%` \| `4.27 -> 2.16` \| `Pass -> Pass` | `+1.516 -> +1.659` \| `5.68% -> 6.22%` \| `5.69 -> 2.76` \| `Pass -> Pass` | `+0.456 -> +0.353` \| `1.67% -> 1.29%` \| `1.64 -> 1.17` \| `Insuff -> Insuff` |
| `Uta75_Amort30d_Top4` | `+1.262 -> +1.515` \| `4.71% -> 5.67%` \| `5.63 -> 3.15` \| `Pass -> Pass` | `+1.827 -> +1.998` \| `6.89% -> 7.54%` \| `7.37 -> 3.19` \| `Pass -> Pass` | `+0.557 -> +0.891` \| `2.05% -> 3.30%` \| `2.53 -> 3.40` \| `Pass -> Pass` |
| `Uta75_Amort30d_Top2` | `+1.389 -> +1.541` \| `5.20% -> 5.74%` \| `5.71 -> 2.00` \| `Pass -> Pass` | `+1.993 -> +2.031` \| `7.54% -> 7.62%` \| `6.78 -> 1.95` \| `Pass -> Insuff` | `+0.503 -> +1.027` \| `1.85% -> 3.81%` \| `2.39 -> 2.73` \| `Pass -> Pass` |
| `Uta85_Blind30d` | `+1.094 -> +0.967` \| `4.07% -> 3.57%` \| `3.80 -> 1.76` \| `Pass -> Pass` | `+1.512 -> +1.693` \| `5.67% -> 6.34%` \| `5.03 -> 2.42` \| `Pass -> Pass` | `+0.365 -> +0.221` \| `1.33% -> 0.80%` \| `1.15 -> 0.64` \| `Insuff -> Insuff` |
| `Uta85_Amort30d_Top4` | `+1.344 -> +1.610` \| `5.02% -> 6.03%` \| `5.28 -> 2.96` \| `Pass -> Pass` | `+1.990 -> +2.103` \| `7.53% -> 7.94%` \| `7.07 -> 2.97` \| `Pass -> Pass` | `+0.507 -> +0.921` \| `1.86% -> 3.41%` \| `1.98 -> 3.11` \| `Insuff -> Pass` |
| `Uta85_Amort30d_Top2` | `+1.495 -> +1.640` \| `5.60% -> 6.12%` \| `5.46 -> 1.87` \| `Pass -> Pass` | `+2.134 -> +2.158` \| `8.09% -> 8.09%` \| `6.35 -> 1.81` \| `Pass -> Insuff` | `+0.499 -> +1.092` \| `1.83% -> 4.05%` \| `2.07 -> 2.56` \| `Insuff -> Fail` |

- **Paridad forense exacta Track 6 vs. Track 5 (`12` pares, `Cash50_Blind30d` vs. `Carry_SpotPerp_TopCarry30d`)**:
  - En `5y`, `ctrl_12p_5y_s1p5_b15.json` (`Cash50_Blind30d`) replica exactamente a `Carry_SpotPerp_TopCarry30d` ([`reports_track5/main_5y_slip1p5.json`](../data_local/reports_track5/main_5y_slip1p5.json)) en `mean_basis_bps = -0.0145657697 bps/d` y `mean_fund_bps = +1.0624872425 bps/d` (idénticos a `10` decimales; `borrow = 0.0`). La única diferencia en rotación es `0.0137806236 - 0.0128062361 = 0.0009743875 = 1.75 / 1,796`, explicada al `100%` por **`recenter_count = 7`** eventos reales de `+80%` (`7 * 2 * 0.125 = 1.75`). La misma identidad bit-a-bit de `basis` y `fund` más `rec * 0.25 / N` aplica en `R1` (`3 * 0.25 / 882 = 0.0008503401`) y `R2` (`3 * 0.25 / 884 = 0.0008484163`). *(Nota: en `ctrl_12p_r2`, `Cash50_Blind30d` muestra `DSR = 0.9040` frente a `0.3216` en Track 5 porque `sr_cross_std_vip0` del lote homogéneo de `9` estrategias Carry es `0.026853`, frente a `0.083396` del lote heterogéneo de Track 5 con estrategias direccionales).*
- **Efecto estructural de expandir `12 -> 30` pares**: Incorporar `17` altcoins duales adicionales (`SUI`, `ORDI`, `PENDLE`, `WIF`, `ARB`, `OP`, `APT`, `INJ`, `TIA`, `SEI`, etc.) eleva el *funding* bruto de `CostAmortized` (`+1.263..+2.190` vs `+1.029..+1.806 bps/d` en `5y`) y el `CAGR` en `R2` (`Top4`: `+0.83..+1.55 pp`), pero **duplica a triplica la volatilidad del *basis* diario** (`std_bps` sube de `2.85–5.23` a `6.15–16.78 bps` en `5y`), reduciendo `AnnSR` a la mitad (`5.28–6.11 -> 1.87–3.29`) y elevando `MaxDD` (`0.33%–0.70% -> 1.01%–3.78%`).

### 5.2 (b) Regla de Selección (`Blind30d` vs. `Amort30d_Top4` vs. `Amort30d_Top2`)

- **Compresión de rotación y tarifas**: Dentro de cada régimen de margen en `30p`, `Amort30d_Top4` recorta `AvgTurnL` y `CostVIP0` en **`-51.5%` a `-53.2%`** en `5y` (`0.0231 -> 0.0108` en `Cash50`, `0.0428 -> 0.0203` en `Uta75`, `0.0592 -> 0.0287` en `Uta85`), **`-59.7%` a `-64.1%`** en `R2`, y **`-62.7%` a `-64.4%`** en `24m`. `Amort30d_Top2` profundiza el recorte hasta **`-73.9%` a `-79.5%`** en `R2` y **`-80.6%` a `-83.8%`** en `24m` al bloquear posiciones concentradas durante semanas.
- **Rescate estadístico en `R2` y costo de concentración en `Top2`**: La compuerta de amortización de `30d` + estancia mínima de `14d` eleva `t_NW` de `1.069..2.628` (`Blind30d`, `0 / 3 Pass` en `R2`) a `5.614..6.452` (`Amort30d_Top4`, **`3 / 3 Pass` en `R2`**). Sin embargo, concentrar en `Top2` (`max_pairs = 2`) duplica el `MaxDD` (`1.01% -> 2.13%` en `Cash50 5y`; `1.80% -> 3.78%` en `Uta85 5y`) e infla `std_bps`, provocando que las `3` variantes `Top2` fallen por `InsufficientSample` en `R1` (`N = 882 < n_min = 1,080..1,361`) y que `Uta85_Amort30d_Top2` quede en `Fail` en `R2` (`DSR = 0.9227 < 0.95`).

### 5.3 (c) Régimen de Margen (`Cash50` vs. `Uta75` vs. `Uta85`)

- **Escalamiento de funding vs. explosión de recentering (`80% -> 35% -> 20%`)**: Pasar de `Cash50` a `Uta75` y `Uta85` eleva el funding capturado (`+1.263 -> +1.894 -> +2.147 bps/d` en `Amort30d_Top4` `5y`), pero estrechar el gatillo de recentering (`+80% -> +35% -> +20%`) dispara los eventos de recentering de **`16 -> 55 -> 100`** en `Blind30d` (`5y`), **`13 -> 32 -> 56`** en `Amort30d_Top4` (`5y`), y **`7 -> 16 -> 28`** en `Amort30d_Top2` (`5y`). En `Blind30d` (`5y` y `R2`), `Uta85` rinde **menos** que `Uta75` (`3.57%` vs `3.81%` CAGR en `5y`; `0.80%` vs `1.29%` en `R2`) porque el peaje acumulado de `100` (`41` en `R2`) recenterings absorbe `1.094 bps/d` (`1.023 bps/d` en `R2`).
- **Utilización realizada (`AvgUtil%`)**: En `Blind30d` y `Amort30d_Top4`, la cobertura efectiva alcanza exactamente `50.00% / 75.00% / 85.00%` en `5y`, `R1` y `R2` (cayendo levemente a `49.25% / 73.88% / 83.73%` en `24m`), mientras que en `Top2` (`entry_apr_pct = 8.0%` con solo `2` ranuras), los periodos con `< 2` pares por encima del `8%` reducen `AvgUtil%` a `46.63% / 69.95% / 79.27%` en `5y` y a **`42.00% / 63.00% / 71.40%`** en `24m`.

### 5.4 (d) Sensibilidad a `borrow_apr` (`0% / 15% / 25%`) y `slippage` (`0.5 / 1.5 / 3.0 bps`) en `5y` y `24m`

Fuentes: `sens_30p_{5y,24m}_{s1p5_b0, s1p5_b25, s0p5_b15, s3p0_b15}.json`, [`main_30p_5y_s1p5_b15.json`](../data_local/reports_track6/main_30p_5y_s1p5_b15.json) y [`oos_30p_24m_s1p5_b15.json`](../data_local/reports_track6/oos_30p_24m_s1p5_b15.json).

| Strategy | `5y` Borrow `0% / 15% / 25%` (`s=1.5`) Net / BorrowBps / CAGR | `5y` Slip `0.5 / 1.5* / 3.0 bps` (`b=15%`) Net / CAGR / Verdict | `24m` Borrow `0% / 15% / 25%` (`s=1.5`) Net / BorrowBps / CAGR | `24m` Slip `0.5 / 1.5* / 3.0 bps` (`b=15%`) Net / CAGR / Verdict |
|---|---|---|---|---|
| `Cash50_Blind30d` | `+0.786/+0.786/+0.786` \| `0.0000` \| `2.90%` | `+0.832` (`3.08%`, `P`) / `+0.786` (`2.90%`, `P`) / `+0.717` (`2.64%`, `P`) | `+0.200/+0.200/+0.200` \| `0.0000` \| `0.73%` | `+0.244` (`0.89%`, `I`) / `+0.200` (`0.73%`, `I`) / `+0.134` (`0.49%`, `I`) |
| `Cash50_Amort30d_Top4` | `+1.060/+1.060/+1.060` \| `0.0000` \| `3.94%` | `+1.101` (`4.09%`, `P`) / `+1.060` (`3.94%`, `P`) / `+1.047` (`3.89%`, `P`) | `+0.427/+0.427/+0.427` \| `0.0000` \| `1.57%` | `+0.413` (`1.52%`, `I`) / `+0.427` (`1.57%`, `I`) / `+0.388` (`1.42%`, `I`) |
| `Cash50_Amort30d_Top2` | `+1.074/+1.074/+1.074` \| `0.0000` \| `3.98%` | `+1.125` (`4.17%`, `P`) / `+1.074` (`3.98%`, `P`) / `+1.013` (`3.75%`, `P`) | `+0.430/+0.430/+0.430` \| `0.0000` \| `1.58%` | `+0.446` (`1.64%`, `I`) / `+0.430` (`1.58%`, `I`) / `+0.436` (`1.60%`, `I`) |
| `Uta75_Blind30d` | `+1.028/+1.028/+1.028` \| `0/0.0001/0.0002` \| `3.81%` | `+1.114` (`4.13%`, `P`) / `+1.028` (`3.81%`, `P`) / `+0.900` (`3.32%`, `P`) | `+0.181/+0.181/+0.181` \| `0.0000` \| `0.66%` | `+0.260` (`0.95%`, `I`) / `+0.181` (`0.66%`, `I`) / `+0.062` (`0.22%`, `I`) |
| `Uta75_Amort30d_Top4` | `+1.515/+1.515/+1.515` \| `0.0000` \| `5.67%` | `+1.574` (`5.90%`, `P`) / `+1.515` (`5.67%`, `P`) / `+1.483` (`5.55%`, `P`) | `+0.601/+0.600/+0.599` \| `0/0.0011/0.0018` \| `2.21%` | `+0.556` (`2.04%`, `I`) / `+0.600` (`2.21%`, `I`) / `+0.523` (`1.92%`, `I`) |
| `Uta75_Amort30d_Top2` | `+1.542/+1.541/+1.540` \| `0/0.0008/0.0014` \| `5.75%->5.74%` | `+1.625` (`6.07%`, `P`) / `+1.541` (`5.74%`, `P`) / `+1.465` (`5.45%`, `P`) | `+0.646/+0.646/+0.646` \| `0.0000` \| `2.38%` | `+0.651` (`2.40%`, `I`) / `+0.646` (`2.38%`, `I`) / `+0.654` (`2.41%`, `I`) |
| `Uta85_Blind30d` | `+0.968/+0.967/+0.967` \| `0/0.0009/0.0015` \| `3.58%->3.57%` | `+1.086` (`4.02%`, `P`) / `+0.967` (`3.57%`, `P`) / `+0.790` (`2.90%`, **`I`**) | `+0.093/+0.093/+0.092` \| `0/0.0004/0.0007` \| `0.33%` | `+0.195` (`0.71%`, `I`) / `+0.093` (`0.33%`, `I`) / `-0.060` (`-0.23%`, **`F`**) |
| `Uta85_Amort30d_Top4` | `+1.610/+1.610/+1.610` \| `0/0.0002/0.0004` \| `6.03%` | `+1.697` (`6.37%`, `P`) / `+1.610` (`6.03%`, `P`) / `+1.582` (`5.92%`, `P`) | `+0.625/+0.623/+0.622` \| `0/0.0020/0.0034` \| `2.30%->2.29%` | `+0.559` (`2.05%`, `I`) / `+0.623` (`2.29%`, `I`) / `+0.527` (`1.94%`, `I`) |
| `Uta85_Amort30d_Top2` | `+1.642/+1.640/+1.639` \| `0/0.0019/0.0032` \| `6.12%->6.11%` | `+1.739` (`6.50%`, `P`) / `+1.640` (`6.12%`, `P`) / `+1.558` (`5.79%`, **`F`**) | `+0.687/+0.687/+0.687` \| `0/0.0001/0.0001` \| `2.53%` | `+0.697` (`2.57%`, `I`) / `+0.687` (`2.53%`, `I`) / `+0.716` (`2.64%`, `I`) |

*(Leyenda de veredictos: `P` = `Pass`, `I` = `InsufficientSample`, `F` = `Fail`.)*

- **Por qué `borrow_apr` (`0%` a `25%`) tiene impacto casi nulo (`<= 0.0034 bps/día`, `< 0.02 pp` CAGR)**: Cada vez que una pata corta sube `>= recenter_trigger_pct` (`+20% / +35% / +80%`), [`open_carry_uta_day`](../src/application/carry_uta_accounting.rs#L116-L120) resetea inmediatamente `entry_perp_px = lin_open`, impidiendo que el déficit no realizado crezca más allá del búfer USDT durante varios días consecutivos. Por ello, **`recenter_turn_l * 18.50 bps` domina a `borrow_bps` por más de `100×`**.
- **Sensibilidad a `slippage` (`0.5 / 1.5 / 3.0 bps`) y no-monotonía en `CostAmortized`**: Bajo `slip = 3.0 bps`, `Uta85_Blind30d` cae a `InsufficientSample` en `5y` (`DSR = 0.8183`) y a `Fail` negativo (`-0.23%` CAGR) en `24m`, mientras `Uta85_Amort30d_Top2` cae a `Fail` en `5y` (`DSR = 0.9497 < 0.95`). En las variantes `CostAmortized`, variar el `slippage` ajusta dinámicamente la barrera `payback_apr_hurdle_pct` (`4.015%` a `0.5 bps`, `4.502%` a `1.5 bps`, `5.232%` a `3.0 bps`), lo que filtra rotaciones marginales en `24m` (`Uta75_Amort30d_Top2` baja `TurnL` de `0.0075` a `0.0054`, dejando `NetVIP0` en `+0.654` a `3.0 bps` vs `+0.646` a `1.5 bps`). Incluso bajo el extremo optimista `0.5 bps` en `24m`, las `9 / 9` celdas permanecen en `InsufficientSample` (`CAGR <= 2.57%`).

### 5.5 (e) Viabilidad Operativa con Capital `$1,000–$2,000 USDT`

- **Tamaño por pata y piso mínimo (`minNotional = $5 USDT`)**:
  - `Cash50` (`0.50`) + `Top4` (`4` pares): `L_i = 12.50%` por pata (`$125.00` en `$1k` / `$250.00` en `$2k`).
  - `Uta75` (`0.75`) + `Top4` / `Top2`: `L_i = 18.75%` (`$187.50` / `$375.00`) en `Top4`, o `37.50%` (`$375.00` / `$750.00`) en `Top2`.
  - `Uta85` (`0.85`) + `Top4` / `Top2`: `L_i = 21.25%` (`$212.50` / `$425.00`) en `Top4`, o `42.50%` (`$425.00` / `$850.00`) en `Top2`.
  - Todas las piernas exceden entre `25×` y `170×` el mínimo de orden de `$5 USDT`.
- **Rendimiento anual absoluto esperado en dólares vs. costo de oportunidad**:
  - En la muestra completa `5y` (`2.90%–6.12%`): `$29.03–$61.15/año` sobre `$1,000` (`$58.06–$122.30/año` sobre `$2,000`).
  - En **`R2` (`Pass`: `2.35%–3.81%`)**: **`$23.53–$38.09/año`** sobre `$1,000` (`$47.06–$76.17/año` sobre `$2,000`).
  - En **`24m` (`0 / 9 Pass`: `0.33%–2.53%`)**: **`$3.31–$25.28/año`** sobre `$1,000` (`$6.62–$50.56/año` sobre `$2,000`).
  - Frente a rendimientos USD libres de riesgo (`~4%–5%` anual en Letras del Tesoro / *cash* de bajo riesgo), desplegar un motor multi-pata UTA Spot+Perp con altcoins volátiles para capturar `1.57%–3.81%` anual (antes de *haircuts* reales de colateral Spot en Bybit, picos horarios de préstamo USDT, *leg-risk* asíncrono y complejidad operativa) ofrece **utilidad económica neta negativa ajustada por esfuerzo y riesgo operacional**.

---

## 6. Catálogo de Tests + Salida Cruda Sin Editar de las 6 Compuertas PowerShell + `git status --short`

### 6.1 Catálogo Resumen de Pruebas de Integración ([`tests/carry_uta_tests.rs`](../tests/carry_uta_tests.rs), `6 / 6 ok`; `47 / 47` total)

| # | Test (`archivo:línea`) | Alcance Verificado | Estado |
|---:|---|---|---|
| 1 | `payback_apr_hurdle_and_cost_amortized_rotation` (`L163-306`) | Fórmula cerrada `payback_apr_hurdle_pct` (`4.501666666666667%` a `18.50 bps` / `30d`), rechazos de fee negativo/`NaN`/`Inf`/`payback_days=0`, bloqueo de rotación bajo spread `+3.0% < hurdle`, protección `min_hold_days = 14`, reemplazo en día `14`, y salida obligatoria cuando `apr <= exit_apr_pct`. | **PASS** |
| 2 | `uta_cross_margin_borrow_and_recenter_physics` (`L308-494`) | Vectores cerrados sobre repunte `+40%/+30%`: `Uta75` (`usdt_buffer=0.25`, `unrealized=0.2625`, `borrowed_usdt_frac=0.0125`, `borrow_bps=0.0125*1500/365`, `recenter=1` en `+40% >= 35%`, `recenter_turn_l=0.75`), día siguiente plano (`0` borrow tras reset), `Cash50` (`0` borrow/recenter), `borrow=0%`, y disparo exacto `+20%` en `Uta85`. | **PASS** |
| 3 | `real_basis_bar_open_settlement_eof_and_maker_diag_non_binding` (`L496-731`) | Basis real `sum L_i*(r_spot - r_perp)*10^4`, `BarOpenSettlement` ignorando funding en `t=1` y EOF `t=3` con `eof_close_turn_l = total_covered_l`, tarifas `18.50` vs `12.00 bps`, y garantía doble de que `diag_zero_cells` (`is_binding=false`) nunca emite `GateVerdict::Pass` incluso cuando supera `t_nw >= 3.5` y `DSR >= 0.95`. | **PASS** |
| 4 | `zero_lookahead_bit_exact_and_late_spot_listing` (`L733-965`) | Mutación `10×` precios / `-500 bps` funding tras `t > 80`: igualdad bit-a-bit (`to_bits()`) de todos los `CarryUtaDayRecord` hasta `exit_ts_ns <= cutoff` en las `9` estrategias; listado tardío Spot en día `60` (`0` asignación antes de decisión día `60` / entrada día `61`, entrada posterior en las `9`). | **PASS** |
| 5 | `cli_scan_carry_uta_end_to_end_and_rejections` (`L967-1112`) | Ejecución CLI `scan-carry-uta --linear-dir --spot-dir --slippage-bps 1.5 --borrow-apr-pct 15.0 --out`, columnas ASCII (`BorrowBp`, `AvgUtil%`, `RecCnt`), roundtrip JSON `CarryUtaReport` (`9` celdas, `diag_zero` no vinculante), y rechazo tipado de flags faltantes/negativos/`NaN`/desconocidos. | **PASS** |
| 6 | `cash50_blind30d_replicates_phase4_topcarry30d` (`L1114-1220`) | Equivalencia bit-exacta entre Track 6 `Cash50_Blind30d` y Track 5 `Carry_SpotPerp_TopCarry30d` sobre serie de `100` días sin recenter/borrow: idénticos timestamps, pesos, `basis`, `fund`, `turnover`, `cost_vip0`, `net_vip0` y `net_maker`. | **PASS** |

### 6.2 Salida Cruda Sin Editar de las 6 Compuertas PowerShell + `git status` y `git diff --name-only`

> Salida literal sin editar capturada en PowerShell 5.1 (`D:\Desarrollo\VORTEX-ALFA`, archivo `steps/110/output.txt`) tras implementar Track 6, compilar en `--release` y ejecutar la suite completa de `47` tests. Al final se anexa el `git status --short` post-documentación confirmando cambios exclusivamente en `DOCS/`.

```text
5.1.26100.9818
=== GATE 1: cargo fmt ===
FMT_EXIT=0
=== GATE 2: cargo clippy ===
    Checking vortex-alfa v0.1.0 (D:\Desarrollo\VORTEX-ALFA)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.62s
CLIPPY_EXIT=0
=== GATE 3: cargo test ===
   Compiling vortex-alfa v0.1.0 (D:\Desarrollo\VORTEX-ALFA)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 9.15s
     Running unittests src\lib.rs (target\debug\deps\vortex_alfa-a216a8d1bd8b90ff.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\vortex_alfa-e53f0a43f8dc11a2.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\adversarial_math_tests.rs (target\debug\deps\adversarial_math_tests-1e7b3596f2b1b423.exe)

running 9 tests
test test_garman_klass_closed_form_and_flat_bar ... ok
test test_acklam_normal_inv_cdf_accuracy ... ok
test test_moments4_closed_form_exact ... ok
test test_newey_west_closed_form_n8 ... ok
test test_holm_bonferroni_order_and_monotonicity ... ok
test test_hawkes_o1_three_step_analytical ... ok
test test_dsr_cross_sectional_std_and_diag_never_passes ... ok
test test_vip0_ceil_fee_e8_and_book_roundtrip_asymmetry ... ok
test test_exact_funding_boundary_bar_open_vs_bar_close ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\carry_uta_tests.rs (target\debug\deps\carry_uta_tests-9b3f8b32f1f47eca.exe)

running 6 tests
test payback_apr_hurdle_and_cost_amortized_rotation ... ok
test uta_cross_margin_borrow_and_recenter_physics ... ok
test real_basis_bar_open_settlement_eof_and_maker_diag_non_binding ... ok
test cash50_blind30d_replicates_phase4_topcarry30d ... ok
test zero_lookahead_bit_exact_and_late_spot_listing ... ok
test cli_scan_carry_uta_end_to_end_and_rejections ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s

     Running tests\cross_sectional_tests.rs (target\debug\deps\cross_sectional_tests-a086dcf4759e346e.exe)

running 7 tests
test test_xs_residual_reversion_ols_beta_and_clamp_analytical ... ok
test test_xs_momentum_vol_adj_skip_and_gk_analytical ... ok
test test_xs_funding_carry_closed_form_and_ranking ... ok
test test_xs_edge_cases_zero_var_flat_bar_ties_insufficient_symbols_and_non_finite ... ok
test test_exact_turnover_funding_bar_open_eof_and_maker_never_passes ... ok
test test_zero_lookahead_end_to_end_track3_xs ... ok
test test_cli_scan_xs_csv_and_global27_report ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s

     Running tests\edge_cases_and_drift_tests.rs (target\debug\deps\edge_cases_and_drift_tests-2a9b90ac347a6026.exe)

running 8 tests
test test_flat_candle_garman_klass_zero ... ok
test test_dust_notional_crossed_book_and_invalid_ohlc_rejected ... ok
test test_zero_variance_never_passes_or_nans ... ok
test test_flash_crash_minus_30_percent_bar_and_tick ... ok
test test_equity_ledger_10000_trades_zero_drift ... ok
test test_duplicate_and_backward_timestamps_rejected ... ok
test test_corrupted_binary_formats_rejected ... ok
test test_reject_non_finite_across_all_components ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests\macro_track5_tests.rs (target\debug\deps\macro_track5_tests-ad877cc04813ebaa.exe)

running 6 tests
test test_equity_metrics_and_passive_baselines_closed_form ... ok
test test_macro_trend_and_donchian_closed_form_vectors ... ok
test test_spot_perp_carry_real_basis_vip0_fees_and_eof ... ok
test test_edge_cases_late_spot_flat_candles_and_invalid_inputs ... ok
test test_zero_lookahead_bit_for_bit_and_symbol_isolation ... ok
test test_cli_scan_macro_end_to_end_and_json_roundtrip ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

     Running tests\market_making_tests.rs (target\debug\deps\market_making_tests-c773f527f66d75db.exe)

running 6 tests
test test_mm_fifo_queue_conservation_and_5ms_adverse_latency ... ok
test test_mm_stoikov_ofi_tfi_as_and_postonly_analytical ... ok
test test_mm_markout_lvr_eof_liquidation_breakeven_and_zero_drift ... ok
test test_mm_diagnostic_frontier_never_passes_binding_gate ... ok
test test_mm_zero_lookahead_and_strict_symbol_isolation ... ok
test test_cli_scan_mm_multifile_par2_and_json_report ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Running tests\zero_lookahead_tests.rs (target\debug\deps\zero_lookahead_tests-68c127dd0d00bc59.exe)

running 5 tests
test test_zero_lookahead_end_to_end_track2_meso ... ok
test test_zero_lookahead_end_to_end_track1_micro ... ok
test test_strict_symbol_isolation_track2_meso ... ok
test test_strict_symbol_isolation_track1_micro ... ok
test test_multifile_par2_vpz1_vkl1_streaming_and_cli ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s

TEST_EXIT=0
=== GATE 4: line count <= 350 ===

File                                                              Lines Pass
----                                                              ----- ----
D:\Desarrollo\VORTEX-ALFA\src\lib.rs                                  7 True
D:\Desarrollo\VORTEX-ALFA\src\main.rs                                17 True
D:\Desarrollo\VORTEX-ALFA\src\application\carry_uta_accounting.rs   273 True
D:\Desarrollo\VORTEX-ALFA\src\application\carry_uta_types.rs        190 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_carry_uta.rs    212 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_macro.rs        259 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_meso.rs         330 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_micro.rs        280 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_mm.rs           337 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_xs.rs           334 True
D:\Desarrollo\VORTEX-ALFA\src\application\macro_accounting.rs       281 True
D:\Desarrollo\VORTEX-ALFA\src\application\macro_baselines.rs        345 True
D:\Desarrollo\VORTEX-ALFA\src\application\mm_accounting.rs          345 True
D:\Desarrollo\VORTEX-ALFA\src\application\mm_fifo_sim.rs            346 True
D:\Desarrollo\VORTEX-ALFA\src\application\mm_markout.rs             158 True
D:\Desarrollo\VORTEX-ALFA\src\application\mod.rs                     13 True
D:\Desarrollo\VORTEX-ALFA\src\domain\error.rs                        45 True
D:\Desarrollo\VORTEX-ALFA\src\domain\fees.rs                        179 True
D:\Desarrollo\VORTEX-ALFA\src\domain\ledger.rs                      113 True
D:\Desarrollo\VORTEX-ALFA\src\domain\mod.rs                           8 True
D:\Desarrollo\VORTEX-ALFA\src\domain\normal_dist.rs                 217 True
D:\Desarrollo\VORTEX-ALFA\src\domain\ports.rs                        18 True
D:\Desarrollo\VORTEX-ALFA\src\domain\stats.rs                       348 True
D:\Desarrollo\VORTEX-ALFA\src\domain\types.rs                       331 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\carry_uta.rs           260 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\cross_sectional.rs     306 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\macro_donchian.rs      192 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\macro_trend.rs         296 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\macro_vol.rs           151 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\market_making.rs       310 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\meso_energy.rs         331 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\micro_detectors.rs     226 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\micro_tail.rs          330 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\mod.rs                  10 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\spot_perp_carry.rs     241 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\binary_codec.rs        342 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\csv_candle_reader.rs   319 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\mod.rs                   4 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\tape_reader.rs         325 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\vkl1_reader.rs         154 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli.rs                     329 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli_carry_uta.rs           128 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli_macro.rs               158 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli_mm.rs                  136 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\mod.rs                       4 True


=== GATE 5: banned patterns ===
BANNED_RECURSIVE_COUNT=0
=== GATE 6: encoding hygiene ===

File                          UTF8_NoBOM LF_Only
----                          ---------- -------
lib.rs                              True    True
main.rs                             True    True
carry_uta_accounting.rs             True    True
carry_uta_types.rs                  True    True
falsifier_carry_uta.rs              True    True
falsifier_macro.rs                  True    True
falsifier_meso.rs                   True    True
falsifier_micro.rs                  True    True
falsifier_mm.rs                     True    True
falsifier_xs.rs                     True    True
macro_accounting.rs                 True    True
macro_baselines.rs                  True    True
mm_accounting.rs                    True    True
mm_fifo_sim.rs                      True    True
mm_markout.rs                       True    True
mod.rs                              True    True
error.rs                            True    True
fees.rs                             True    True
ledger.rs                           True    True
mod.rs                              True    True
normal_dist.rs                      True    True
ports.rs                            True    True
stats.rs                            True    True
types.rs                            True    True
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
binary_codec.rs                     True    True
csv_candle_reader.rs                True    True
mod.rs                              True    True
tape_reader.rs                      True    True
vkl1_reader.rs                      True    True
cli.rs                              True    True
cli_carry_uta.rs                    True    True
cli_macro.rs                        True    True
cli_mm.rs                           True    True
mod.rs                              True    True
adversarial_math_tests.rs           True    True
carry_uta_tests.rs                  True    True
cross_sectional_tests.rs            True    True
edge_cases_and_drift_tests.rs       True    True
macro_track5_tests.rs               True    True
market_making_tests.rs              True    True
zero_lookahead_tests.rs             True    True


=== GIT STATUS & DIFF ===
 M src/application/mod.rs
 M src/domain/signals/mod.rs
 M src/interfaces/cli.rs
 M src/interfaces/mod.rs
?? scripts/audit_carry30_datasets.py
?? scripts/fetch_bybit_carry30_5y.py
?? scripts/run_track6_sweep.ps1
?? src/application/carry_uta_accounting.rs
?? src/application/carry_uta_types.rs
?? src/application/falsifier_carry_uta.rs
?? src/domain/signals/carry_uta.rs
?? src/interfaces/cli_carry_uta.rs
?? tests/carry_uta_tests.rs
--- git diff --name-only ---
src/application/mod.rs
src/domain/signals/mod.rs
src/interfaces/cli.rs
src/interfaces/mod.rs
```

**Salida final verificada de `git status --short` (post-documentación, confirmando exclusión de `data_local/`):**

```text
 M DOCS/INDEX.md
 M src/application/mod.rs
 M src/domain/signals/mod.rs
 M src/interfaces/cli.rs
 M src/interfaces/mod.rs
?? DOCS/PHASE_5_TRACK6_CARRY_UTA_REPORT.md
?? scripts/audit_carry30_datasets.py
?? scripts/fetch_bybit_carry30_5y.py
?? scripts/run_track6_sweep.ps1
?? src/application/carry_uta_accounting.rs
?? src/application/carry_uta_types.rs
?? src/application/falsifier_carry_uta.rs
?? src/domain/signals/carry_uta.rs
?? src/interfaces/cli_carry_uta.rs
?? tests/carry_uta_tests.rs
```

---

## 7. Veredicto Frío Binario de Fase 5

- **Veredicto Binario de Despliegue**: **NO — DESCARTADO PARA DESPLIEGUE REAL EN CAPITAL PEQUEÑO (`$1,000–$2,000 USDT`)**.
- **Criterio de Falla Cuantitativa (`Verdict_VIP0 == Pass` AND `CAGR >= 6.0%` simultáneos)**: **`0 / 9`** configuraciones cumplen la condición conjunta en **`R2`** (`2024-05-25..2026-11-25`) ni en la cohorte **`24m`** (`2024-11-25..2026-11-25`):
  - En **`R2`**, aunque la rotación amortizada (`Amort30d`) rescata la significancia estadística (`5 / 9 Pass`), el mayor `CAGR%` de una celda aprobada es **`3.81%`** (`Uta75_Amort30d_Top2`, `AnnSR = 2.73`, `MaxDD = 0.59%`, `DSR = 0.9517`), seguido por **`3.41%`** (`Uta85_Amort30d_Top4`) y **`3.30%`** (`Uta75_Amort30d_Top4`); el techo nominal absoluto `Uta85_Amort30d_Top2` (`4.05%`) reprueba el filtro DSR (`0.9227 < 0.95`, `Fail`).
  - En **`24m`**, los **`9 / 9`** candidatos fallan (`InsufficientSample`, `N = 700 < n_min = 793..58,371`, `DSR <= 0.9352`) con un `CAGR%` máximo de apenas **`2.53%`** (`Uta85_Amort30d_Top2`; `1.57%–2.38%` en las demás variantes `Amort30d`), incluso reduciendo slippage a `0.5 bps` (`<= 2.57%`) o bajo `MakerDiag` (`<= 2.76%`).
- **Síntesis empírica acumulada de VORTEX-ALFA (Tracks 1–6)**:
  - Tracks 1 (`0/20`), 2 (`0/36`), 3 (`0/27`), 4 (`0/6`; `0/30` frontera) y las señales direccionales de Track 5 (`0/6`) quedaron falsados estadísticamente en `VIP-0`.
  - Track 5 (`Carry_SpotPerp_TopCarry30d`: `1/9`) y Track 6 (`9/9 Pass` en `5y`) confirman que el *funding carry* delta-neutral Spot–Perp con amortización de tarifas (`>= 30d`) es la única prima real detectable estadísticamente en Bybit `VIP-0` bajo datos históricos de 5 años.
  - Sin embargo, la compresión estructural de las primas de *funding* en 2024–2026 deja el rendimiento neto real (`~1.57%–3.81%` anual, o `$15.70–$38.10/año` por cada `$1,000 USDT`) **por debajo del costo de oportunidad libre de riesgo en USD (`~4%–5%`)**, antes incluso de descontar *haircuts* reales de colateral Spot altcoin en Bybit UTA, picos horarios de tasa de préstamo USDT y riesgo asíncrono de ejecución entre patas Spot/Perp.

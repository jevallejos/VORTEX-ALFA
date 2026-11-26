# VORTEX-ALFA — Phase 4 Track 5 Macro (`1d`) Empirical Falsification Report

> **Veredicto Empírico Global**: **`1 / 9 Pass`** en el barrido principal de `5` años (`VIP-0 + 1.5 bps` slippage, `M = 9`, `t_crit = 3.5000`) — **`Carry_SpotPerp_TopCarry30d` APRUEBA** en la muestra completa de `5` años (`N = 1,796 >= n_min = 199`, `mean = +0.811 bps/día`, `CAGR = 3.00%`, `AnnSR = 4.75`, `MaxDD = 0.55%`, `Calmar = 5.47`, `t_nw = 8.594 >= 3.5000`, `DSR = 0.9967 >= 0.95`), en sensibilidad `0.5 bps` (`DSR = 0.9990`) y `3.0 bps` (`DSR = 0.9773`), y en Régimen 1 `R1` (`t_nw = 7.363`, `DSR = 0.9999`), pero **no aprueba de forma independiente en Régimen 2 `R2`** (`0 / 9 Pass` en `R2`: `mean = +0.3665 bps/día`, `t_nw = 3.6946`, pero `N = 884 < n_min = 1,106` y `DSR = 0.3216` por compresión de funding). Las `6` variantes direccionales (`VolTargetTrend` × 3 y `DonchianBreakout` × 3) junto con `Carry_SpotPerp_10Apr` y `Carry_SpotPerp_15Apr` quedan en **`0 / 8 Pass`** (`2 Fail`, `6 InsufficientSample`).
> **Engine**: `target\release\vortex-alfa.exe scan-macro` ([`src/application/falsifier_macro.rs`](../src/application/falsifier_macro.rs), [`src/interfaces/cli_macro.rs`](../src/interfaces/cli_macro.rs))
> **Universo & Timeframe**: `12` pares mayores Bybit V5 (`0:BTCUSDT .. 11:ATOMUSDT`) en dual **`linear` + `spot`**, velas cerradas **`1d`** (`interval = D`)
> **Ventana Histórica UTC**: `2021-11-25T00:00:00Z` (`1637798400000`) a `2026-11-25T00:00:00Z` (`1795564800000`), **`5` años exactos** (`1,826` barras esperadas por serie; `21,912` velas `linear` + `21,807` velas `spot` = `43,719` eventos en `1,826` slices diarios)
> **Partición Bi-Régimen OOS**: **`R1`** `2021-11-25T00:00:00Z`–`2024-05-25T00:00:00Z` (`912` días) | **`R2`** `2024-05-25T00:00:00Z`–`2026-11-25T00:00:00Z` (`914` días)
> **Fricción Vinculante (`VIP-0 + 1.5 bps` slip) vs. `VIP0-MakerDiag`**: Direccionales Perp: `5.50 + 1.50 = 7.00 bps`/turn (`MakerDiag: 2.00 bps`); Dual Spot–Perp Carry: `(10.00 + 1.50) + (5.50 + 1.50) = 18.50 bps`/`turn_L` (`MakerDiag: 10.00 + 2.00 = 12.00 bps`/`turn_L`, pues Spot VIP-0 cobra `10.00 bps` tanto en Maker como en Taker); Spot Pasivo: `11.50 bps`/turn (`MakerDiag: 10.00 bps`).
> **Compuertas de Código**: `6 / 6` compuertas PS en verde (`41/41` tests `ok`, `40/40` archivos `src/**/*.rs <= 350` líneas, `0` warnings Clippy, `0` patrones prohibidos)
> **Index**: [`DOCS/INDEX.md`](./INDEX.md) | [`DOCS/PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md) | [`DOCS/PHASE_1A_TRACK2_FALSIFICATION_REPORT.md`](./PHASE_1A_TRACK2_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_1B_TRACK1_FALSIFICATION_REPORT.md`](./PHASE_1B_TRACK1_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md`](./PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_3A_TRACK4_MM_DESIGN_REPORT.md`](./PHASE_3A_TRACK4_MM_DESIGN_REPORT.md) | [`DOCS/PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md`](./PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md)

---

## 0. Conformidad con las 4 Leyes Inviolables ("Las 4 D") en Fase 4

| Ley | Regla | Evidencia Verificada en Código Fuente y Auditoría | Estado |
|---|---|---|:---:|
| **Ley 1 — Cero Duplicación (SSOT / DRY)** | Reutilizar primitivas del dominio (`types`, `fees`, `stats`, `garman_klass_vol`) y lector `CsvCandleReader`; cero cálculos estadísticos o financieros en Python/PS | [`macro_trend.rs:134`](../src/domain/signals/macro_trend.rs#L134) reutiliza [`garman_klass_vol`](../src/domain/signals/meso_energy.rs#L33-L54); [`macro_accounting.rs:35-45`](../src/application/macro_accounting.rs#L35-L45) reutiliza [`FeeSchedule::bybit_vip0`](../src/domain/fees.rs#L24-L31) y [`FeeSchedule::custom`](../src/domain/fees.rs#L45-L60); [`falsifier_macro.rs:137-138`](../src/application/falsifier_macro.rs#L137-L138), [`macro_accounting.rs:264-269`](../src/application/macro_accounting.rs#L264-L269) y [`macro_baselines.rs:153,294-298`](../src/application/macro_baselines.rs#L294-L298) reutilizan [`evaluate_hypothesis_batch`](../src/domain/stats.rs#L277-L348), `welford_mean`, `Moments4` y `newey_west`; [`cli_macro.rs:49-50`](../src/interfaces/cli_macro.rs#L49-L50) reutiliza [`CsvCandleReader`](../src/infrastructure/csv_candle_reader.rs). [`fetch_bybit_macro5y.py`](../scripts/fetch_bybit_macro5y.py), [`audit_macro5y_datasets.py`](../scripts/audit_macro5y_datasets.py) y [`run_track5_sweep.ps1`](../scripts/run_track5_sweep.ps1) no calculan señales, rotación, fees, Newey-West ni DSR. | **PASS** |
| **Ley 2 — Cero Desviación (Strict Scope Lock)** | Prohibido modificar Tracks 1–4, `stats.rs`, `fees.rs`, `ledger.rs`, `types.rs`, `Cargo.toml` o los `35` tests previos | `git diff --name-only` confirma únicamente los `4` archivos de registro mínimo: [`src/application/mod.rs`](../src/application/mod.rs), [`src/domain/signals/mod.rs`](../src/domain/signals/mod.rs), [`src/interfaces/cli.rs`](../src/interfaces/cli.rs) (`scan-macro` en [L54, L64](../src/interfaces/cli.rs#L54)) y [`src/interfaces/mod.rs`](../src/interfaces/mod.rs). Track 5 agrega `8` módulos nuevos en `src/` y [`tests/macro_track5_tests.rs`](../tests/macro_track5_tests.rs) (`6` tests; `41/41` tests totales `ok`). | **PASS** |
| **Ley 3 — Cero Diluvio (Zero Bloat `<= 350` líneas)** | Todos los `src/**/*.rs <= 350` líneas, `Cargo.toml` con `3` deps y scripts Python stdlib puro | Los `40/40` archivos en `src/**/*.rs` cumplen `<= 350` líneas (máximo global `348` en `stats.rs`): `macro_vol.rs` (`151`), `macro_trend.rs` (`296`), `macro_donchian.rs` (`192`), `spot_perp_carry.rs` (`241`), `macro_baselines.rs` (`345`), `macro_accounting.rs` (`281`), `falsifier_macro.rs` (`259`), `cli_macro.rs` (`158`), `cli.rs` (`328`). [`Cargo.toml`](../Cargo.toml) conserva exactamente `3` dependencias (`thiserror`, `serde`, `serde_json`); scripts Python en stdlib puro (`0` `pip install`). | **PASS** |
| **Ley 4 — Cero Disfraz (Zero Disguise)** | Costos duales reales Spot (`10.00 bps` Maker y Taker) + Perp (`5.50 bps` Taker) + slippage, basis real Spot vs. Perp, fechas reales sin relleno sintético y reporte completo (`9` + `2`) | `Carry_SpotPerp` evalúa el basis real `(r_spot - r_perp)` usando series reales `category=spot` vs `category=linear` a `18.50 bps` por rotación (`SPOT_VIP0_FEE_BPS = 10.00` en [`macro_accounting.rs:13,62-68`](../src/application/macro_accounting.rs#L62-L68)); `BNBUSDT` Spot inicia en su fecha real de listado `2022-03-10T00:00:00Z` (`105` barras iniciales ausentes sin síntesis de precios y excluido de Carry antes de esa fecha); `VIP0-MakerDiag` tiene `is_binding = false` y jamás emite `Pass`; se reportan íntegras las `9` celdas y los `2` benchmarks en `5` escenarios (`main`, `R1`, `R2`, `0.5`, `3.0 bps`), incluyendo el fracaso independiente en `R2` (`0 / 9 Pass`). | **PASS** |

---

## 1. Manifiesto de los 12 Pares (`5` Años `1d`: `2021-11-25` a `2026-11-25`) y Auditoría de Integridad

Todos los contratos perpetuos lineales completaron el **`100.0%`** de las `1,826` barras diarias esperadas (`21,912` velas `linear`), mientras que `11 / 12` pares Spot completaron el `100.0%` y `BNBUSDT` Spot registró cobertura del `94.2497%` desde su listado real el `2022-03-10T00:00:00Z` (`21,807` velas `spot`; `43,719` velas únicas en el conjunto completo; `87,438` filas auditadas a través de `72` archivos CSV entre `full`, `R1` y `R2` en [`data_local/bybit_macro5y_1d/audit_report.json`](../data_local/bybit_macro5y_1d/audit_report.json)).

| ID | Símbolo | `Linear` (Min..Max UTC · N · Cob% · Gaps) | `Spot` (Min..Max UTC · N · Cob% · LeadMiss · Gaps) | Dual Overlap (Full / R1 / R2) | Intervalos Funding | Raw Events | Sub-24h Agg. | Raw Sum (bps) | CSV Sum (bps) | `diff_bps` |
|---:|---|---|---|---|---|---:|---:|---:|---:|---|
| 0 | `BTCUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 · 0 | 1,826 (912 / 914) | `8.0h` | 5,478 | 3,652 | 3302.1228 | 3302.1228 | `0` |
| 1 | `ETHUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 · 0 | 1,826 (912 / 914) | `8.0h` | 5,478 | 3,652 | 3129.3518 | 3129.3518 | `0` |
| 2 | `SOLUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 · 0 | 1,826 (912 / 914) | `2.0h, 8.0h` | 5,838 | 4,012 | -1438.935 | -1438.935 | `0` |
| 3 | `BNBUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 | `2022-03-10..2026-11-24` · 1,721 · 94.2497% · 105 · 0 | 1,721 (807 / 914) | `8.0h` | 5,478 | 3,652 | -1453.4353 | -1453.4353 | `0` |
| 4 | `XRPUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 · 0 | 1,826 (912 / 914) | `8.0h` | 5,478 | 3,652 | 3580.0192 | 3580.0192 | `0` |
| 5 | `ADAUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 · 0 | 1,826 (912 / 914) | `8.0h` | 5,478 | 3,652 | 3278.9392 | 3278.9392 | `0` |
| 6 | `DOGEUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 · 0 | 1,826 (912 / 914) | `8.0h` | 5,478 | 3,652 | 4386.3871 | 4386.3871 | `0` |
| 7 | `LINKUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 · 0 | 1,826 (912 / 914) | `8.0h` | 5,478 | 3,652 | 4124.4648 | 4124.4648 | `0` |
| 8 | `AVAXUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 · 0 | 1,826 (912 / 914) | `8.0h` | 5,478 | 3,652 | 2320.8208 | 2320.8208 | `0` |
| 9 | `LTCUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 · 0 | 1,826 (912 / 914) | `8.0h` | 5,478 | 3,652 | 3860.7875 | 3860.7875 | `0` |
| 10 | `DOTUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 · 0 | 1,826 (912 / 914) | `8.0h` | 5,478 | 3,652 | 1409.5901 | 1409.5901 | `0` |
| 11 | `ATOMUSDT` | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 | `2021-11-25..2026-11-24` · 1,826 · 100.0% · 0 · 0 | 1,826 (912 / 914) | `8.0h` | 5,478 | 3,652 | 627.9238 | 627.9238 | `0` |

### Auditoría de Integridad & Reconciliación Causal de Funding (`BarOpenSettlement`)

- **Auditor (`scripts/audit_macro5y_datasets.py` -> `audit_report.json`) — Estado `PASS`**: `12` JSONs crudos de funding (`raw_funding_failures = 0`), `72` archivos CSV (`missing_files_total = 0`, `rows_checked = 87,438`), `nan_inf_total = 0`, `non_pos_price_total = 0`, `invalid_ohlc_total = 0`, `neg_vol_total = 0`, `nonzero_spot_funding_total = 0`, `duplicate_ts_total = 0`, `backward_ts_total = 0`, `wrong_sym_id_total = 0`, `out_of_window_total = 0`, `misaligned_ts_total = 0`, `header_failures = 0`, `encoding_failures = 0`, `gap_files_total = 0`, `gap_count_total = 0`, `missing_bars_total = 0`, `trailing_missing_bars_total = 0`, `leading_incomplete_files = 2` (`BNBUSDT` Spot en `full` y `r1`), `split_mismatch_total = 0`, `per_bar_funding_mismatch_total = 0`.
- **Agregación causal en cubetas semiabiertas `(bar_open_ts - 24h, bar_open_ts]`**: Se consolidaron `44,184` eventos sub-24h (incluyendo intervalos `[2.0, 8.0]h` observados en `SOLUSDT`, `5,838` eventos crudos frente a `5,478` en los demás pares) dentro de `1,826` cubetas diarias por símbolo con **`unmatched_funding_events_total = 0`** y **`funding_reconciliation_failures = 0`** (`diff_bps = 0` en los `12 / 12` pares).

> [!WARNING]
> **Advertencias Metodológicas y de Cobertura del Universo (`12` pares, `5` años)**:
> 1. **Listado tardío real de `BNBUSDT` Spot (`2022-03-10T00:00:00Z`)**: Mientras `BNBUSDT` Linear opera desde `2021-11-25`, el mercado `category=spot` de Bybit para `BNBUSDT` comienza el `2022-03-10T00:00:00Z` (`1,721 / 1,826` barras = `94.2497%`, `105` barras iniciales ausentes; `807 / 912` en `R1`, `914 / 914` en `R2`). En estricto acatamiento de la Ley 4, **no se sintetizaron precios** ni se sustituyó Spot por Linear: [`SpotPerpCarryEngine::on_slice`](../src/domain/signals/spot_perp_carry.rs#L156-L165) y [`open_trading_day`](../src/application/macro_accounting.rs#L140-L143) excluyen a `BNBUSDT` de `SpotPerpCarry` antes del `2022-03-10`, mientras `Baseline_BuyHold_EW` lo incorpora al listarse pagando la rotación real correspondiente (`macro_baselines.rs:260-272`).
> 2. **Sesgo residual de supervivencia**: Los `12` pares mayores (`BTCUSDT..ATOMUSDT`) fueron seleccionados condicionalmente sobre activos líquidos que sobrevivieron activos en Bybit V5 hasta `2026-11-25`, excluyendo por construcción tokens colapsados o deslistados del ciclo 2021–2026 (como antiguos top-cap de 2021). Por ello, los estimadores positivos observados constituyen una **cota superior optimista** frente a un universo libre de sesgo de supervivencia; el fracaso de las `6` estrategias direccionales (`0 / 6 Pass`) ocurre incluso bajo esta ventaja, y cualquier viabilidad de `Carry_SpotPerp_TopCarry30d` debe interpretarse considerando la compresión estructural del funding observada en `R2`.

---

## 2. Trazabilidad `Ecuación (LaTeX) -> archivo:línea` (Track 5 Macro `1d`)

| Concepto / Ecuación (LaTeX) | Ubicación Exacta (`archivo:línea`) | Análisis Dimensional | Guardas de Dominio y Numéricas |
|---|---|---|---|
| **Volatilidad Causal EWMA (`span=20`, seed muestral) y GK diaria**: $\hat{v}_{20} = \frac{1}{19}\sum_{j=1}^{20}(r_j - \bar{r})^2$, $\hat{v}_t = (1-\alpha)\hat{v}_{t-1} + \alpha r_t^2$ ($\alpha=\frac{2}{21}$), $\sigma_{\text{ann},t} = \text{clamp}\!\left(\sqrt{365\,\hat{v}_t},\, 0.15,\, 3.00\right)$ | [`src/domain/signals/macro_vol.rs:8-30, 48-100`](../src/domain/signals/macro_vol.rs#L48-L100) (`update` `L67-92`, `current_sigma_ann` `L95-100`); `garman_klass_vol` en [`macro_trend.rs:134`](../src/domain/signals/macro_trend.rs#L134) $\to$ [`meso_energy.rs:33-54`](../src/domain/signals/meso_energy.rs#L33-L54) | $r_t = \ln(C_t/C_{t-1})$ adimensional; $\sigma_{\text{ann},t} \in [\text{frac}/\sqrt{\text{año}}]$ con `days_per_year = 365.0` | Warmup estricto de `20` retornos diarios (`None` antes de `20` retornos / `21` barras); clamp `[0.15, 3.00]`; rechaza retornos no finitos (`NonFiniteValue`). |
| **`MacroVolTargetTrend` (`20/60/120d`, vol-target y banda `0.02`)**: $S_{\text{trend},i,t} = \frac{1}{3}\sum_{h\in\{20,60,120\}} \text{sgn}\!\left(\ln\frac{C_{i,t}}{C_{i,t-h}}\right)$, $w_{i,t}^{\text{raw}} = \frac{d_{i,t}}{N_{\text{act}}}\frac{\sigma_{\text{tgt}}}{\sigma_{\text{ann},i,t}}$, banda $\lvert w^{\text{tgt}}-w_{t-1} \rvert \ge 0.02$ | [`src/domain/signals/macro_trend.rs:17-73, 205-295`](../src/domain/signals/macro_trend.rs#L205-L295) (`TREND_HORIZONS` `L17`, `default_trend_params` `L67-73`, `compute_multi_horizon_signal` `L205-224`, `evaluate` `L249-295`); [`macro_vol.rs:115-146`](../src/domain/signals/macro_vol.rs#L115-L146) | $S_{\text{trend}} \in [-1, 1]$ adimensional; $w_{i,t}$ fracción del capital; $\sum_i \lvert w_{i,t} \rvert \le L_{\max}$ | Requiere `121` barras diarias (`120d` lookback); `LongOnly` recorta `signal.max(0.0)`; banda anti-churn retiene peso previo bit-a-bit si cambio `< 0.02`; reescala si excede `max_gross_leverage` (`1.0` o `1.5`). |
| **`DonchianBreakout` (Canales estrictamente previos `[t-n..=t-1]`)**: $U_{\text{in},t} = \max_{k\in[t-n_{\text{in}},\,t-1]} H_k$, $L_{\text{in},t} = \min_{k\in[t-n_{\text{in}},\,t-1]} L_k$ (análogo $n_{\text{out}}$), tamaño $w_{i,t}^{\text{raw}} = \frac{p_{i,t}}{N_{\text{act}}}\frac{0.20}{\sigma_{\text{ann},i,t}}$ | [`src/domain/signals/macro_trend.rs:183-202`](../src/domain/signals/macro_trend.rs#L183-L202) (`prior_high_low_bounds`); [`src/domain/signals/macro_donchian.rs:45-88, 126-191`](../src/domain/signals/macro_donchian.rs#L126-L191) (`default_donchian_params` `L62-68`, `prior_donchian_channel` `L71-88`, `evaluate` `L126-191`) | Precios $H_k, L_k, C_t$ en $[\text{USDT}]$; $p_{i,t} \in \{-1, 0, +1\}$ (`{0, +1}` en `LO`); $\sigma_{\text{tgt}} = 0.20$, $L_{\max} = 1.0$ | `range(end_idx - lookback_bars..end_idx)` excluye estrictamente la barra `t` (cero lookahead intrabarra); requiere `exit_n <= entry_n`; rompe a largo si `C_t > U_in`, corto si `!long_only && C_t < L_in`, cierra si cruza `(L_out, U_out)`. |
| **`SpotPerpCarryEngine` (APR rolling, histéresis, cadencia y cobertura `0.5 * alloc`)**: $\text{APR}_i(t) = \frac{365}{100 \cdot W}\sum_{j=0}^{W-1} f_{i,t-j}\;[\%]$, $L_i = \frac{0.5}{\max(\lvert S_{\text{act}} \rvert, 2)}$ | [`src/domain/signals/spot_perp_carry.rs:37-62, 115-237`](../src/domain/signals/spot_perp_carry.rs#L115-L237) (`default_carry_params` `L56-62`, `on_slice` `L115-227`, `window_apr_pct` `L230-237`) | $f_{i}$ en $[\text{bps/día}]$; $\text{APR}_i$ en $[\%]$; $L_i \in [0, 0.25]$ nocional cubierto por pata ($\sum 2L_i \le 1.0$) | Exige `linear.contains_key && spot.contains_key && window.len() == W` (excluye `BNBUSDT` antes de `2022-03-10`); histéresis `entry_apr_pct > exit_apr_pct`; desempate determinista por `SymbolId` ascendente; `max_pairs = 4`; cadencia `min_rebalance_days` (`1d` vs `30d`). |
| **Basis Real Spot–Perp, Funding `BarOpenSettlement` y Costos Duales VIP-0**:<br>• Perp: $R_{\text{net},t} = \sum_i w_i r_{i,\text{lin}}\cdot 10^4 - \sum_i w_i f_{i,\text{exit}} - \text{Turn}_t \cdot c_{\text{perp}}$<br>• Carry: $R_{\text{net},t} = \sum_i L_i (r_{i,\text{spot}} - r_{i,\text{lin}})\cdot 10^4 + \sum_i L_i f_{i,\text{exit}} - \text{Turn}_{L,t} \cdot c_{\text{carry}}$ | [`src/application/falsifier_macro.rs:83-118`](../src/application/falsifier_macro.rs#L83-L118) (`Open(t+1)` y EOF); [`src/application/macro_accounting.rs:13-69, 123-252`](../src/application/macro_accounting.rs#L174-L252) (`SPOT_VIP0_FEE_BPS = 10.00` `L13`, `MacroFeeProfiles` `L22-69`, `open_trading_day` `L123-171`, `settle_trading_day` `L174-252`) | Todos en $[\text{bps}]$ sobre capital; $c_{\text{perp,VIP0}} = 5.50 + \text{slip}$ (`7.00`), $c_{\text{perp,Mkr}} = 2.00$; $c_{\text{carry,VIP0}} = 10.00 + 5.50 + 2\cdot\text{slip}$ (`18.50`), $c_{\text{carry,Mkr}} = 12.00$ | Decisión al cierre `t`, apertura en `Open(t+1)`; en `BarOpenSettlement` ([`L196-200`](../src/application/macro_accounting.rs#L196-L200)), el funding del intervalo `(t, t+1]` se cobra al abrir `t+2` (ignora barra EOF al cerrar en `Close(T)`) y añade `close_turnover = \sum \lvert w_i \rvert` en EOF ([`L215`](../src/application/macro_accounting.rs#L215)). |
| **Curva de Capital (`CAGR%`, `AnnSR`, `MaxDD%`, `Calmar`) y Spot Benchmarks**: $E_t = \max(0, E_{t-1}(1 + R_{\text{net},t}/10^4))$, $\text{CAGR\%} = (E_T^{365/N}-1)\cdot 100$, $\text{SR}_{\text{ann}} = \frac{\mu}{\sigma}\sqrt{365}$ | [`src/application/macro_baselines.rs:132-345`](../src/application/macro_baselines.rs#L132-L345) (`compute_equity_metrics` `L132-167`, `compute_passive_baselines` `L169-198`, `run_baseline` `L220-302`, `weight_turnover` `L334-345`) | $E_t$ adimensional (`E_0 = 1.0`); `CAGR%`, `MaxDD%` en $[\%]$; $\text{SR}_{\text{ann}}$, `Calmar` adimensionales | Renta pasiva `BuyHold_BTC` y `BuyHold_EW` sigue pesos *drifted* entre días y solo paga `11.50 bps` (`10.00` Maker) al entrar, al cambiar el universo (listado Spot de `BNBUSDT`) y al cierre EOF. |

---

## 3. Tabla Completa del Barrido Principal (`5` Años, `VIP-0 + 1.5 bps` slip, `M = 9`, `t_crit = 3.5000`)

Ejecutado mediante [`scripts/run_track5_sweep.ps1`](../scripts/run_track5_sweep.ps1) sobre [`data_local/bybit_macro5y_1d`](../data_local/bybit_macro5y_1d) con `target\release\vortex-alfa.exe scan-macro --slippage-bps 1.5`:

- **Cabecera de lote observada ([`main_5y_slip1p5.json`](../data_local/reports_track5/main_5y_slip1p5.json))**: `slices_processed = 1,826` | `linear_candles_processed = 21,912` | `spot_candles_processed = 21,807` | `events_processed = 43,719` | `days_per_year = 365.0` | `slippage_bps = 1.5` | `M = 9` | `t_crit = 3.5000` (Holm-Bonferroni con piso `3.50` en [`t_crit_holm`](../src/domain/stats.rs#L131-L137)) | `sr_cross_std_vip0 = 0.097932` (`0.0979`) | `sr_cross_std_diag = 0.095481` (`0.0955`).

| Strategy | N_days | n_min | Price/BasisBps | FundBps | AvgDailyTurn | NetVIP0_bps | MakerDiag_bps | CAGR% | AnnSharpe | MaxDD% | Calmar | t_NW | t_crit | DSR | Verdict_VIP0 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `MacroTrend_LS_Vol15` | 1,705 | 10,460 | +1.666 | -0.130 | 0.014 | +1.437 | +1.507 | 5.05% | 0.65 | 12.01% | 0.42 | 1.405 | 3.5000 | 0.0000 | `InsufficientSample` |
| `MacroTrend_LS_Vol30` | 1,705 | 11,433 | +3.739 | -0.356 | 0.058 | +2.978 | +3.267 | 9.82% | 0.63 | 24.39% | 0.40 | 1.363 | 3.5000 | 0.0000 | `InsufficientSample` |
| `MacroTrend_LO_Vol20` | 1,705 | 18,586 | +1.437 | -0.271 | 0.007 | +1.117 | +1.152 | 3.80% | 0.49 | 10.72% | 0.35 | 1.024 | 3.5000 | 0.0000 | `InsufficientSample` |
| `Donchian_20_10_LS` | 1,805 | 462,498 | +0.764 | -0.228 | 0.030 | +0.329 | +0.477 | 0.46% | 0.10 | 23.23% | 0.02 | 0.211 | 3.5000 | 0.0000 | `InsufficientSample` |
| `Donchian_55_20_LS` | 1,770 | `-` | +0.125 | -0.232 | 0.018 | -0.234 | -0.144 | -1.47% | -0.08 | 24.28% | -0.06 | -0.172 | 3.5000 | 0.0000 | `Fail` |
| `Donchian_55_20_LO` | 1,770 | 338,005 | +0.543 | -0.237 | 0.009 | +0.245 | +0.289 | 0.59% | 0.12 | 13.73% | 0.04 | 0.243 | 3.5000 | 0.0000 | `InsufficientSample` |
| `Carry_SpotPerp_10Apr` | 1,819 | `-` | -0.017 | +1.059 | 0.085 | -0.537 | +0.018 | -1.94% | -2.54 | 9.51% | -0.20 | -4.507 | 3.5000 | 0.0000 | `Fail` |
| `Carry_SpotPerp_15Apr` | 1,819 | 13,773 | +0.012 | +0.677 | 0.033 | +0.083 | +0.296 | 0.30% | 0.57 | 2.53% | 0.12 | 0.933 | 3.5000 | 0.0000 | `InsufficientSample` |
| **`Carry_SpotPerp_TopCarry30d`** | **1,796** | **199** | **-0.015** | **+1.062** | **0.013** | **+0.811** | **+0.894** | **3.00%** | **4.75** | **0.55%** | **5.47** | **8.594** | **3.5000** | **0.9967** | **`Pass`** |
| `Baseline_BuyHold_BTC` *(benchmark)* | 1,826 | `-` | +3.779 | 0.000 | 0.0011 | +3.766 | +3.768 | 0.87% | 0.27 | 73.19% | 0.01 | 0.604 | `-` | `-` | *Benchmark* |
| `Baseline_BuyHold_EW` *(benchmark)* | 1,826 | `-` | +0.373 | 0.000 | 0.0012 | +0.358 | +0.360 | -17.76% | 0.02 | 76.80% | -0.23 | 0.047 | `-` | `-` | *Benchmark* |

**Hallazgos estructurales del barrido principal (`5` años):**
- **Aprobación única (`1 / 9 Pass`)**: `Carry_SpotPerp_TopCarry30d` satisface los tres filtros vinculantes simultáneamente: $N = 1,796 \ge n_{\min} = 199$, $t_{\text{NW}} = 8.594 \ge 3.5000$ ($p_{\text{Holm}} = 7.56\times 10^{-17}$), y $\text{DSR} = 0.9967 \ge 0.95$. La cadencia `min_rebalance_days = 30` mantiene `AvgDailyTurn = 0.0128` (`mean_cost_vip0 = 0.237 bps/día`), reteniendo el `+1.062 bps/día` de funding bruto frente a un *basis* medio de `-0.015 bps/día` y logrando `final_equity = 1.1567` (`+3.00%` CAGR) con `MaxDD = 0.55%` y `AnnSR = 4.75`.
- **Contraste frente a rotación diaria en Carry (`10Apr` y `15Apr`)**: `Carry_SpotPerp_10Apr` recolecta el mismo funding bruto (`+1.059 bps/día`), pero con `min_rebalance_days = 1` dispara su rotación diaria `6.66×` (`AvgDailyTurn = 0.0853`, `mean_cost_vip0 = 1.579 bps/día` a `18.50 bps`/turn), destruyendo la ganancia neta hasta `-0.537 bps/día` (`CAGR = -1.94%`, `t_nw = -4.507`, `Fail`). `Carry_SpotPerp_15Apr` reduce la rotación (`0.0328`), pero el costo (`0.607 bps/día`) deja apenas `+0.083 bps/día` (`t_nw = 0.933`, `n_min = 13,773`).
- **Falsación `0 / 6` de las familias direccionales (`VolTargetTrend` y `DonchianBreakout`)**: Aunque `MacroTrend_LS_Vol30` logra `+9.82%` CAGR nominal, su volatilidad diaria (`std = 90.98 bps`) limita su `AnnSharpe` a `0.63` (`MaxDD = 24.39%`, `t_nw = 1.363`, `n_min = 11,433 >> 1,705`, `DSR = 6.83e-7`). Las `3` variantes `DonchianBreakout` quedan entre `-0.234` y `+0.329 bps/día` (`t_nw <= 0.243`). Asimismo, bajo `VIP0-MakerDiag` (`is_binding = false`), **ninguna** de las `6` estrategias direccionales alcanza `t_nw >= 3.5` (máximo `1.497`) ni `DSR >= 0.95` (máximo `3.16e-6`).
- **Arrastre por volatilidad (*volatility drag*) en `Baseline_BuyHold_EW`**: Pese a una media aritmética diaria levemente positiva (`+0.358 bps/día`), la dispersión extrema de los 12 pares Spot produce una caída geométrica severa (`final_equity = 0.3759`, `CAGR = -17.76%`, `MaxDD = 76.80%`), mientras `Baseline_BuyHold_BTC` obtiene `0.87%` CAGR sufriendo un `MaxDD` de `73.19%` (`AnnSR = 0.27`).

---

## 4. Matriz de Estabilidad Bi-Régimen OOS (`R1` vs. `R2`)

Comparación exacta entre **`R1`** (`2021-11-25` a `2024-05-25`, `912` slices, `10,944` linear / `10,839` spot, `sr_std(VIP0) = 0.1129`, [`oos_r1_slip1p5.json`](../data_local/reports_track5/oos_r1_slip1p5.json)) y **`R2`** (`2024-05-25` a `2026-11-25`, `914` slices, `10,968` linear / `10,968` spot, `sr_std(VIP0) = 0.0834`, [`oos_r2_slip1p5.json`](../data_local/reports_track5/oos_r2_slip1p5.json)), ambos bajo `VIP-0 + 1.5 bps` slippage (`t_crit = 3.5000`):

| Strategy / Benchmark | R1 N | R1 n_min | R1 NetVIP0 | R1 CAGR% | R1 AnnSR | R1 MaxDD% | R1 t_NW | R1 DSR | R1 Verdict | R2 N | R2 n_min | R2 NetVIP0 | R2 CAGR% | R2 AnnSR | R2 MaxDD% | R2 t_NW | R2 DSR | R2 Verdict |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `MacroTrend_LS_Vol15` | 791 | 22,852 | +0.923 | 3.13% | 0.44 | 12.01% | 0.649 | 0.0000 | `InsufficientSample` | 793 | 6,147 | +1.951 | 7.01% | 0.85 | 5.02% | 1.248 | 0.0102 | `InsufficientSample` |
| `MacroTrend_LS_Vol30` | 791 | 13,915 | +2.727 | 8.78% | 0.57 | 24.39% | 0.844 | 0.0000 | `InsufficientSample` | 793 | 6,843 | +3.730 | 12.97% | 0.81 | 11.33% | 1.192 | 0.0085 | `InsufficientSample` |
| `MacroTrend_LO_Vol20` | 791 | 5,814 | +1.998 | 7.19% | 0.88 | 6.56% | 1.299 | 0.0002 | `InsufficientSample` | 793 | 52,508 | +0.573 | 1.85% | 0.29 | 9.42% | 0.411 | 0.0009 | `InsufficientSample` |
| `Donchian_20_10_LS` | 891 | 132,241 | +0.603 | 1.50% | 0.18 | 18.09% | 0.276 | 0.0000 | `InsufficientSample` | 893 | 21,829,658 | +0.049 | -0.59% | 0.01 | 19.63% | 0.022 | 0.0001 | `InsufficientSample` |
| `Donchian_55_20_LS` | 856 | `-` | -0.619 | -2.81% | -0.21 | 24.28% | -0.311 | 0.0000 | `Fail` | 858 | 2,822,174 | +0.126 | -0.21% | 0.04 | 15.34% | 0.065 | 0.0001 | `InsufficientSample` |
| `Donchian_55_20_LO` | 856 | 38,775 | +0.737 | 2.41% | 0.34 | 11.50% | 0.508 | 0.0000 | `InsufficientSample` | 858 | `-` | -0.098 | -0.67% | -0.05 | 13.73% | -0.068 | 0.0001 | `Fail` |
| `Carry_SpotPerp_10Apr` | 905 | `-` | -0.410 | -1.49% | -1.69 | 5.09% | -2.207 | 0.0000 | `Fail` | 907 | `-` | -0.675 | -2.44% | -3.83 | 6.15% | -4.874 | 0.0000 | `Fail` |
| `Carry_SpotPerp_15Apr` | 905 | 1,680 | +0.288 | 1.05% | 1.63 | 1.10% | 1.936 | 0.0054 | `InsufficientSample` | 907 | `-` | -0.038 | -0.14% | -0.36 | 1.58% | -0.445 | 0.0000 | `Fail` |
| **`Carry_SpotPerp_TopCarry30d`** | **882** | **118** | **+1.110** | **4.13%** | **6.16** | **0.34%** | **7.363** | **0.9999** | **`Pass`** | **884** | **1,106** | **+0.367** | **1.34%** | **2.01** | **0.59%** | **3.695** | **0.3216** | **`InsufficientSample`** |
| `Baseline_BuyHold_BTC` | 912 | `-` | +6.300 | 7.45% | 0.41 | 73.19% | 0.641 | `-` | *Benchmark* | 914 | `-` | +1.213 | -5.38% | 0.10 | 52.97% | 0.160 | `-` | *Benchmark* |
| `Baseline_BuyHold_EW` | 912 | `-` | +1.384 | -16.74% | 0.07 | 76.80% | 0.125 | `-` | *Benchmark* | 914 | `-` | -1.253 | -22.10% | -0.07 | 72.04% | -0.114 | `-` | *Benchmark* |

### Diagnóstico de Compresión de Funding entre `R1` y `R2` en `Carry_SpotPerp_TopCarry30d`

- **Descomposición exacta `R1` vs. `R2`**:
  - En **`R1` (`Pass`)**: `mean_fund_bps = +1.4276 bps/día`, `mean_price_or_basis_bps = -0.0344`, `avg_daily_turnover = 0.01531` (`mean_cost_vip0_bps = 0.2832`), produciendo `mean_net_vip0_bps = +1.1100 bps/día` (`std = 3.4443`, `sr_hat = 0.3223`, `t_nw = 7.3634 >= 3.5000`, `DSR = 0.9999 >= 0.95`, `N = 882 >= n_min = 118`).
  - En **`R2` (`InsufficientSample`)**: La estrategia permanece netamente positiva (`mean_net_vip0_bps = +0.3665 bps/día`, `CAGR = +1.34%`, `AnnSR = 2.01`, `MaxDD = 0.59%`) e incluso **supera el umbral Holm-Bonferroni** (`t_nw = 3.6946 >= 3.5000`, `p_holm = 0.00176`). Sin embargo, el funding bruto capturado cae un **`-53.7%`** (`+1.4276 -> +0.6604 bps/día`) mientras la fricción de rotación permanece prácticamente constante (`0.2832 -> 0.2982 bps/día`, `turn = 0.01612`), lo que reduce `sr_hat` de `0.3223` a `0.1053` (`std = 3.4818`). En consecuencia, el tamaño muestral mínimo exigido sube a **`n_min = 1,106 > N = 884`** y la razón de Sharpe deflacionada cae a **`DSR = 0.3216 < 0.95`** (incluso bajo `VIP0-MakerDiag` en `R2`, `DSR = 0.8354 < 0.95` con `t_nw = 5.162`).
- **Lectura honesta de régimen**: `Carry_SpotPerp_TopCarry30d` no invirtió signo ni perdió dinero en `R2` (a diferencia de `BuyHold_BTC`, `-5.38%` CAGR, y `BuyHold_EW`, `-22.10%` CAGR), pero la compresión de primas de *funding* en 2024–2026 redujo la relación señal/ruido diaria a un nivel que exige más de `3` años adicionales (`n_min = 1,106` días) para validar significancia DSR independiente dentro del régimen reciente.

---

## 5. Sensibilidad de Slippage (`0.5 / 1.5 / 3.0 bps`) y Viabilidad Operativa (`$1,000–$2,000 USDT`)

### 5.1 Sensibilidad Completa en `5` Años ([`sens_5y_slip0p5.json`](../data_local/reports_track5/sens_5y_slip0p5.json), [`main_5y_slip1p5.json`](../data_local/reports_track5/main_5y_slip1p5.json), [`sens_5y_slip3p0.json`](../data_local/reports_track5/sens_5y_slip3p0.json))

| Strategy / Benchmark | Costo VIP-0 / Turn | VIP0 (`0.5 bps`) | VIP0 (`1.5 bps*`) | VIP0 (`3.0 bps`) | MakerDiag (`1.5*` ref.) |
|---|---|---|---|---|---|
| `MacroTrend_LS_Vol15` | `6.0 / 7.0* / 8.5` (`2.0`) | +1.451 (`5.10%`, `t=1.419`, `DSR=0.0000`, `Insuff`) | +1.437 (`5.05%`, `t=1.405`, `DSR=0.0000`, `Insuff`) | +1.416 (`4.97%`, `t=1.384`, `DSR=0.0000`, `Insuff`) | +1.507 (`5.32%`, `t=1.475`, `DSR=0.0000`, `Insuff`) |
| `MacroTrend_LS_Vol30` | `6.0 / 7.0* / 8.5` (`2.0`) | +3.036 (`10.05%`, `t=1.390`, `DSR=0.0000`, `Insuff`) | +2.978 (`9.82%`, `t=1.363`, `DSR=0.0000`, `Insuff`) | +2.891 (`9.47%`, `t=1.323`, `DSR=0.0000`, `Insuff`) | +3.267 (`10.98%`, `t=1.497`, `DSR=0.0000`, `Insuff`) |
| `MacroTrend_LO_Vol20` | `6.0 / 7.0* / 8.5` (`2.0`) | +1.124 (`3.83%`, `t=1.030`, `DSR=0.0000`, `Insuff`) | +1.117 (`3.80%`, `t=1.024`, `DSR=0.0000`, `Insuff`) | +1.106 (`3.76%`, `t=1.014`, `DSR=0.0000`, `Insuff`) | +1.152 (`3.93%`, `t=1.056`, `DSR=0.0000`, `Insuff`) |
| `Donchian_20_10_LS` | `6.0 / 7.0* / 8.5` (`2.0`) | +0.359 (`0.57%`, `t=0.230`, `DSR=0.0000`, `Insuff`) | +0.329 (`0.46%`, `t=0.211`, `DSR=0.0000`, `Insuff`) | +0.285 (`0.29%`, `t=0.182`, `DSR=0.0000`, `Insuff`) | +0.477 (`1.00%`, `t=0.306`, `DSR=0.0000`, `Insuff`) |
| `Donchian_55_20_LS` | `6.0 / 7.0* / 8.5` (`2.0`) | -0.216 (`-1.41%`, `t=-0.158`, `DSR=0.0000`, `Fail`) | -0.234 (`-1.47%`, `t=-0.172`, `DSR=0.0000`, `Fail`) | -0.261 (`-1.57%`, `t=-0.191`, `DSR=0.0000`, `Fail`) | -0.144 (`-1.15%`, `t=-0.105`, `DSR=0.0000`, `Fail`) |
| `Donchian_55_20_LO` | `6.0 / 7.0* / 8.5` (`2.0`) | +0.254 (`0.63%`, `t=0.252`, `DSR=0.0000`, `Insuff`) | +0.245 (`0.59%`, `t=0.243`, `DSR=0.0000`, `Insuff`) | +0.232 (`0.55%`, `t=0.231`, `DSR=0.0000`, `Insuff`) | +0.289 (`0.75%`, `t=0.286`, `DSR=0.0000`, `Insuff`) |
| `Carry_SpotPerp_10Apr` | `16.5 / 18.5* / 21.5` (`12.0`) | -0.366 (`-1.33%`, `t=-3.258`, `DSR=0.0000`, `Fail`) | -0.537 (`-1.94%`, `t=-4.507`, `DSR=0.0000`, `Fail`) | -0.793 (`-2.86%`, `t=-6.086`, `DSR=0.0000`, `Fail`) | +0.018 (`0.06%`, `t=0.181`, `DSR=0.0000`, `Insuff`) |
| `Carry_SpotPerp_15Apr` | `16.5 / 18.5* / 21.5` (`12.0`) | +0.149 (`0.54%`, `t=1.715`, `DSR=0.0001`, `Insuff`) | +0.083 (`0.30%`, `t=0.933`, `DSR=0.0000`, `Insuff`) | -0.015 (`-0.06%`, `t=-0.164`, `DSR=0.0000`, `Fail`) | +0.296 (`1.09%`, `t=3.522`, `DSR=0.1273`, `Fail`) |
| **`Carry_SpotPerp_TopCarry30d`** | **`16.5 / 18.5* / 21.5`** (`12.0`) | **+0.837 (`3.10%`, `t=8.985`, `DSR=0.9990`, `Pass`)** | **+0.811 (`3.00%`, `t=8.594`, `DSR=0.9967`, `Pass`)** | **+0.773 (`2.86%`, `t=8.006`, `DSR=0.9773`, `Pass`)** | +0.894 (`3.32%`, `t=9.852`, `DSR=0.9998`, `Fail` diag) |
| `Baseline_BuyHold_BTC` | `10.5 / 11.5* / 13.0` (`10.0`) | +3.767 (`0.87%`, `t=0.605`, `DD=73.19%`) | +3.766 (`0.87%`, `t=0.604`, `DD=73.19%`) | +3.765 (`0.86%`, `t=0.604`, `DD=73.19%`) | +3.768 (`0.87%`, `DD=73.19%`) |
| `Baseline_BuyHold_EW` | `10.5 / 11.5* / 13.0` (`10.0`) | +0.359 (`-17.76%`, `t=0.047`, `DD=76.80%`) | +0.358 (`-17.76%`, `t=0.047`, `DD=76.80%`) | +0.356 (`-17.77%`, `t=0.047`, `DD=76.80%`) | +0.360 (`-17.76%`, `DD=76.80%`) |

### 5.2 Viabilidad Operativa con Capital `$1,000–$2,000 USDT` (`Carry_SpotPerp_TopCarry30d`)

1. **Dimensionamiento exacto (`1x` cubierto `50/50`)**: Con `max_pairs = 4`, `min_diversification = 2` y `L_i = 0.5 / max(|S_act|, 2)` ([`spot_perp_carry.rs:49-50, 208-212`](../src/domain/signals/spot_perp_carry.rs#L208-L212)), cuando los `4` slots están activos cada par asigna **`12.5%` del capital en Spot Long + `12.5%` en Perp Short** (`25.0%` por par, `100%` bruto total):
   - Sobre `$1,000 USDT`: **`$125.00 USDT` por pata** (`8` patas de `$125.00`, o `$250.00`/pata con `2` pares activos).
   - Sobre `$2,000 USDT`: **`$250.00 USDT` por pata** (`8` patas de `$250.00`, o `$500.00`/pata con `2` pares activos).
   - Ambos montos superan con holgura (`25×`–`50×`) el piso típico de `$5 USDT` (`minNotional`) en Bybit Spot y Linear, mitigando errores de redondeo de `qtyStep` que afectaban a microcuentas de `$64 USDT` en Track 3.
2. **Buffer de liquidación y amortización de costos**:
   - Al mantener `1x` nominal corto en Perp respaldado por su equivalente `50/50` en colateral USDT dentro de la estructura cubierta, la pata corta tolera un repunte direccional de hasta **`+100%`** antes de requerir reasignación de margen entre la pierna Spot (ganadora) y la pierna Perp (perdedora).
   - La cadencia de rebalanceo `min_rebalance_days = 30` mantiene `AvgDailyTurn = 0.0128` (`mean_cost_vip0 = 0.237 bps/día` a `1.5 bps` slip), amortizando los `18.50 bps` por unidad de rotación (`37.00 bps` RT apertura+cierre) sobre ventanas de tenencia `>= 30` días, incluso bajo deslizamiento severo de `3.0 bps` (`CAGR = 2.86%`, `t_nw = 8.006`, `DSR = 0.9773`, `Pass`).
3. **Rentabilidad nominal esperada y advertencias operativas reales**:
   - Un `3.00%` CAGR neto (`AnnSR = 4.75`, `MaxDD = 0.55%`) sobre `$1,000–$2,000 USDT` genera **`~$30–$60 USDT/año`** netos: funciona como **estrategia de preservación de capital / *yield* delta-neutral de baja volatilidad**, no como motor de crecimiento acelerado de cuenta pequeña.
   - **Riesgos operativos no modelados en velas `1d`**: (a) en cuentas sin margen unificado (*UTA* con colateral cruzado activo), una subida violenta exige transferir capital manualmente desde Spot hacia Perp para evitar liquidación de la pata corta; (b) ejecución asíncrona Spot vs. Perp introduce *leg-risk* momentáneo; (c) como evidenció `R2`, el *funding* puede comprimirse a tasas netas cercanas a `~1.34%` anual (`CAGR = 1.34%` en `R2`), reduciendo el rendimiento absoluto sobre cuentas pequeñas.

---

## 6. Salida Cruda Sin Editar de las 6 Compuertas PowerShell + `git status --short`

### 6.1 Catálogo Resumen de Pruebas de Integración ([`tests/macro_track5_tests.rs`](../tests/macro_track5_tests.rs), `6 / 6 ok`)

| # | Test (`archivo:línea`) | Alcance Verificado | Estado |
|---:|---|---|---|
| 1 | `test_macro_trend_and_donchian_closed_form_vectors` (`L215-531`) | `CausalEwmaVol` (seed muestral 20d, $\alpha=2/21$, clamp `[0.15, 3.00]`), `VolTargetTrendStrategy` (`20/60/120d`, banda `0.02` bit-a-bit, `LongOnly`, tope `max_gross_leverage`) y `DonchianBreakoutStrategy` (canales previos excluyendo mecha barra `t`, transiciones `LS`/`LO`). | **PASS** |
| 2 | `test_spot_perp_carry_real_basis_vip0_fees_and_eof` (`L533-832`) | Histéresis `APR`, cadencia `30d` con desempate `SymbolId`, basis real `(r_spot - r_perp)`, `BarOpenSettlement` + EOF, `18.50 bps` VIP-0 vs `12.00 bps` `MakerDiag`, y garantía de que `diag_zero_cells` nunca emite `Pass`. | **PASS** |
| 3 | `test_equity_metrics_and_passive_baselines_closed_form` (`L834-1049`) | Formulación cerrada de `compute_equity_metrics` (`final_equity`, `CAGR%`, `AnnSR`, `MaxDD%`, `Calmar`, ruina `-100%`), `Baseline_BuyHold_BTC` / `EW` con pesos *drifted*, listado tardío y cierre EOF. | **PASS** |
| 4 | `test_zero_lookahead_bit_for_bit_and_symbol_isolation` (`L1051-1282`) | Mutación `10x` precio / `-500 bps` funding tras corte `t > 135`: igualdad `to_bits()` en los `9` runners hasta `exit_ts_ns <= cutoff`; aislamiento total de estado por `SymbolId` entre `Trend`, `Donchian` y `Carry`. | **PASS** |
| 5 | `test_edge_cases_late_spot_flat_candles_and_invalid_inputs` (`L1284-1510`) | Listado tardío Spot (`Carry` excluye hasta existir dual mientras `Trend`/`Donchian` operan `linear`), velas planas (`GK=0`, `sigma_ann=0.15`), rechazo de parámetros inválidos/NaN y timestamps duplicados/no monótonos. | **PASS** |
| 6 | `test_cli_scan_macro_end_to_end_and_json_roundtrip` (`L1512-1649`) | Invocación CLI `scan-macro --linear-dir --spot-dir --slippage-bps 1.5 --out`, tabla ASCII (`9` estrategias + `2` benchmarks), roundtrip JSON `MacroFalsificationReport` y rechazo tipado de argumentos CLI inválidos. | **PASS** |

### 6.2 Salida Cruda Sin Editar de las 6 Compuertas y `git status --short`

> Salida literal capturada en PowerShell 5.1 (`D:\Desarrollo\VORTEX-ALFA`) tras completar Track 5, los `5` barridos en `--release` y la suite de `41` tests de integración, preservada sin editar en los Gates 1–6. Al pie se anexa el `git status --short` final verificado tras redactar este reporte y actualizar `DOCS/INDEX.md`.

```text
Using Windows PowerShell: 5.1.26100.9818
=== GATE 1: cargo fmt --all -- --check ===
FMT_EXIT=0
=== GATE 2: cargo clippy --all-targets --all-features -- -D warnings ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.16s
CLIPPY_EXIT=0
=== GATE 3: cargo test --all-targets -- --nocapture ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.16s
     Running unittests src\lib.rs (target\debug\deps\vortex_alfa-a216a8d1bd8b90ff.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\vortex_alfa-e53f0a43f8dc11a2.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\adversarial_math_tests.rs (target\debug\deps\adversarial_math_tests-1e7b3596f2b1b423.exe)

running 9 tests
test test_acklam_normal_inv_cdf_accuracy ... ok
test test_garman_klass_closed_form_and_flat_bar ... ok
test test_holm_bonferroni_order_and_monotonicity ... ok
test test_hawkes_o1_three_step_analytical ... ok
test test_moments4_closed_form_exact ... ok
test test_newey_west_closed_form_n8 ... ok
test test_dsr_cross_sectional_std_and_diag_never_passes ... ok
test test_vip0_ceil_fee_e8_and_book_roundtrip_asymmetry ... ok
test test_exact_funding_boundary_bar_open_vs_bar_close ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\cross_sectional_tests.rs (target\debug\deps\cross_sectional_tests-a086dcf4759e346e.exe)

running 7 tests
test test_xs_momentum_vol_adj_skip_and_gk_analytical ... ok
test test_xs_funding_carry_closed_form_and_ranking ... ok
test test_xs_edge_cases_zero_var_flat_bar_ties_insufficient_symbols_and_non_finite ... ok
test test_xs_residual_reversion_ols_beta_and_clamp_analytical ... ok
test test_exact_turnover_funding_bar_open_eof_and_maker_never_passes ... ok
test test_zero_lookahead_end_to_end_track3_xs ... ok
test test_cli_scan_xs_csv_and_global27_report ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s

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

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

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

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests\zero_lookahead_tests.rs (target\debug\deps\zero_lookahead_tests-68c127dd0d00bc59.exe)

running 5 tests
test test_zero_lookahead_end_to_end_track1_micro ... ok
test test_zero_lookahead_end_to_end_track2_meso ... ok
test test_strict_symbol_isolation_track2_meso ... ok
test test_strict_symbol_isolation_track1_micro ... ok
test test_multifile_par2_vpz1_vkl1_streaming_and_cli ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

TEST_EXIT=0
=== GATE 4: src/**/*.rs <= 350 lines ===

File                                                              Lines Pass
----                                                              ----- ----
D:\Desarrollo\VORTEX-ALFA\src\lib.rs                                  7 True
D:\Desarrollo\VORTEX-ALFA\src\main.rs                                17 True
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
D:\Desarrollo\VORTEX-ALFA\src\application\mod.rs                     10 True
D:\Desarrollo\VORTEX-ALFA\src\domain\error.rs                        45 True
D:\Desarrollo\VORTEX-ALFA\src\domain\fees.rs                        179 True
D:\Desarrollo\VORTEX-ALFA\src\domain\ledger.rs                      113 True
D:\Desarrollo\VORTEX-ALFA\src\domain\mod.rs                           8 True
D:\Desarrollo\VORTEX-ALFA\src\domain\normal_dist.rs                 217 True
D:\Desarrollo\VORTEX-ALFA\src\domain\ports.rs                        18 True
D:\Desarrollo\VORTEX-ALFA\src\domain\stats.rs                       348 True
D:\Desarrollo\VORTEX-ALFA\src\domain\types.rs                       331 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\cross_sectional.rs     306 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\macro_donchian.rs      192 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\macro_trend.rs         296 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\macro_vol.rs           151 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\market_making.rs       310 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\meso_energy.rs         331 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\micro_detectors.rs     226 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\micro_tail.rs          330 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\mod.rs                   9 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\spot_perp_carry.rs     241 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\binary_codec.rs        342 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\csv_candle_reader.rs   319 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\mod.rs                   4 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\tape_reader.rs         325 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\vkl1_reader.rs         154 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli.rs                     328 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli_macro.rs               158 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli_mm.rs                  136 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\mod.rs                       3 True


=== GATE 5: Banned patterns in src/**/*.rs ===
BANNED_MATCHES=0
=== GATE 6: UTF-8 NoBOM + LF-only in src & tests ===

File                          UTF8_NoBOM LF_Only
----                          ---------- -------
lib.rs                              True    True
main.rs                             True    True
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
cli_macro.rs                        True    True
cli_mm.rs                           True    True
mod.rs                              True    True
adversarial_math_tests.rs           True    True
cross_sectional_tests.rs            True    True
edge_cases_and_drift_tests.rs       True    True
macro_track5_tests.rs               True    True
market_making_tests.rs              True    True
zero_lookahead_tests.rs             True    True

=== GIT DIFF --NAME-ONLY (LAW 2 SCOPE LOCK) ===
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
?? DOCS/PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md
?? DOCS/PHASE_4_TRACK5_MACRO_FALSIFICATION_REPORT.md
?? scripts/audit_macro5y_datasets.py
?? scripts/fetch_bybit_macro5y.py
?? scripts/run_track5_sweep.ps1
?? src/application/falsifier_macro.rs
?? src/application/macro_accounting.rs
?? src/application/macro_baselines.rs
?? src/domain/signals/macro_donchian.rs
?? src/domain/signals/macro_trend.rs
?? src/domain/signals/macro_vol.rs
?? src/domain/signals/spot_perp_carry.rs
?? src/interfaces/cli_macro.rs
?? tests/macro_track5_tests.rs
```

---

## 7. Veredicto Frío de Fase 4

- **Resultado de Falsación Principal (`5` años `1d`, `VIP-0 + 1.5 bps` slippage, `M = 9`, `t_crit = 3.5000`)**: **`1 / 9 Pass`** (`2 Fail`, `6 InsufficientSample`).
- **Única estrategia aprobada en muestra completa y sensibilidad (`0.5 / 1.5 / 3.0 bps`)**: **`Carry_SpotPerp_TopCarry30d`** (`N = 1,796 >= n_min = 199`, `mean = +0.811 bps/día`, **`CAGR = 3.00%`**, **`AnnSR = 4.75`**, **`MaxDD = 0.55%`**, **`Calmar = 5.47`**, `t_nw = 8.594`, `DSR = 0.9967`), superando en rendimiento ajustado por riesgo a los dos benchmarks pasivos Spot (`Baseline_BuyHold_BTC`: `0.87%` CAGR, `73.19%` MaxDD, `0.27` Sharpe; `Baseline_BuyHold_EW`: `-17.76%` CAGR, `76.80%` MaxDD, `0.02` Sharpe).
- **Contraste crítico frente a rotación diaria (`Carry_SpotPerp_10Apr`) y límite de régimen `R2`**:
  - `Carry_SpotPerp_10Apr` (`min_rebalance_days = 1`) captura esencialmente el mismo *funding* bruto (`+1.059` vs `+1.062 bps/día`), pero fracasa rotundamente (`Fail`, `-1.94%` CAGR, `t_nw = -4.507`) porque sus rebalanceos diarios consumen `1.579 bps/día` contra la tarifa dual de `18.50 bps` (`Spot 10.00 + Perp 5.50 + 3.00 slip`).
  - Aunque `Carry_SpotPerp_TopCarry30d` aprueba holgadamente en `R1` (`t_nw = 7.363`, `DSR = 0.9999`) y mantiene retorno positivo en `R2` (`+0.3665 bps/día`, `CAGR = 1.34%`, `AnnSR = 2.01`, `MaxDD = 0.59%`, `t_nw = 3.695 >= 3.5000`), la caída de `-53.7%` en el *funding* bruto durante `R2` (`+1.428 -> +0.660 bps/día`) eleva `n_min` a `1,106 > 884` y reduce `DSR` a `0.3216 < 0.95`, dejando **`0 / 9 Pass` en `R2` independiente**.
- **Falsación definitiva de las familias direccionales macro (`VolTargetTrend` y `DonchianBreakout`)**: **`0 / 6 Pass`** tanto en `VIP-0` como bajo `VIP0-MakerDiag` en los `5` años, en `R1`, en `R2`, y en los tres niveles de slippage (`0.5 / 1.5 / 3.0 bps`). En los `12` mayores de Bybit `1d`, ninguna señal tendencial o de ruptura alcanza potencia estadística suficiente frente al ruido diario (`t_nw` máximo `1.405` en `VIP-0` y `1.497` en `MakerDiag`, `DSR <= 0.0102` en todos los escenarios).
- **Implicación operativa**: El único candidato estadísticamente sobreviviente del programa completo Tracks 1–5 (`0/20` + `0/36` + `0/27` + `0/6` + `1/9`) es **`Carry_SpotPerp_TopCarry30d`** como motor conservador de *funding carry* con cadencia `>= 30` días (`~3.00%` CAGR histórico, `~1.34%` en `R2`), sujeto a disciplina estricta de amortización de tarifas Spot (`10.00 bps`), gestión de margen entre patas Spot/Perp y aceptación explícita de compresión de tasas de *funding* en regímenes recientes.

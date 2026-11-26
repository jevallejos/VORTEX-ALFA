# VORTEX-ALFA — Phase 2 Track 3 Empirical Falsification Report

> **Commit Base (`main`)**: `66999b7` + módulos aditivos Track 3 (`6/6` compuertas PS7 en verde, `29/29` tests `ok`)
> **Engine**: `target\release\vortex-alfa.exe scan-xs` ([`src/application/falsifier_xs.rs`](../src/application/falsifier_xs.rs))
> **Universo & Timeframe**: `30` perpetuos lineales Bybit V5 (`0:BTCUSDT .. 29:ICPUSDT`), velas cerradas `4h` (`29` pares operables con `--exclude-benchmark` por defecto; `BTCUSDT` como benchmark de mercado)
> **Ventana Histórica UTC**: `2024-11-25T00:00:00Z` (`1732492800000`) a `2026-11-25T00:00:00Z` (`1795564800000`), `24` meses exactos (`730` días, `4,380` barras `4h` por símbolo)
> **Partición Temporal OOS**: Año 1 `2024-11-25T00:00:00Z`–`2025-11-25T00:00:00Z` (`2,190` barras/símbolo) | Año 2 `2025-11-25T00:00:00Z`–`2026-11-25T00:00:00Z` (`2,190` barras/símbolo)
> **Fricción Vinculante**: `VIP0-Taker` (`5.50 bps` fee taker + `--slippage-bps 1.5` = `7.00 bps` por unidad de rotación `turnover`, `14.00 bps` en apertura+cierre completos; `is_binding = true`) | Diagnóstico no vinculante `VIP0-MakerDiag` (`2.00 bps` fee maker + `0.00 bps` slippage = `2.00 bps` por unidad de rotación; `is_binding = false`)
> **Index**: [`DOCS/INDEX.md`](./INDEX.md) | [`DOCS/PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md) | [`DOCS/PHASE_1A_TRACK2_FALSIFICATION_REPORT.md`](./PHASE_1A_TRACK2_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_1B_TRACK1_FALSIFICATION_REPORT.md`](./PHASE_1B_TRACK1_FALSIFICATION_REPORT.md)

---

## 0. Conformidad con las 4 Leyes D en Fase 2

| Ley | Regla | Evidencia Verificada |
|---|---|---|
| **Ley 1 — Cero Duplicación (SSOT / DRY)** | Reutilización estricta de primitives de dominio y lectores existentes; cero cálculo estadístico/señal en Python/PS | [`src/domain/signals/cross_sectional.rs`](../src/domain/signals/cross_sectional.rs#L171) reutiliza [`garman_klass_vol`](../src/domain/signals/meso_energy.rs#L33-L54); [`src/application/falsifier_xs.rs`](../src/application/falsifier_xs.rs#L173-L176) reutiliza [`FeeSchedule::bybit_vip0`](../src/domain/fees.rs#L24-L31) y [`evaluate_hypothesis_batch`](../src/domain/stats.rs#L277-L348) ([llamada en L301-302](../src/application/falsifier_xs.rs#L301-L302)); [`src/interfaces/cli.rs`](../src/interfaces/cli.rs#L188-L192) reutiliza [`CsvCandleReader`](../src/infrastructure/csv_candle_reader.rs) y [`StreamingVkl1Reader`](../src/infrastructure/vkl1_reader.rs). [`scripts/fetch_bybit_xs30.py`](../scripts/fetch_bybit_xs30.py), [`scripts/audit_xs30_datasets.py`](../scripts/audit_xs30_datasets.py) y [`scripts/run_track3_sweep.ps1`](../scripts/run_track3_sweep.ps1) no calculan señales, rotación, fees, Newey-West ni DSR. |
| **Ley 2 — Cero Desviación (Scope Lock)** | Prohibido modificar Track 1, Track 2, estadística base o tests existentes | `0` cambios en Track 1 (`micro_detectors.rs`, `micro_tail.rs`, `falsifier_micro.rs`), Track 2 (`meso_energy.rs`, `falsifier_meso.rs`), `stats.rs`, `fees.rs`, `types.rs`, [`Cargo.toml`](../Cargo.toml) o los `22` tests previos. Track 3 se implementa de forma aditiva en [`cross_sectional.rs`](../src/domain/signals/cross_sectional.rs), [`falsifier_xs.rs`](../src/application/falsifier_xs.rs), [`cli.rs`](../src/interfaces/cli.rs#L164-L197) (`scan-xs`, registrado en [L52](../src/interfaces/cli.rs#L52)) y [`tests/cross_sectional_tests.rs`](../tests/cross_sectional_tests.rs) (`7` tests nuevos; `29/29` total). |
| **Ley 3 — Cero Diluvio** | `src/**/*.rs <= 350` líneas, `3` deps en `Cargo.toml`, Python stdlib | Máximo `348` líneas en `src/domain/stats.rs`; `cross_sectional.rs` = `306` líneas, `falsifier_xs.rs` = `334` líneas, `cli.rs` = `322` líneas (`26/26` archivos `Pass`); [`Cargo.toml`](../Cargo.toml) intacto (`thiserror`, `serde`, `serde_json`); scripts Python en stdlib puro. |
| **Ley 4 — Cero Disfraz** | Reporte íntegro de las 27 celdas, turnover de cierre EOF, denominador bruto `2L`, `MakerDiag` no vinculante | Las `27` celdas completas (`0 / 27 Pass`) se reportan sin omitir negativas (`XsResidualReversion`); [`settle_pos`](../src/application/falsifier_xs.rs#L128-L131) y [`run_xs_falsification_detailed`](../src/application/falsifier_xs.rs#L264-L270) cobran rotación de liquidación EOF; [`build_dollar_neutral_weights`](../src/domain/signals/cross_sectional.rs#L284-L306) normaliza por bruto `2L = 1.0`; `VIP0-MakerDiag` (`is_binding = false`) jamás aprueba y tampoco alcanza `t_NW >= 3.5` ni `DSR >= 0.95` (`0 / 27`). |

---

## 1. Manifiesto de los 30 Pares Reales (`4h`, 24 Meses) y Auditoría de Integridad

Todos los `30` perpetuos lineales del universo oficial estuvieron activos en el rango completo `2024-11-25T00:00:00Z`–`2026-11-25T00:00:00Z` y completaron el **`100.0%`** de las `4,380` barras cerradas `4h` esperadas (`131,400` velas en `bybit_xs30_4h`, `65,700` en `bybit_xs30_4h_y1` y `65,700` en `bybit_xs30_4h_y2`; `90` CSVs y `262,800` filas auditadas en `data_local/bybit_xs30_4h/audit_report.json`).

| ID | Símbolo | Min UTC | Max UTC | Velas `4h` | `Y1`/`Y2` | Gaps | Cob. | Intervalos Funding | Raw Events | Sub-4h Agg. | Buckets | Funding Raw Sum (bps) | Funding CSV Sum (bps) | `diff_bps` |
|---:|---|---|---|---:|---|---:|---:|---|---:|---:|---:|---:|---:|---|
| 0 | `BTCUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 911.9298 | 911.9298 | `0` |
| 1 | `ETHUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 913.7089 | 913.7089 | `0` |
| 2 | `SOLUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 411.5407 | 411.5407 | `0` |
| 3 | `AVAXUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 552.459 | 552.459 | `0` |
| 4 | `TAOUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `4h, 8h` | 4,226 | 0 | 4,226 | 1050.2966 | 1050.2966 | `0` |
| 5 | `SUIUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 1061.7912 | 1061.7912 | `0` |
| 6 | `ORDIUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `4h, 8h` | 4,226 | 0 | 4,226 | 1244.6355 | 1244.6355 | `0` |
| 7 | `PENDLEUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 1170.2135 | 1170.2135 | `0` |
| 8 | `WIFUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `4h, 8h` | 4,226 | 0 | 4,226 | 510.827 | 510.827 | `0` |
| 9 | `LINKUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 1033.3196 | 1033.3196 | `0` |
| 10 | `NEARUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 993.2417 | 993.2417 | `0` |
| 11 | `LTCUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 966.3448 | 966.3448 | `0` |
| 12 | `BNBUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 605.968 | 605.968 | `0` |
| 13 | `XRPUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 761.9967 | 761.9967 | `0` |
| 14 | `DOGEUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 951.6266 | 951.6266 | `0` |
| 15 | `ADAUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 860.6796 | 860.6796 | `0` |
| 16 | `DOTUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 249.6876 | 249.6876 | `0` |
| 17 | `ATOMUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | -1066.1102 | -1066.1102 | `0` |
| 18 | `UNIUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 911.1792 | 911.1792 | `0` |
| 19 | `AAVEUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 949.1709 | 949.1709 | `0` |
| 20 | `ARBUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 987.2377 | 987.2377 | `0` |
| 21 | `OPUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 902.8357 | 902.8357 | `0` |
| 22 | `APTUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | -965.973 | -965.973 | `0` |
| 23 | `INJUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `1h, 6h, 8h` | 2,339 | 128 | 2,212 | -240.5598 | -240.5598 | `0` |
| 24 | `TIAUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `4h, 8h` | 4,226 | 0 | 4,226 | 514.978 | 514.978 | `0` |
| 25 | `SEIUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | -781.16 | -781.16 | `0` |
| 26 | `FILUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 532.3257 | 532.3257 | `0` |
| 27 | `BCHUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | -35.5449 | -35.5449 | `0` |
| 28 | `ETCUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 721.9904 | 721.9904 | `0` |
| 29 | `ICPUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T20:00:00Z` | 4,380 | 2,190/2,190 | 0 | 100.0% | `8h` | 2,190 | 0 | 2,190 | 158.0321 | 158.0321 | `0` |

### Auditoría de Integridad & Agregación Causal de Funding (`Opción A`)

- **Auditoría integral (`data_local/bybit_xs30_4h/audit_report.json`) — Estado `PASS`**: `30` JSONs crudos de funding (`raw_funding_failures = 0`), `90` archivos CSV (`rows_checked = 262,800`), `nan_inf_total = 0`, `non_pos_price_total = 0`, `invalid_ohlc_total = 0`, `neg_vol_total = 0`, `duplicate_ts_total = 0`, `backward_ts_total = 0`, `wrong_sym_id_total = 0`, `out_of_window_total = 0`, `misaligned_ts_total = 0`, `gap_count_total = 0`, `missing_bars_total = 0`, `split_mismatch_total = 0`, `per_bar_funding_mismatch_total = 0`.
- **Ciclos de funding observados en los 30 pares**:
  - `25` pares mantuvieron ciclo fijo de `8h` (`2,190` liquidaciones cada uno).
  - `4` pares (`TAOUSDT`, `ORDIUSDT`, `WIFUSDT`, `TIAUSDT`) alternaron ciclos `[4.0, 8.0]h` (`4,226` liquidaciones cada uno, todas coincidentes exactamente con aperturas `4h` UTC).
  - `1` par (**`[23] INJUSDT`**) operó con ciclos `[1.0, 6.0, 8.0]h` (`2,339` eventos crudos), al activar Bybit temporalmente liquidación de `1h` entre `2026-02-19T17:00Z` y `2026-02-26T18:00Z` (seguido de un salto de `6.0h` hacia `2026-02-27T00:00Z` al restaurar el ciclo de `8h`).
- **Agregación causal en cubetas semiabiertas `(bar_open_ts - 4h, bar_open_ts]` (Opción A)**: Implementada en [`aggregate_funding_causal_buckets`](../scripts/fetch_bybit_xs30.py#L308-L325), cada evento sub-4h se asigna únicamente a la barra `4h` cuya apertura cierra el intervalo causal (`target_open_ts >= f_ts`, hasta `2026-11-24T20:00:00Z`), evitando cualquier anticipación (*lookahead*). Los `128` eventos sub-4h de `INJUSDT` quedaron consolidados en `2,212` cubetas alineadas con **`unmatched_events_count = 0`**, **`per_bar_mismatch_count = 0`** y **`reconciliation_diff_bps = 0`** (`-240.5598 == -240.5598 bps`) en los **`30 / 30`** pares.

### Declaración Explícita de Sesgo Residual de Supervivencia

> [!WARNING]
> **Sesgo Residual de Supervivencia del Universo (`30` pares)**: El panel completo `0:BTCUSDT .. 29:ICPUSDT` fue seleccionado condicionalmente sobre contratos perpetuos que permanecieron listados y líquidos en Bybit V5 hasta `2026-11-25`, excluyendo por construcción tokens deslistados o severamente deteriorados entre `2024-11-25` y `2026-11-25`. En consecuencia, los estimadores puntuales positivos observados dentro de la muestra constituyen una **cota superior optimista** respecto a un universo libre de sesgo de supervivencia; el fracaso `0 / 27 Pass` ocurre incluso bajo esta ventaja de selección.

---

## 2. Trazabilidad `Ecuación (LaTeX) -> archivo:línea` (Track 3 Cross-Sectional)

| Concepto / Ecuación (LaTeX) | Ubicación Exacta (`archivo:línea`) | Análisis Dimensional | Guardas de Dominio y Numéricas |
|---|---|---|---|
| **`XsFundingCarry`**: $S_{\text{carry},i,t} = -\sum_{j=0}^{W_f - 1} f_{i,t-j}$ | [`src/domain/signals/cross_sectional.rs`](../src/domain/signals/cross_sectional.rs#L220-L225) ([ventana L56](../src/domain/signals/cross_sectional.rs#L56), [buffer L193](../src/domain/signals/cross_sectional.rs#L193)) | $f_{i,t}, S_{\text{carry},i,t}$ en $[\text{bps}]$ ($W_f = 18$) | Exige `funding_window.len() == 18`; funding finito (`NonFiniteBps`); signo negativo favorece recibir funding positivo como Short o negativo como Long. |
| **`XsResidualReversion` OLS $\hat{\beta}$, Clamp y Residuo**: $\hat{\beta}_i = \text{clamp}\!\left(\frac{\sum (r_i-\bar{r}_i)(r_B-\bar{r}_B)}{\sum (r_B-\bar{r}_B)^2},\; \beta_{\min}, \beta_{\max}\right)$, $S_{\text{rev},i,t} = -\sum_{u=t-W_r+1}^{t} (r_{i,u} - \hat{\beta}_i r_{B,u})$ | [`src/domain/signals/cross_sectional.rs`](../src/domain/signals/cross_sectional.rs#L254-L282) ([config L40-44](../src/domain/signals/cross_sectional.rs#L40-L44), [pares L196-205](../src/domain/signals/cross_sectional.rs#L196-L205), [score L226-230](../src/domain/signals/cross_sectional.rs#L226-L230)) | Retornos logarítmicos y residuo adimensionales ($W_\beta = 84$, $W_r = 18$) | Exige par alineado con `BTCUSDT` en `prev_ts_ns`; $\text{Var}(r_B) > 10^{-12}$; $\hat{\beta}_i \in [-3.0, 5.0]$; `beta_pairs.len() >= 84`; resultado finito. |
| **`XsMomentumVolAdj` Skip + GK Divisor**: $S_{\text{mom},i,t} = \frac{\ln(C_{i,t-W_s} / C_{i,t-W_m})}{\frac{1}{W_v}\sum_{j=0}^{W_v-1}\sigma_{\text{GK},i,t-j}}$ | [`src/domain/signals/cross_sectional.rs`](../src/domain/signals/cross_sectional.rs#L231-L243) ([config L45-48](../src/domain/signals/cross_sectional.rs#L45-L48), [GK L171](../src/domain/signals/cross_sectional.rs#L171), [hist L194-195](../src/domain/signals/cross_sectional.rs#L194-L195)) | Numerador y denominador $\bar{\sigma}_{\text{GK}}$ adimensionales ($W_m=84$, $W_s=6$, $W_v=84$) | Reutiliza [`garman_klass_vol`](../src/domain/signals/meso_energy.rs#L33-L54); exige `close_history.len() == 85`, `gk_history.len() == 84`, $\bar{\sigma}_{\text{GK}} > 10^{-9}$, precios $> 0$. |
| **Pesos Dollar-Neutral (`1x` Bruto `2L`)**: $w_i = +\frac{1}{2K}\;(\text{Top-}K),\quad w_i = -\frac{1}{2K}\;(\text{Bottom-}K)$ | [`src/domain/signals/cross_sectional.rs`](../src/domain/signals/cross_sectional.rs#L284-L306) | $\sum_i \lvert w_i \rvert = 1.0$, $\sum_i w_i = 0$ | Exige `top_k > 0`, scores finitos y `scores.len() / 2 >= top_k` (sin solape Long/Short); desempate determinista por `SymbolId` ascendente. |
| **Funding `BarOpenSettlement`**: $F_{\text{port}} = -\sum_i w_i \sum_{b \in \mathcal{B}} f_{i,b}$ | [`src/application/falsifier_xs.rs`](../src/application/falsifier_xs.rs#L112-L127) ([acumulación L213-220](../src/application/falsifier_xs.rs#L213-L220), [entrada L241-244](../src/application/falsifier_xs.rs#L241-L244)) | $F_{\text{port}}$ en $[\text{bps}]$ sobre bruto `2L = 1.0` | En `BarOpenSettlement` (`!is_close`, [L206](../src/application/falsifier_xs.rs#L206)), excluye `entry_bar` ($\mathcal{B} = \{\text{entry}+1, \dots, \text{exit}\}$); `accrue` (`pos.2 != idx` o `!is_close`) cobra hasta la barra de salida inclusive. |
| **Rotación (`turnover`) y Cierre EOF**: $\text{Turn}_p = \sum_i \lvert w_{i,p} - w_{i,p-1} \rvert + \mathbb{I}_{\text{no-roll}}\sum_i \lvert w_{i,p} \rvert$ | [`src/application/falsifier_xs.rs`](../src/application/falsifier_xs.rs#L128-L131) ([roll L223-L252](../src/application/falsifier_xs.rs#L223-L252), [EOF L264-L270](../src/application/falsifier_xs.rs#L264-L270)) | Adimensional ($\text{Turn} \in [0, 2]$ por periodo) | En roll continuo (`is_roll = true`) evita doble conteo de cierre y cobra solo $\lvert \Delta w_i \rvert$; en cierre EOF (`eof = true, is_roll = false`) suma $\sum \lvert w_{i,p} \rvert$. |
| **Retorno Neto (`pnl_net_bps`)**: $R_{\text{net}} = \sum_i w_i \frac{P_{\text{exit},i}-P_{\text{entry},i}}{P_{\text{entry},i}}\cdot 10^4 + F_{\text{port}} - \text{Turn}\cdot c_{\text{leg}}$ | [`src/application/falsifier_xs.rs`](../src/application/falsifier_xs.rs#L125-L136) ([tarifas L173-176](../src/application/falsifier_xs.rs#L173-L176)) | $[\text{bps}]$; $c_{\text{VIP0}} = 5.50 + \text{slip}$, $c_{\text{Maker}} = 2.00$ | Entrada en `open` de barra siguiente a la decisión (`has_all` exige todas las patas presentes); salida EOF en `close` final; validación finita de todo término. |
| **Sharpe Anualizado (`ann_sharpe`)**: $\text{SR}_{\text{ann}} = \widehat{\text{SR}}_p \cdot \sqrt{\frac{\text{bars\_per\_year}}{H}}$ | [`src/application/falsifier_xs.rs`](../src/application/falsifier_xs.rs#L303-L308) ([`bars_per_year` L27](../src/application/falsifier_xs.rs#L27)) | Adimensional ($\text{bars\_per\_year} = 2190.0$ en `4h`) | Calculado únicamente si $\widehat{\text{SR}}_p$ es finito (`N >= 5` y $\sigma > 0$); aplica igual escala a `VIP0-Taker` y `VIP0-MakerDiag`. |
| **`max_drawdown_bps`**: $\text{MaxDD} = \max_t \left(\max_{u \le t} E_u - E_t\right),\quad E_t = \sum_{k=1}^t R_{\text{net},k}$ | [`src/application/falsifier_xs.rs`](../src/application/falsifier_xs.rs#L90-L97) ([diagnóstico L289-291](../src/application/falsifier_xs.rs#L289-L291)) | $[\text{bps}]$ acumulados (`>= 0.0`) | Acumulación causal exacta sobre la curva de retornos netos por periodo (tanto `VIP0-Taker` como `VIP0-MakerDiag`). |

---

## 3. Tabla Completa del Barrido Principal (27 Celdas: `K ∈ {1, 2, 3}` × 3 Factores × 3 Horizontes)

Ejecutado vía [`scripts/run_track3_sweep.ps1`](../scripts/run_track3_sweep.ps1) con `target\release\vortex-alfa.exe scan-xs --format csv --slippage-bps 1.5` (`7.00 bps` por unidad de rotación en `VIP0-Taker*` vs `2.00 bps` en `VIP0-MakerDiag`), `slices = 4,380`, `events = 131,400`.

- **Umbrales Holm-Bonferroni (`t_crit`)** ([`src/domain/stats.rs`](../src/domain/stats.rs#L131-L137)):
  - Por lote `K` (`M = 9`): `t_crit_9 = 3.5000` (piso `3.50`); `sr_std(VIP0)` = `0.051478` (`K=1`), `0.071538` (`K=2`), `0.088698` (`K=3`).
  - Global (`M = 27`, `data_local/reports_track3/main_24m_global27_slip1p5.json`): valor exacto observado `t_crit_holm(27) = 3.5603` ($\Phi^{-1}(1 - 0.01/54) = 3.560343$; ninguna celda se aproxima siquiera a `3.50`, máximo `t_NW = 2.008`), con `sr_std(VIP0) = 0.070278` y `sr_std(MakerDiag) = 0.070022`.
- **`n_min` y `DSR` en la tabla**: corresponden a `M = 27` (`main_24m_global27_slip1p5.json`); `Verdict_VIP0` (`M = 9`) y `Verdict_Global27` (`M = 27`) proceden directamente de sus respectivos JSONs.

| K | Factor | Horizon | N | n_min | PriceBps | FundBps | AvgTurn | VIP0_Taker_Net | MakerDiag_Net | Std | AnnSharpe | MaxDD_bps | t_NW | t_crit_9 | t_crit_27 | DSR | Verdict_VIP0 | Verdict_Global27 |
|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| 1 | `XsFundingCarry` | `6b` | 727 | 4,009 | 15.452 | 2.832 | 0.838 | 12.421 | 16.609 | 220.862 | 1.074 | 3955.5 | 1.473 | 3.5000 | 3.5603 | 0.0099 | `InsufficientSample` | `InsufficientSample` |
| 1 | `XsFundingCarry` | `18b` | 243 | 14,160 | 14.155 | 7.598 | 1.391 | 12.016 | 18.970 | 401.595 | 0.330 | 7283.0 | 0.435 | 3.5000 | 3.5603 | 0.0402 | `InsufficientSample` | `InsufficientSample` |
| 1 | `XsFundingCarry` | `42b` | 104 | 1,604 | 46.210 | 14.710 | 1.490 | 50.488 | 57.940 | 567.774 | 0.642 | 4577.0 | 0.892 | 3.5000 | 3.5603 | 0.2926 | `InsufficientSample` | `InsufficientSample` |
| 1 | `XsResidualReversion` | `6b` | 716 | - | -7.480 | 0.092 | 1.219 | -15.923 | -9.827 | 357.641 | -0.851 | 21927.6 | -1.144 | 3.5000 | 3.5603 | 0.0000 | `Fail` | `Fail` |
| 1 | `XsResidualReversion` | `18b` | 239 | - | -13.688 | -0.026 | 1.858 | -26.718 | -17.430 | 544.200 | -0.542 | 12584.9 | -0.803 | 3.5000 | 3.5603 | 0.0013 | `Fail` | `Fail` |
| 1 | `XsResidualReversion` | `42b` | 103 | 2,271,141 | 15.552 | -0.611 | 1.825 | 2.164 | 11.291 | 916.178 | 0.017 | 10583.5 | 0.026 | 3.5000 | 3.5603 | 0.0779 | `InsufficientSample` | `InsufficientSample` |
| 1 | `XsMomentumVolAdj` | `6b` | 716 | - | 4.371 | 0.026 | 0.747 | -0.833 | 2.903 | 261.236 | -0.061 | 8201.1 | -0.085 | 3.5000 | 3.5603 | 0.0000 | `Fail` | `Fail` |
| 1 | `XsMomentumVolAdj` | `18b` | 239 | 1,739 | 44.977 | -0.041 | 1.117 | 37.116 | 42.701 | 434.602 | 0.942 | 3253.5 | 1.331 | 3.5000 | 3.5603 | 0.1865 | `InsufficientSample` | `InsufficientSample` |
| 1 | `XsMomentumVolAdj` | `42b` | 103 | - | 2.389 | 0.737 | 1.592 | -8.020 | -0.058 | 645.075 | -0.090 | 8604.8 | -0.138 | 3.5000 | 3.5603 | 0.0594 | `Fail` | `Fail` |
| 2 | `XsFundingCarry` | `6b` | 727 | 3,574 | 13.025 | 2.114 | 0.763 | 9.796 | 13.613 | 164.466 | 1.138 | 3183.5 | 1.617 | 3.5000 | 3.5603 | 0.0130 | `InsufficientSample` | `InsufficientSample` |
| 2 | `XsFundingCarry` | `18b` | 243 | 1,270 | 32.840 | 5.611 | 1.331 | 29.132 | 35.789 | 291.488 | 1.102 | 3801.5 | 1.459 | 3.5000 | 3.5603 | 0.2487 | `InsufficientSample` | `InsufficientSample` |
| 2 | `XsFundingCarry` | `42b` | 104 | 428 | 70.862 | 11.661 | 1.389 | 72.796 | 79.743 | 422.581 | 1.244 | 3128.3 | 1.745 | 3.5000 | 3.5603 | 0.6250 | `InsufficientSample` | `InsufficientSample` |
| 2 | `XsResidualReversion` | `6b` | 716 | - | 0.241 | -0.141 | 1.133 | -7.834 | -2.167 | 244.719 | -0.612 | 13310.5 | -0.855 | 3.5000 | 3.5603 | 0.0000 | `Fail` | `Fail` |
| 2 | `XsResidualReversion` | `18b` | 239 | - | -11.220 | 0.098 | 1.755 | -23.408 | -14.632 | 459.843 | -0.562 | 15463.8 | -0.731 | 3.5000 | 3.5603 | 0.0011 | `Fail` | `Fail` |
| 2 | `XsResidualReversion` | `42b` | 103 | - | -3.869 | -0.582 | 1.752 | -16.718 | -7.956 | 546.793 | -0.221 | 5229.4 | -0.354 | 3.5000 | 3.5603 | 0.0421 | `Fail` | `Fail` |
| 2 | `XsMomentumVolAdj` | `6b` | 716 | 36,313 | 8.170 | -0.011 | 0.644 | 3.652 | 6.871 | 195.440 | 0.357 | 4972.1 | 0.495 | 3.5000 | 3.5603 | 0.0004 | `InsufficientSample` | `InsufficientSample` |
| 2 | `XsMomentumVolAdj` | `18b` | 239 | 3,991 | 25.206 | 0.298 | 1.033 | 18.270 | 23.437 | 324.138 | 0.622 | 3998.3 | 0.794 | 3.5000 | 3.5603 | 0.0963 | `InsufficientSample` | `InsufficientSample` |
| 2 | `XsMomentumVolAdj` | `42b` | 103 | 3,758 | 39.620 | 1.700 | 1.422 | 31.363 | 38.475 | 539.949 | 0.419 | 4458.6 | 0.640 | 3.5000 | 3.5603 | 0.1904 | `InsufficientSample` | `InsufficientSample` |
| 3 | `XsFundingCarry` | `6b` | 727 | 4,420 | 10.484 | 1.701 | 0.741 | 6.998 | 10.703 | 130.671 | 1.023 | 2528.5 | 1.443 | 3.5000 | 3.5603 | 0.0084 | `InsufficientSample` | `InsufficientSample` |
| 3 | `XsFundingCarry` | `18b` | 243 | 2,609 | 21.071 | 4.557 | 1.328 | 16.333 | 22.973 | 234.283 | 0.769 | 3988.3 | 1.063 | 3.5000 | 3.5603 | 0.1259 | `InsufficientSample` | `InsufficientSample` |
| 3 | `XsFundingCarry` | `42b` | 104 | 417 | 61.446 | 9.653 | 1.317 | 61.878 | 68.464 | 354.886 | 1.259 | 2149.0 | 2.008 | 3.5000 | 3.5603 | 0.6184 | `InsufficientSample` | `InsufficientSample` |
| 3 | `XsResidualReversion` | `6b` | 716 | - | -2.149 | -0.066 | 1.065 | -9.668 | -4.345 | 190.363 | -0.970 | 10690.3 | -1.431 | 3.5000 | 3.5603 | 0.0000 | `Fail` | `Fail` |
| 3 | `XsResidualReversion` | `18b` | 239 | - | -10.545 | 0.015 | 1.679 | -22.284 | -13.888 | 333.203 | -0.738 | 11804.5 | -0.941 | 3.5000 | 3.5603 | 0.0006 | `Fail` | `Fail` |
| 3 | `XsResidualReversion` | `42b` | 103 | - | -47.165 | -0.291 | 1.673 | -59.168 | -50.802 | 450.435 | -0.949 | 6937.5 | -1.440 | 3.5000 | 3.5603 | 0.0025 | `Fail` | `Fail` |
| 3 | `XsMomentumVolAdj` | `6b` | 716 | 62,267 | 6.274 | -0.060 | 0.562 | 2.281 | 5.090 | 159.860 | 0.273 | 3253.5 | 0.380 | 3.5000 | 3.5603 | 0.0003 | `InsufficientSample` | `InsufficientSample` |
| 3 | `XsMomentumVolAdj` | `18b` | 239 | 15,059 | 14.436 | 0.183 | 0.926 | 8.136 | 12.766 | 280.411 | 0.320 | 4359.3 | 0.418 | 3.5000 | 3.5603 | 0.0418 | `InsufficientSample` | `InsufficientSample` |
| 3 | `XsMomentumVolAdj` | `42b` | 103 | 10,068 | 23.681 | 1.052 | 1.362 | 15.195 | 22.008 | 428.239 | 0.256 | 4553.5 | 0.398 | 3.5000 | 3.5603 | 0.1394 | `InsufficientSample` | `InsufficientSample` |

**Resumen estadístico del barrido principal (27 celdas bajo `VIP0-Taker + 1.5 bps`):**
- **`Pass` (`M = 9` o `M = 27`)**: **`0 / 27`** (`10 Fail`, `17 InsufficientSample`).
- **Celdas con mayor `t_NW` y `DSR`** (evaluando también `M = 9` frente a `M = 27`):
  - `K=3 | XsFundingCarry | 42b`: `VIP0_Taker_Net = +61.878 bps`, `t_NW = 2.008`, `N = 104 < n_min_27 = 417` (`n_min_9 = 403`), `DSR_27 = 0.6184` (`DSR_9 = 0.6461` en `main_24m_k3_slip1p5.json`).
  - `K=2 | XsFundingCarry | 42b`: `VIP0_Taker_Net = +72.796 bps`, `t_NW = 1.745`, `N = 104 < n_min_27 = 428` (`n_min_9 = 413`), `DSR_27 = 0.6250` (`DSR_9 = 0.7526` en `main_24m_k2_slip1p5.json`).
  - `K=2 | XsFundingCarry | 6b`: `VIP0_Taker_Net = +9.796 bps`, `t_NW = 1.617`, `N = 727 < n_min_27 = 3,574` (`n_min_9 = 3,454`), `DSR_27 = 0.0130` (`DSR_9 = 0.0936`).
- **Desempeño por factor**: `XsResidualReversion` fracasa en `8 / 9` celdas (`Fail`) y solo alcanza `+2.164 bps` (`t = 0.026`, `n_min = 2,271,141`) en `K=1 | 42b`. `XsFundingCarry` concentra los 3 mayores `t_NW` (`2.008`, `1.745`, `1.617`), pero permanece por debajo de `2.01` frente al piso `3.5000` (`M = 9`) y `3.5603` (`M = 27`).

---

## 4. Matriz de Estabilidad Temporal OOS (Año 1 vs. Año 2)

Se incluyen **todas las `18` celdas** con `VIP0_Taker_Net > 0` o `MakerDiag_Net > 0` en el barrido principal de 24 meses (`--slippage-bps 1.5`), ordenadas por `24m t_NW` descendente (comparando `oos_y1_global27_slip1p5.json` y `oos_y2_global27_slip1p5.json` en `data_local/reports_track3/`; los veredictos coinciden idénticamente en los corridos por lote `M = 9`):

| # | K | Factor | Horizon | 24m N | 24m Mean (VIP0) | 24m Mean (Maker) | 24m t_NW | Y1 N | Y1 Mean | Y1 t_NW | Y1 Verdict | Y2 N | Y2 Mean | Y2 t_NW | Y2 Verdict |
|---:|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|---|
| 1 | 3 | `XsFundingCarry` | `42b` | 104 | 61.878 | 68.464 | 2.008 | 52 | 67.179 | 1.299 | `InsufficientSample` | 52 | -63.666 | -0.669 | `Fail` |
| 2 | 2 | `XsFundingCarry` | `42b` | 104 | 72.796 | 79.743 | 1.745 | 52 | 139.673 | 2.362 | `InsufficientSample` | 52 | -28.183 | -0.625 | `Fail` |
| 3 | 2 | `XsFundingCarry` | `6b` | 727 | 9.796 | 13.613 | 1.617 | 362 | 15.521 | 1.813 | `InsufficientSample` | 362 | 5.071 | 0.607 | `InsufficientSample` |
| 4 | 1 | `XsFundingCarry` | `6b` | 727 | 12.421 | 16.609 | 1.473 | 362 | 18.244 | 1.330 | `InsufficientSample` | 362 | 7.500 | 0.779 | `InsufficientSample` |
| 5 | 2 | `XsFundingCarry` | `18b` | 243 | 29.132 | 35.789 | 1.459 | 121 | 74.246 | 2.838 | `InsufficientSample` | 121 | -42.276 | -0.945 | `Fail` |
| 6 | 3 | `XsFundingCarry` | `6b` | 727 | 6.998 | 10.703 | 1.443 | 362 | 12.069 | 1.697 | `InsufficientSample` | 362 | 2.858 | 0.434 | `InsufficientSample` |
| 7 | 1 | `XsMomentumVolAdj` | `18b` | 239 | 37.116 | 42.701 | 1.331 | 117 | 64.300 | 1.442 | `InsufficientSample` | 117 | -52.158 | -1.324 | `Fail` |
| 8 | 3 | `XsFundingCarry` | `18b` | 243 | 16.333 | 22.973 | 1.063 | 121 | 51.262 | 2.625 | `InsufficientSample` | 121 | -32.382 | -0.927 | `Fail` |
| 9 | 1 | `XsFundingCarry` | `42b` | 104 | 50.488 | 57.940 | 0.892 | 52 | 101.172 | 1.239 | `InsufficientSample` | 52 | -22.383 | -0.311 | `Fail` |
| 10 | 2 | `XsMomentumVolAdj` | `18b` | 239 | 18.270 | 23.437 | 0.794 | 117 | 44.816 | 1.262 | `InsufficientSample` | 117 | -49.259 | -1.486 | `Fail` |
| 11 | 2 | `XsMomentumVolAdj` | `42b` | 103 | 31.363 | 38.475 | 0.640 | 51 | 116.006 | 1.799 | `InsufficientSample` | 51 | -94.530 | -1.229 | `Fail` |
| 12 | 2 | `XsMomentumVolAdj` | `6b` | 716 | 3.652 | 6.871 | 0.495 | 351 | 7.770 | 0.717 | `InsufficientSample` | 351 | -2.858 | -0.279 | `Fail` |
| 13 | 1 | `XsFundingCarry` | `18b` | 243 | 12.016 | 18.970 | 0.435 | 121 | 65.500 | 1.626 | `InsufficientSample` | 121 | -6.015 | -0.175 | `Fail` |
| 14 | 3 | `XsMomentumVolAdj` | `18b` | 239 | 8.136 | 12.766 | 0.418 | 117 | 37.248 | 1.361 | `InsufficientSample` | 117 | -10.321 | -0.432 | `Fail` |
| 15 | 3 | `XsMomentumVolAdj` | `42b` | 103 | 15.195 | 22.008 | 0.398 | 51 | 92.113 | 2.088 | `InsufficientSample` | 51 | -97.379 | -1.973 | `Fail` |
| 16 | 3 | `XsMomentumVolAdj` | `6b` | 716 | 2.281 | 5.090 | 0.380 | 351 | 6.214 | 0.712 | `InsufficientSample` | 351 | -3.829 | -0.442 | `Fail` |
| 17 | 1 | `XsResidualReversion` | `42b` | 103 | 2.164 | 11.291 | 0.026 | 51 | -106.068 | -1.100 | `Fail` | 51 | -7.778 | -0.083 | `Fail` |
| 18 | 1 | `XsMomentumVolAdj` | `6b` | 716 | -0.833 | 2.903 | -0.085 | 351 | 10.078 | 0.735 | `InsufficientSample` | 351 | -13.569 | -0.953 | `Fail` |

### Hallazgos Críticos de Estabilidad OOS

1. **Inversión generalizada de signo en Año 2 (`0 / 27 Pass` en Y1 y `0 / 27 Pass` en Y2)**:
   - **`XsFundingCarry 18b` y `42b`**: Todos los horizontes `18b` y `42b` (`K = 1, 2, 3`) pasan de positivos en Año 1 (hasta `+139.673 bps, t = 2.362` en `K=2 42b` y `+74.246 bps, t = 2.838` en `K=2 18b`) a **negativos en Año 2** (`K=3 42b: -63.666 bps`, `K=2 42b: -28.183 bps`, `K=2 18b: -42.276 bps`, `K=3 18b: -32.382 bps`).
   - **`XsFundingCarry 6b`**: Único subgrupo que permanece positivo en Año 2 (`K=1: +7.500`, `K=2: +5.071`, `K=3: +2.858 bps`), pero con **decaimiento severo de `-58.9%` (`K=1`), `-67.3%` (`K=2`) y `-76.3%` (`K=3`)** respecto a Año 1 (`+18.244`, `+15.521`, `+12.069 bps`) y `t_NW <= 0.779` en Año 2.
   - **`XsMomentumVolAdj`**: Todas las `9` celdas (`K = 1, 2, 3 × 6b, 18b, 42b`) fueron positivas en Año 1 (`+6.214` a `+116.006 bps`) y **todas las `9` cayeron a negativas en Año 2** (`-2.858` a `-97.379 bps`).
2. **Nota técnica de fase de rejilla en `42b` (`24m` continuo vs. `Y2` aislado)**: En la corrida continua de `24m` (`4,380` slices), `XsFundingCarry` completa el calentamiento de `18` barras una sola vez al inicio (`idx = 17`, primera entrada en `idx = 18`), avanzando sobre la rejilla `42b` (`18, 60, ..., 2160, 2202, ...`), de modo que su primera entrada en la región de Año 2 ocurre en `idx = 2202` (`12` barras después de `2025-11-25T00:00:00Z`). En la corrida aislada `Y2` (`2,190` slices), reiniciar el calentamiento de `18` barras (`3` días) fija la primera entrada en el índice local `18` (`2,208` global), desplazando en `6` barras (`24h`) la fase de rebalanceo no solapado de `42b` respecto a `24m` (por ello `N_Y1 + N_Y2 = 104 == N_24m`, pero las ventanas individuales muestreadas en `Y2` no son idénticas a la segunda mitad de `24m`).

---

## 5. Sensibilidad de Slippage, Descomposición de Costos y Viabilidad `$64 USDT`

### 5.1 Sensibilidad `0.5 / 1.5 / 3.0 bps` (`VIP0-Taker`) vs. `VIP0-MakerDiag` (`2.0 + 0.0 bps`)

Resultados completos sobre `24m` (`M = 27`, `sens_24m_global27_slip0p5.json`, `main_24m_global27_slip1p5.json`, `sens_24m_global27_slip3p0.json` en `data_local/reports_track3/`); los corridos individuales `K = 2` y `K = 3` (`M = 9`) reproducen exactamente los mismos `Mean`, `t_NW` y veredictos (`0 / 9 Pass` tanto a `0.5 bps` como a `3.0 bps`):

| K | Factor | Horizon | PriceBps | FundBps | AvgTurn | Cost(0.5) | Cost(1.5*) | Cost(3.0) | VIP0 (0.5 bps) | VIP0 (1.5 bps*) | VIP0 (3.0 bps) | MakerDiag (2.0+0) |
|---:|---|---|---:|---:|---:|---:|---:|---:|---|---|---|---|
| 1 | `XsFundingCarry` | `6b` | 15.452 | 2.832 | 0.838 | 5.026 | 5.864 | 7.120 | 13.258 (`t=1.573`, `DSR=0.0131`) | 12.421 (`t=1.473`, `DSR=0.0099`) | 11.164 (`t=1.323`, `DSR=0.0063`) | 16.609 (`t=1.972`, `DSR=0.0359`) |
| 1 | `XsFundingCarry` | `18b` | 14.155 | 7.598 | 1.391 | 8.346 | 9.737 | 11.823 | 13.407 (`t=0.486`, `DSR=0.0454`) | 12.016 (`t=0.435`, `DSR=0.0402`) | 9.929 (`t=0.359`, `DSR=0.0333`) | 18.970 (`t=0.689`, `DSR=0.0711`) |
| 1 | `XsFundingCarry` | `42b` | 46.210 | 14.710 | 1.490 | 8.942 | 10.433 | 12.668 | 51.978 (`t=0.918`, `DSR=0.3024`) | 50.488 (`t=0.892`, `DSR=0.2926`) | 48.252 (`t=0.852`, `DSR=0.2780`) | 57.940 (`t=1.024`, `DSR=0.3418`) |
| 1 | `XsResidualReversion` | `6b` | -7.480 | 0.092 | 1.219 | 7.316 | 8.535 | 10.364 | -14.704 (`t=-1.055`, `DSR=0.0000`) | -15.923 (`t=-1.144`, `DSR=0.0000`) | -17.752 (`t=-1.277`, `DSR=0.0000`) | -9.827 (`t=-0.703`, `DSR=0.0000`) |
| 1 | `XsResidualReversion` | `18b` | -13.688 | -0.026 | 1.858 | 11.146 | 13.004 | 15.791 | -24.861 (`t=-0.747`, `DSR=0.0016`) | -26.718 (`t=-0.803`, `DSR=0.0013`) | -29.505 (`t=-0.887`, `DSR=0.0010`) | -17.430 (`t=-0.523`, `DSR=0.0033`) |
| 1 | `XsResidualReversion` | `42b` | 15.552 | -0.611 | 1.825 | 10.951 | 12.777 | 15.515 | 3.990 (`t=0.047`, `DSR=0.0809`) | 2.164 (`t=0.026`, `DSR=0.0779`) | -0.573 (`t=-0.007`, `DSR=0.0736`) | 11.291 (`t=0.133`, `DSR=0.0931`) |
| 1 | `XsMomentumVolAdj` | `6b` | 4.371 | 0.026 | 0.747 | 4.483 | 5.230 | 6.351 | -0.086 (`t=-0.009`, `DSR=0.0001`) | -0.833 (`t=-0.085`, `DSR=0.0000`) | -1.954 (`t=-0.198`, `DSR=0.0000`) | 2.903 (`t=0.295`, `DSR=0.0002`) |
| 1 | `XsMomentumVolAdj` | `18b` | 44.977 | -0.041 | 1.117 | 6.703 | 7.820 | 9.496 | 38.233 (`t=1.372`, `DSR=0.1981`) | 37.116 (`t=1.331`, `DSR=0.1865`) | 35.440 (`t=1.270`, `DSR=0.1696`) | 42.701 (`t=1.534`, `DSR=0.2475`) |
| 1 | `XsMomentumVolAdj` | `42b` | 2.389 | 0.737 | 1.592 | 9.553 | 11.146 | 13.534 | -6.427 (`t=-0.111`, `DSR=0.0624`) | -8.020 (`t=-0.138`, `DSR=0.0594`) | -10.408 (`t=-0.179`, `DSR=0.0550`) | -0.058 (`t=-0.001`, `DSR=0.0755`) |
| 2 | `XsFundingCarry` | `6b` | 13.025 | 2.114 | 0.763 | 4.580 | 5.344 | 6.489 | 10.559 (`t=1.743`, `DSR=0.0181`) | 9.796 (`t=1.617`, `DSR=0.0130`) | 8.650 (`t=1.428`, `DSR=0.0077`) | 13.613 (`t=2.246`, `DSR=0.0566`) |
| 2 | `XsFundingCarry` | `18b` | 32.840 | 5.611 | 1.331 | 7.988 | 9.319 | 11.316 | 30.463 (`t=1.526`, `DSR=0.2730`) | 29.132 (`t=1.459`, `DSR=0.2487`) | 27.135 (`t=1.358`, `DSR=0.2143`) | 35.789 (`t=1.795`, `DSR=0.3792`) |
| 2 | `XsFundingCarry` | `42b` | 70.862 | 11.661 | 1.389 | 8.337 | 9.726 | 11.810 | 74.186 (`t=1.778`, `DSR=0.6392`) | 72.796 (`t=1.745`, `DSR=0.6250`) | 70.712 (`t=1.694`, `DSR=0.6033`) | 79.743 (`t=1.914`, `DSR=0.6928`) |
| 2 | `XsResidualReversion` | `6b` | 0.241 | -0.141 | 1.133 | 6.800 | 7.934 | 9.634 | -6.700 (`t=-0.731`, `DSR=0.0000`) | -7.834 (`t=-0.855`, `DSR=0.0000`) | -9.534 (`t=-1.042`, `DSR=0.0000`) | -2.167 (`t=-0.236`, `DSR=0.0000`) |
| 2 | `XsResidualReversion` | `18b` | -11.220 | 0.098 | 1.755 | 10.531 | 12.287 | 14.919 | -21.653 (`t=-0.676`, `DSR=0.0014`) | -23.408 (`t=-0.731`, `DSR=0.0011`) | -26.041 (`t=-0.813`, `DSR=0.0008`) | -14.632 (`t=-0.456`, `DSR=0.0031`) |
| 2 | `XsResidualReversion` | `42b` | -3.869 | -0.582 | 1.752 | 10.515 | 12.267 | 14.896 | -14.966 (`t=-0.317`, `DSR=0.0450`) | -16.718 (`t=-0.354`, `DSR=0.0421`) | -19.347 (`t=-0.410`, `DSR=0.0381`) | -7.956 (`t=-0.169`, `DSR=0.0579`) |
| 2 | `XsMomentumVolAdj` | `6b` | 8.170 | -0.011 | 0.644 | 3.863 | 4.507 | 5.473 | 4.295 (`t=0.582`, `DSR=0.0006`) | 3.652 (`t=0.495`, `DSR=0.0004`) | 2.686 (`t=0.364`, `DSR=0.0003`) | 6.871 (`t=0.930`, `DSR=0.0020`) |
| 2 | `XsMomentumVolAdj` | `18b` | 25.206 | 0.298 | 1.033 | 6.201 | 7.234 | 8.785 | 19.303 (`t=0.840`, `DSR=0.1056`) | 18.270 (`t=0.794`, `DSR=0.0963`) | 16.719 (`t=0.726`, `DSR=0.0835`) | 23.437 (`t=1.021`, `DSR=0.1476`) |
| 2 | `XsMomentumVolAdj` | `42b` | 39.620 | 1.700 | 1.422 | 8.534 | 9.956 | 12.090 | 32.785 (`t=0.670`, `DSR=0.1981`) | 31.363 (`t=0.640`, `DSR=0.1904`) | 29.230 (`t=0.597`, `DSR=0.1791`) | 38.475 (`t=0.787`, `DSR=0.2301`) |
| 3 | `XsFundingCarry` | `6b` | 10.484 | 1.701 | 0.741 | 4.446 | 5.187 | 6.298 | 7.739 (`t=1.596`, `DSR=0.0127`) | 6.998 (`t=1.443`, `DSR=0.0084`) | 5.887 (`t=1.213`, `DSR=0.0043`) | 10.703 (`t=2.210`, `DSR=0.0536`) |
| 3 | `XsFundingCarry` | `18b` | 21.071 | 4.557 | 1.328 | 7.967 | 9.295 | 11.287 | 17.661 (`t=1.150`, `DSR=0.1457`) | 16.333 (`t=1.063`, `DSR=0.1259`) | 14.342 (`t=0.933`, `DSR=0.0996`) | 22.973 (`t=1.498`, `DSR=0.2442`) |
| 3 | `XsFundingCarry` | `42b` | 61.446 | 9.653 | 1.317 | 7.904 | 9.221 | 11.197 | 63.195 (`t=2.052`, `DSR=0.6321`) | 61.878 (`t=2.008`, `DSR=0.6184`) | 59.902 (`t=1.943`, `DSR=0.5973`) | 68.464 (`t=2.226`, `DSR=0.6835`) |
| 3 | `XsResidualReversion` | `6b` | -2.149 | -0.066 | 1.065 | 6.388 | 7.453 | 9.050 | -8.603 (`t=-1.273`, `DSR=0.0000`) | -9.668 (`t=-1.431`, `DSR=0.0000`) | -11.265 (`t=-1.670`, `DSR=0.0000`) | -4.345 (`t=-0.640`, `DSR=0.0000`) |
| 3 | `XsResidualReversion` | `18b` | -10.545 | 0.015 | 1.679 | 10.075 | 11.755 | 14.273 | -20.605 (`t=-0.869`, `DSR=0.0008`) | -22.284 (`t=-0.941`, `DSR=0.0006`) | -24.803 (`t=-1.048`, `DSR=0.0004`) | -13.888 (`t=-0.584`, `DSR=0.0022`) |
| 3 | `XsResidualReversion` | `42b` | -47.165 | -0.291 | 1.673 | 10.039 | 11.712 | 14.222 | -57.495 (`t=-1.399`, `DSR=0.0029`) | -59.168 (`t=-1.440`, `DSR=0.0025`) | -61.678 (`t=-1.502`, `DSR=0.0021`) | -50.802 (`t=-1.235`, `DSR=0.0046`) |
| 3 | `XsMomentumVolAdj` | `6b` | 6.274 | -0.060 | 0.562 | 3.372 | 3.933 | 4.776 | 2.843 (`t=0.474`, `DSR=0.0004`) | 2.281 (`t=0.380`, `DSR=0.0003`) | 1.438 (`t=0.239`, `DSR=0.0002`) | 5.090 (`t=0.849`, `DSR=0.0016`) |
| 3 | `XsMomentumVolAdj` | `18b` | 14.436 | 0.183 | 0.926 | 5.556 | 6.483 | 7.872 | 9.062 (`t=0.466`, `DSR=0.0469`) | 8.136 (`t=0.418`, `DSR=0.0418`) | 6.746 (`t=0.347`, `DSR=0.0348`) | 12.766 (`t=0.658`, `DSR=0.0722`) |
| 3 | `XsMomentumVolAdj` | `42b` | 23.681 | 1.052 | 1.362 | 8.175 | 9.537 | 11.581 | 16.558 (`t=0.433`, `DSR=0.1470`) | 15.195 (`t=0.398`, `DSR=0.1394`) | 13.152 (`t=0.344`, `DSR=0.1284`) | 22.008 (`t=0.577`, `DSR=0.1796`) |

### 5.2 Descomposición Precio vs. Carry vs. Costo de Rotación y Viabilidad `$64 USDT`

1. **Descomposición económica**: En `XsFundingCarry 42b`, la mayor parte de la ganancia bruta proviene del **movimiento direccional de precio** (`PriceBps = +70.862 bps` en `K=2`, `+61.446 bps` en `K=3`) más que del carry puro (`FundBps = +11.661` y `+9.653 bps`), mientras que el costo de rotación bajo `1.5 bps` de slippage (`AvgTurn = 1.389 -> 9.726 bps` en `K=2`; `1.317 -> 9.221 bps` en `K=3`) apenas resta una fracción moderada. Sin embargo, la dispersión transversal (`Std = 354.886–422.581 bps`) limita el Sharpe por operación, de modo que **incluso reduciendo el costo a `VIP0-MakerDiag` (`2.0 bps` por rotación y `0` slippage)** el máximo `t_NW` alcanza apenas `2.246` (`K=2 XsFundingCarry 6b`) y el máximo `DSR_27` apenas `0.6928` (`DSR_9 = 0.8073` en `K=2 XsFundingCarry 42b`). En contraste, `XsResidualReversion` exhibe rotación elevada (`AvgTurn = 1.065..1.858`) y `PriceBps` negativo en `7 / 9` celdas.
2. **Viabilidad operativa con capital `$64 USDT` (`1x` bruto `2L = 1.0`)**:
   - **`K = 1` (`2` patas, `1` Long + `1` Short)**: **`$32.00 USDT`** por pata (`$64 / 2`).
   - **`K = 2` (`4` patas, `2` Long + `2` Short)**: **`$16.00 USDT`** por pata (`$64 / 4`).
   - **`K = 3` (`6` patas, `3` Long + `3` Short)**: **`$10.67 USDT`** por pata (`$64 / 6`).
   - Aunque `$10.67` supera nominalmente el mínimo por orden de `$5 USDT` (`minNotional`) en `1x` bruto sin margen de variación, la **cuantización discreta de `qtyStep`** en perpetuos de alto precio unitario (`TAOUSDT`, `AAVEUSDT`, `BCHUSDT`) introduce redondeos fraccionarios significativos respecto a los pesos continuos exactos (`±1/6` o `±1/4` del capital), rompiendo la neutralidad exacta $\sum w_i = 0$ a escala de `$64 USDT`.

---

## 6. Salida Cruda Sin Editar de las 6 Compuertas PowerShell + `git status --short`

> Salida capturada literalmente en `C:\Users\fergu\.gemini\antigravity\brain\a5098bf8-b48c-4142-8f14-4b741ac2ada4\scratch\gates6_raw.txt` tras la implementación de Track 3 y los barridos en release.

```text
=== 6.1 cargo fmt --all -- --check ===

EXIT_CODE=0
=== 6.2 cargo clippy --all-targets --all-features -- -D warnings ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.17s

EXIT_CODE=0
=== 6.3 cargo test --all-targets -- --nocapture ===
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.15s
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
test test_dsr_cross_sectional_std_and_diag_never_passes ... ok
test test_newey_west_closed_form_n8 ... ok
test test_exact_funding_boundary_bar_open_vs_bar_close ... ok
test test_vip0_ceil_fee_e8_and_book_roundtrip_asymmetry ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\cross_sectional_tests.rs (target\debug\deps\cross_sectional_tests-a086dcf4759e346e.exe)

running 7 tests
test test_xs_momentum_vol_adj_skip_and_gk_analytical ... ok
test test_xs_funding_carry_closed_form_and_ranking ... ok
test test_xs_residual_reversion_ols_beta_and_clamp_analytical ... ok
test test_xs_edge_cases_zero_var_flat_bar_ties_insufficient_symbols_and_non_finite ... ok
test test_exact_turnover_funding_bar_open_eof_and_maker_never_passes ... ok
test test_zero_lookahead_end_to_end_track3_xs ... ok
test test_cli_scan_xs_csv_and_global27_report ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.33s

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

     Running tests\zero_lookahead_tests.rs (target\debug\deps\zero_lookahead_tests-68c127dd0d00bc59.exe)

running 5 tests
test test_zero_lookahead_end_to_end_track1_micro ... ok
test test_zero_lookahead_end_to_end_track2_meso ... ok
test test_strict_symbol_isolation_track2_meso ... ok
test test_strict_symbol_isolation_track1_micro ... ok
test test_multifile_par2_vpz1_vkl1_streaming_and_cli ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s


EXIT_CODE=0
=== 6.4 line count <= 350 ===

File                                                              Lines Pass
----                                                              ----- ----
D:\Desarrollo\VORTEX-ALFA\src\lib.rs                                  7 True
D:\Desarrollo\VORTEX-ALFA\src\main.rs                                17 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_meso.rs         330 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_micro.rs        280 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_xs.rs           334 True
D:\Desarrollo\VORTEX-ALFA\src\application\mod.rs                      3 True
D:\Desarrollo\VORTEX-ALFA\src\domain\error.rs                        45 True
D:\Desarrollo\VORTEX-ALFA\src\domain\fees.rs                        179 True
D:\Desarrollo\VORTEX-ALFA\src\domain\ledger.rs                      113 True
D:\Desarrollo\VORTEX-ALFA\src\domain\mod.rs                           8 True
D:\Desarrollo\VORTEX-ALFA\src\domain\normal_dist.rs                 217 True
D:\Desarrollo\VORTEX-ALFA\src\domain\ports.rs                        18 True
D:\Desarrollo\VORTEX-ALFA\src\domain\stats.rs                       348 True
D:\Desarrollo\VORTEX-ALFA\src\domain\types.rs                       331 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\cross_sectional.rs     306 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\meso_energy.rs         331 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\micro_detectors.rs     226 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\micro_tail.rs          330 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\mod.rs                   4 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\binary_codec.rs        342 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\csv_candle_reader.rs   319 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\mod.rs                   4 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\tape_reader.rs         325 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\vkl1_reader.rs         154 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli.rs                     322 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\mod.rs                       1 True



=== 6.5 banned patterns ===
GLOB_MATCH_COUNT=0
REC_MATCH_COUNT=0

=== 6.6 encoding UTF8_NoBOM + LF_Only ===

File                          UTF8_NoBOM LF_Only
----                          ---------- -------
lib.rs                              True    True
main.rs                             True    True
falsifier_meso.rs                   True    True
falsifier_micro.rs                  True    True
falsifier_xs.rs                     True    True
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
meso_energy.rs                      True    True
micro_detectors.rs                  True    True
micro_tail.rs                       True    True
mod.rs                              True    True
binary_codec.rs                     True    True
csv_candle_reader.rs                True    True
mod.rs                              True    True
tape_reader.rs                      True    True
vkl1_reader.rs                      True    True
cli.rs                              True    True
mod.rs                              True    True
adversarial_math_tests.rs           True    True
cross_sectional_tests.rs            True    True
edge_cases_and_drift_tests.rs       True    True
zero_lookahead_tests.rs             True    True



=== 6.7 git status --short ===
 M src/application/mod.rs
 M src/domain/signals/mod.rs
 M src/interfaces/cli.rs
?? DOCS/PHASE_1B_TRACK1_FALSIFICATION_REPORT.md
?? scripts/audit_xs30_datasets.py
?? scripts/fetch_bybit_xs30.py
?? scripts/run_track3_sweep.ps1
?? src/application/falsifier_xs.rs
?? src/domain/signals/cross_sectional.rs
?? tests/cross_sectional_tests.rs
```

---

## 7. Veredicto Frío de Fase 2

- **Resultado de Falsación Global (`M = 27` y `M = 9` por `K`, `VIP0-Taker + 1.5 bps` slippage)**: **`0 / 27 Pass`** (`10 Fail`, `17 InsufficientSample`) en `24m`, **`0 / 27 Pass`** en Año 1 OOS, **`0 / 27 Pass`** en Año 2 OOS, y **`0 / 27 Pass`** en sensibilidad a `0.5` y `3.0 bps`.
- **Insuficiencia intrínseca incluso bajo `VIP0-MakerDiag` (`2.0 bps` maker, `0` slippage)**: Ni siquiera eliminando el slippage y bajando el costo a tarifa maker (`VIP0-MakerDiag`, perfil no vinculante) ninguna de las 27 celdas alcanza la barrera estadística: **`0 / 27`** superan `t_NW >= 3.5` (máximo `MakerDiag t_NW = 2.246` en `K=2 | XsFundingCarry | 6b`) ni `DSR >= 0.95` (máximo `MakerDiag DSR_27 = 0.6928`, `DSR_9 = 0.8073` en `K=2 | XsFundingCarry | 42b`).
- **Diagnóstico de Causa Raíz**: El fracaso de Track 3 **no obedece únicamente a las comisiones taker**, sino a **bajo SNR estadístico** (alta dispersión por operación `Std = 130.7–916.2 bps` que exige `n_min = 403..2,271,141` frente a `N = 103..727` disponibles en 24 meses) y **no estacionariedad severa entre Año 1 y Año 2** (colapso de `XsFundingCarry 18b/42b` y `XsMomentumVolAdj` a retornos negativos en Año 2, y decaimiento `-58.9%` a `-76.3%` en `XsFundingCarry 6b`), ocurriendo además dentro de un panel sujeto a sesgo residual de supervivencia.
- **Veredicto Operativo Final (`$64 USDT`)**: **Track 3 (`XsFundingCarry`, `XsResidualReversion`, `XsMomentumVolAdj` en `4h` sobre 30 pares Bybit V5) queda EMPÍRICAMENTE FALSIFICADO (`0 / 27 Pass`).** Con los tres tracks del laboratorio cerrados sin aprobaciones (`Track 1: 0/20`, `Track 2: 0/36`, `Track 3: 0/27`), no existe fundamento empírico ni estadístico para desplegar capital real (`$64 USDT`) ni para reoptimizar parámetros *post-hoc* sobre esta muestra.

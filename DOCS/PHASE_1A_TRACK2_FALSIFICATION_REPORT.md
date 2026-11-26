# VORTEX-ALFA — Phase 1A Track 2 Empirical Falsification Report

> **Commit Base**: `05d1306` (`main`) | **Engine**: `target\release\vortex-alfa.exe scan-meso` (Law 1 & Law 2 compliant: `0` changes to `src/` and `tests/`)
> **Ventana Histórica UTC**: `2024-11-25T00:00:00Z` (`1732492800000`) a `2026-11-25T00:00:00Z` (`1795564800000`), 24 meses exactos (`730` días)
> **Partición Temporal OOS**: Año 1 `2024-11-25T00:00:00Z`–`2025-11-25T00:00:00Z` (`1764028800000`) | Año 2 `2025-11-25T00:00:00Z`–`2026-11-25T00:00:00Z`
> **Fricción Vinculante**: Bybit VIP-0 Taker/Taker (`5.50 + 5.50 = 11.00 bps` RT) + `--slippage-bps 1.5` por pierna (`3.00 bps` RT) = **`14.00 bps` RT**
> **Index**: [`DOCS/INDEX.md`](./INDEX.md) | [`DOCS/PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md)

---

## 0. Conformidad con las 4 Leyes D en Fase 1A

| Ley | Regla | Evidencia Verificada |
|---|---|---|
| **Ley 1 — Cero Duplicación** | Cero recálculo de señales, fees, Newey-West o DSR en Python/PS | [`scripts/fetch_bybit_history.py`](../scripts/fetch_bybit_history.py) y [`scripts/audit_track2_datasets.py`](../scripts/audit_track2_datasets.py) solo descargan, validan, alinean funding (`BarOpenSettlement`) y particionan CSVs. Las 54 evaluaciones corren vía `vortex-alfa.exe scan-meso` ([`scripts/run_track2_sweep.ps1`](../scripts/run_track2_sweep.ps1)). |
| **Ley 2 — Cero Desviación** | Prohibido modificar ecuaciones/umbrales de `src/domain/` o `src/application/` | Auditoría previa confirmó que [`src/infrastructure/csv_candle_reader.rs`](../src/infrastructure/csv_candle_reader.rs) ya soporta directorios y monotonicidad por `SymbolId` (`last_ts_by_symbol: HashMap<u16, i64>`). `src/**`, `tests/**` y `Cargo.toml` permanecen sin cambios (`0` diffs). |
| **Ley 3 — Cero Diluvio** | `src/**/*.rs <= 350` líneas, `Cargo.toml` con 3 deps, descargador stdlib | Máximo `348` líneas en `src/domain/stats.rs`; `Cargo.toml` intacto (`thiserror`, `serde`, `serde_json`); scripts Python 3.12 en stdlib puro (`0` `pip install`). |
| **Ley 4 — Cero Disfraz** | Reporte crudo íntegro sin filtrar perdedores ni usar `DiagZero`/menor slippage para aprobar | Las 36 celdas principales se reportan completas (`0/36 Pass`); sensibilidad `0.5 / 3.0 bps` y `DiagZero` se presentan estrictamente como diagnóstico no vinculante. |

---

## 1. Manifiesto de Datos Reales (Bybit V5 Linear Perpetuals, 24 Meses)

Todos los 12 perpetuos del universo oficial estuvieron activos antes de `2024-11-25T00:00:00Z` y completaron el `100%` de las barras cerradas esperadas (`70,080` velas `15m`, `17,520` velas `1h`, `4,380` velas `4h` por símbolo; `1,103,760` velas únicas en el conjunto principal y `3,219,300` filas totales auditadas incluyendo particiones `Y1`, `Y2` y `Ex-BTC`).

| ID | Símbolo | Min UTC (15m/1h/4h) | Max UTC `15m` | Velas `15m` | Velas `1h` | Velas `4h` | Gaps | Intervalo Funding (h) | Funding Eventos | Funding Raw Sum (bps) | `diff_bps` (`15m`/`1h`/`4h`) |
|---:|---|---|---|---:|---:|---:|---:|---|---:|---:|---|
| 0 | `BTCUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T23:45:00Z` | 70,080 | 17,520 | 4,380 | 0/0/0 | 8h | 2,190 | 911.9298 | `0` / `0` / `0` |
| 1 | `ETHUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T23:45:00Z` | 70,080 | 17,520 | 4,380 | 0/0/0 | 8h | 2,190 | 913.7089 | `0` / `0` / `0` |
| 2 | `SOLUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T23:45:00Z` | 70,080 | 17,520 | 4,380 | 0/0/0 | 8h | 2,190 | 411.5407 | `0` / `0` / `0` |
| 3 | `AVAXUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T23:45:00Z` | 70,080 | 17,520 | 4,380 | 0/0/0 | 8h | 2,190 | 552.459 | `0` / `0` / `0` |
| 4 | `TAOUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T23:45:00Z` | 70,080 | 17,520 | 4,380 | 0/0/0 | 4h, 8h | 4,226 | 1050.2966 | `0` / `0` / `0` |
| 5 | `SUIUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T23:45:00Z` | 70,080 | 17,520 | 4,380 | 0/0/0 | 8h | 2,190 | 1061.7912 | `0` / `0` / `0` |
| 6 | `ORDIUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T23:45:00Z` | 70,080 | 17,520 | 4,380 | 0/0/0 | 4h, 8h | 4,226 | 1244.6355 | `0` / `0` / `0` |
| 7 | `PENDLEUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T23:45:00Z` | 70,080 | 17,520 | 4,380 | 0/0/0 | 8h | 2,190 | 1170.2135 | `0` / `0` / `0` |
| 8 | `WIFUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T23:45:00Z` | 70,080 | 17,520 | 4,380 | 0/0/0 | 4h, 8h | 4,226 | 510.827 | `0` / `0` / `0` |
| 9 | `LINKUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T23:45:00Z` | 70,080 | 17,520 | 4,380 | 0/0/0 | 8h | 2,190 | 1033.3196 | `0` / `0` / `0` |
| 10 | `NEARUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T23:45:00Z` | 70,080 | 17,520 | 4,380 | 0/0/0 | 8h | 2,190 | 993.2417 | `0` / `0` / `0` |
| 11 | `LTCUSDT` | `2024-11-25T00:00:00Z` | `2026-11-24T23:45:00Z` | 70,080 | 17,520 | 4,380 | 0/0/0 | 8h | 2,190 | 966.3448 | `0` / `0` / `0` |

### Auditoría de Integridad & Reconciliación de Funding (`BarOpenSettlement`)

- **Auditor (`scripts/audit_track2_datasets.py`) — Estado `PASS`**: `141` archivos CSV verificados (`3,219,300` filas totales en `bybit_24m`, `bybit_24m_y1`, `bybit_24m_y2`, `bybit_24m_exbtc`), `nan_inf=0`, `non_pos_price=0`, `invalid_ohlc=0`, `neg_vol=0`, `duplicate_ts=0`, `backward_ts=0`, `wrong_sym_id=0`, `out_of_window=0`, `split_mismatch=0`, `exbtc_mismatch=0`.
- **Cierre estricto de velas**: Solo se conservaron barras cerradas con `open_ts + interval_ms <= 2026-11-25T00:00:00Z` (última apertura `15m`: `2026-11-24T23:45:00Z`, `1h`: `2026-11-24T23:00:00Z`, `4h`: `2026-11-24T20:00:00Z`) y eventos de funding en `[2024-11-25T00:00:00Z, 2026-11-25T00:00:00Z)`.
- **Observación de ciclos de funding en `4h`**: 9 pares mantuvieron ciclo fijo de `8h` (`2,190` liquidaciones en `730` días), mientras que `TAOUSDT`, `ORDIUSDT` y `WIFUSDT` operaron con ciclos de `8h` y `4h` (`4,226` liquidaciones cada uno). Ninguno de los 12 símbolos registró ciclos de `1h` o `2h` en la ventana estudiada; dado que los ciclos de `4h` coinciden exactamente con las aperturas UTC de las barras `4h` (`00:00, 04:00, 08:00, 12:00, 16:00, 20:00`), la reconciliación fue exacta en los 36 pares `(símbolo, timeframe)`: `unmatched_interior_count = 0`, `unmatched_gap_count = 0` y `reconciliation_diff_bps = 0` (`sum(funding_raw) == sum(funding_csv)` al bit).
- **Trampa operativa evitada**: [`CsvCandleReader`](../src/infrastructure/csv_candle_reader.rs#L126-L156) recorre subdirectorios recursivamente, por lo que `bybit_24m`, `bybit_24m_y1`, `bybit_24m_y2` y `bybit_24m_exbtc` se ubicaron como directorios hermanos bajo `data_local/`.

---

## 2. Tabla Completa del Barrido Principal (36 Celdas: 3 TF × 3 Hipótesis × 4 Horizontes)

Ejecutado con `target\release\vortex-alfa.exe scan-meso --format csv --slippage-bps 1.5` (`14.00 bps` RT bajo `VIP-0`). `t_crit_12 = 3.5000` (Holm-Bonferroni por lote de `M=12`, con piso `3.50` en [`src/domain/stats.rs`](../src/domain/stats.rs#L131-L137)); `t_crit_36 = 3.6353` (corte global sobre `M_global = 36`; valor exacto `Phi^-1(1 - 0.01/72) = 3.635188`, ninguna celda cae entre `3.6352` y `3.6353`).

- **`15m`**: `events_processed = 840,960`, `sr_cross_std(VIP-0) = 0.079281`, `sr_cross_std(DiagZero) = 0.044394`
- **`1h`**: `events_processed = 210,240`, `sr_cross_std(VIP-0) = 0.051459`, `sr_cross_std(DiagZero) = 0.031486`
- **`4h`**: `events_processed = 52,560`, `sr_cross_std(VIP-0) = 0.046328`, `sr_cross_std(DiagZero) = 0.043355`

| TF | Hypothesis | Horizon | N | n_min | Mean_VIP0_bps | Mean_DiagZero_bps | Std_bps | t_NW | t_crit_12 | t_crit_36 | DSR | Verdict_VIP0 | Verdict_Global36 |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|
| `15m` | `VolCompressionBreakout` | `1b` | 14,994 | - | -14.330 | -7.330 | 48.677 | -35.905 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `15m` | `VolCompressionBreakout` | `3b` | 9,582 | - | -13.587 | -6.587 | 83.082 | -15.751 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `15m` | `VolCompressionBreakout` | `6b` | 7,723 | - | -13.018 | -6.018 | 113.510 | -10.112 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `15m` | `VolCompressionBreakout` | `12b` | 6,562 | - | -11.914 | -4.914 | 163.667 | -5.918 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `15m` | `TsmomVolumeWeighted` | `1b` | 434,060 | - | -14.271 | -7.271 | 62.399 | -159.193 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `15m` | `TsmomVolumeWeighted` | `3b` | 182,912 | - | -14.425 | -7.425 | 99.396 | -62.320 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `15m` | `TsmomVolumeWeighted` | `6b` | 105,447 | - | -14.398 | -7.398 | 136.667 | -35.809 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `15m` | `TsmomVolumeWeighted` | `12b` | 59,198 | - | -13.598 | -6.598 | 185.484 | -18.148 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `15m` | `FundingCrowdingReversal` | `1b` | 15,181 | - | -14.530 | -7.530 | 54.719 | -31.495 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `15m` | `FundingCrowdingReversal` | `3b` | 15,181 | - | -12.914 | -5.914 | 90.809 | -17.845 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `15m` | `FundingCrowdingReversal` | `6b` | 15,181 | - | -11.917 | -4.917 | 129.069 | -11.070 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `15m` | `FundingCrowdingReversal` | `12b` | 15,181 | - | -8.168 | -1.168 | 168.629 | -5.815 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `1h` | `VolCompressionBreakout` | `1b` | 8,652 | - | -15.327 | -8.327 | 82.425 | -17.211 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `1h` | `VolCompressionBreakout` | `3b` | 4,880 | - | -14.880 | -7.880 | 142.975 | -7.308 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `1h` | `VolCompressionBreakout` | `6b` | 3,715 | - | -14.466 | -7.466 | 209.198 | -4.244 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `1h` | `VolCompressionBreakout` | `12b` | 3,014 | - | -2.073 | 4.927 | 311.483 | -0.370 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `1h` | `TsmomVolumeWeighted` | `1b` | 153,880 | - | -13.622 | -6.622 | 109.411 | -48.785 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `1h` | `TsmomVolumeWeighted` | `3b` | 58,495 | - | -13.578 | -6.578 | 183.160 | -17.455 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `1h` | `TsmomVolumeWeighted` | `6b` | 31,328 | - | -12.589 | -5.589 | 257.259 | -8.214 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `1h` | `TsmomVolumeWeighted` | `12b` | 16,457 | - | -13.278 | -6.278 | 363.780 | -4.644 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `1h` | `FundingCrowdingReversal` | `1b` | 4,270 | - | -14.355 | -7.355 | 113.916 | -8.259 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `1h` | `FundingCrowdingReversal` | `3b` | 4,270 | - | -11.531 | -4.531 | 173.251 | -3.787 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `1h` | `FundingCrowdingReversal` | `6b` | 4,113 | - | -13.140 | -6.140 | 252.553 | -3.257 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `1h` | `FundingCrowdingReversal` | `12b` | 2,783 | - | -6.315 | 0.685 | 391.060 | -0.772 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `4h` | `VolCompressionBreakout` | `1b` | 3,332 | - | -6.291 | 0.709 | 152.996 | -2.284 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `4h` | `VolCompressionBreakout` | `3b` | 1,722 | 18,833 | 7.276 | 14.276 | 285.272 | 1.013 | 3.5000 | 3.6353 | 0.0156 | `InsufficientSample` | `InsufficientSample` |
| `4h` | `VolCompressionBreakout` | `6b` | 1,221 | 1,231 | 41.568 | 48.568 | 416.620 | 3.337 | 3.5000 | 3.6353 | 0.7916 | `InsufficientSample` | `InsufficientSample` |
| `4h` | `VolCompressionBreakout` | `12b` | 921 | 87,390 | 7.244 | 14.244 | 611.884 | 0.350 | 3.5000 | 3.6353 | 0.0239 | `InsufficientSample` | `InsufficientSample` |
| `4h` | `TsmomVolumeWeighted` | `1b` | 45,552 | - | -12.217 | -5.217 | 205.791 | -13.270 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `4h` | `TsmomVolumeWeighted` | `3b` | 16,345 | - | -10.984 | -3.984 | 359.620 | -4.186 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `4h` | `TsmomVolumeWeighted` | `6b` | 8,443 | - | -14.524 | -7.524 | 520.019 | -2.728 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `4h` | `TsmomVolumeWeighted` | `12b` | 4,281 | - | -39.264 | -32.264 | 687.372 | -3.838 | 3.5000 | 3.6353 | 0.0000 | `Fail` | `Fail` |
| `4h` | `FundingCrowdingReversal` | `1b` | 724 | 1,621,017 | 0.903 | 7.903 | 328.366 | 0.071 | 3.5000 | 3.6353 | 0.0217 | `InsufficientSample` | `InsufficientSample` |
| `4h` | `FundingCrowdingReversal` | `3b` | 537 | 539,210 | 2.791 | 9.791 | 585.475 | 0.106 | 3.5000 | 3.6353 | 0.0448 | `InsufficientSample` | `InsufficientSample` |
| `4h` | `FundingCrowdingReversal` | `6b` | 469 | 312,950 | 6.150 | 13.150 | 983.040 | 0.159 | 3.5000 | 3.6353 | 0.0555 | `InsufficientSample` | `InsufficientSample` |
| `4h` | `FundingCrowdingReversal` | `12b` | 394 | 4,677 | 46.367 | 53.367 | 905.976 | 1.070 | 3.5000 | 3.6353 | 0.2881 | `InsufficientSample` | `InsufficientSample` |

**Resumen estadístico del barrido principal (36 celdas bajo `VIP-0 + 1.5 bps`):**
- **`Pass` (`M=12` o `M=36`)**: **`0 / 36`**.
- **`Fail` (`Mean_VIP0_bps <= 0`)**: **`29 / 36`** (`12/12` en `15m`, `12/12` en `1h`, `5/12` en `4h`).
- **`InsufficientSample` (`Mean_VIP0_bps > 0`, `N < n_min`)**: **`7 / 36`**, todas en `4h` (`VolCompressionBreakout` en `3b, 6b, 12b` y `FundingCrowdingReversal` en `1b, 3b, 6b, 12b`).
- **Celda más fuerte del barrido**: `4h | VolCompressionBreakout | 6b` con `Mean_VIP0_bps = +41.568`, `t_NW = 3.337` (`< 3.5000` y `< 3.6353`), `DSR = 0.7916` (`< 0.95`), `N = 1,221 < n_min = 1,231` (`n_min_36 = 1,328`), veredicto `InsufficientSample`.
- **Celda de mayor media bruta**: `4h | FundingCrowdingReversal | 12b` con `Mean_VIP0_bps = +46.367`, pero con alta dispersión (`Std = 905.976`), `N = 394` frente a `n_min = 4,677` (`n_min_36 = 5,046`), `t_NW = 1.070`, `DSR = 0.2881`.

---

## 3. Matriz de Estabilidad Temporal OOS (Año 1 vs. Año 2)

Se incluyen **todas las 7 celdas con `Mean_VIP0_bps > 0` o `t_NW >= 2.0`** en el barrido principal (todas en `4h`), ampliadas hasta el **Top 10 ordenado por `t_NW` descendente** para máxima visibilidad (`--slippage-bps 1.5`, `VIP-0`).

| # | TF | Hypothesis | Horizon | 24m N | 24m Mean (bps) | 24m t_NW | 24m DSR | Y1 N | Y1 Mean (bps) | Y1 t_NW | Y1 DSR | Y1 Verdict | Y2 N | Y2 Mean (bps) | Y2 t_NW | Y2 DSR | Y2 Verdict |
|---:|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|---:|---|
| 1 | `4h` | `VolCompressionBreakout` | `6b` | 1,221 | 41.568 | 3.337 | 0.7916 | 641 | 61.270 | 3.181 | 0.7192 | `InsufficientSample` | 548 | 6.409 | 0.420 | 0.2046 | `InsufficientSample` |
| 2 | `4h` | `FundingCrowdingReversal` | `12b` | 394 | 46.367 | 1.070 | 0.2881 | 160 | 76.689 | 1.186 | 0.3884 | `InsufficientSample` | 676 | 17.992 | 0.694 | 0.2304 | `InsufficientSample` |
| 3 | `4h` | `VolCompressionBreakout` | `3b` | 1,722 | 7.276 | 1.013 | 0.0156 | 912 | 14.949 | 1.371 | 0.0271 | `InsufficientSample` | 754 | -6.858 | -0.757 | 0.0144 | `Fail` |
| 4 | `4h` | `VolCompressionBreakout` | `12b` | 921 | 7.244 | 0.350 | 0.0239 | 466 | -9.825 | -0.279 | 0.0036 | `Fail` | 428 | 1.595 | 0.072 | 0.1538 | `InsufficientSample` |
| 5 | `4h` | `FundingCrowdingReversal` | `6b` | 469 | 6.150 | 0.159 | 0.0555 | 197 | -32.390 | -0.823 | 0.0117 | `Fail` | 883 | 1.650 | 0.078 | 0.0641 | `InsufficientSample` |
| 6 | `4h` | `FundingCrowdingReversal` | `3b` | 537 | 2.791 | 0.106 | 0.0448 | 226 | -20.826 | -0.758 | 0.0104 | `Fail` | 1,029 | 1.236 | 0.090 | 0.0536 | `InsufficientSample` |
| 7 | `4h` | `FundingCrowdingReversal` | `1b` | 724 | 0.903 | 0.071 | 0.0217 | 312 | -24.086 | -1.629 | 0.0002 | `Fail` | 1,385 | -5.096 | -0.653 | 0.0071 | `Fail` |
| 8 | `1h` | `VolCompressionBreakout` | `12b` | 3,014 | -2.073 | -0.370 | 0.0000 | 1,525 | -7.091 | -0.844 | 0.0000 | `Fail` | 1,479 | 4.877 | 0.664 | 0.0002 | `InsufficientSample` |
| 9 | `1h` | `FundingCrowdingReversal` | `12b` | 2,783 | -6.315 | -0.772 | 0.0000 | 1,236 | -13.612 | -1.171 | 0.0000 | `Fail` | 3,783 | -0.543 | -0.096 | 0.0000 | `Fail` |
| 10 | `4h` | `VolCompressionBreakout` | `1b` | 3,332 | -6.291 | -2.284 | 0.0000 | 1,811 | -7.191 | -1.799 | 0.0000 | `Fail` | 1,403 | -6.541 | -1.693 | 0.0001 | `Fail` |

### Hallazgos Críticos de Estabilidad OOS

1. **Colapso interanual de `4h | VolCompressionBreakout | 6b`**: En el Año 1 (`2024-11-25`–`2025-11-25`) concentró casi todo el efecto (`N = 641`, `Mean = +61.270 bps`, `t_NW = 3.181`, `DSR = 0.7192`). En el Año 2 (`2025-11-25`–`2026-11-25`) el edge se **derrumbó un 89.5%** a `Mean = +6.409 bps` (`N = 548`, `t_NW = 0.420`, `DSR = 0.2046`). Incluso sin exigir `t_crit_36`, la señal no es estable fuera de muestra.
2. **Inversión de signo en `VolCompressionBreakout 3b` y `12b`**: `3b` pasa de `+14.949 bps` (`t = 1.371`) en Y1 a `-6.858 bps` (`t = -0.757`) en Y2; `12b` pasa de `-9.825 bps` en Y1 a `+1.595 bps` en Y2.
3. **Efecto del régimen Welford en `FundingCrowdingReversal` (`N_Y1 + N_Y2 > N_24m`)**: En `4h FundingCrowdingReversal`, `12b` tiene `N = 394` en `24m`, frente a `160` (Y1) + `676` (Y2) = `836` en las corridas aisladas. Causa exacta verificada en [`src/domain/signals/meso_energy.rs`](../src/domain/signals/meso_energy.rs#L242-L259) y los datos crudos de funding: en Y2 el nivel y desviación estándar del funding cayeron ~35–40% respecto a Y1 (ej. `BTCUSDT` bajó de `mean=0.575, std=0.726 bps` en Y1 a `0.258, 0.437 bps` en Y2; `NEARUSDT` de `0.624, 1.412` a `0.284, 0.938`). En la corrida continua de `24m`, la varianza expansiva acumulada en Y1 amortiguó los z-scores de Y2, mientras que al reiniciar Welford al inicio de Y2 se dispararon muchas más señales sobre un régimen de baja amplitud (`Y2 12b: +17.992 bps, t=0.694` vs `Y1 12b: +76.689 bps, t=1.186`).
4. **Cero celdas estables**: Ninguna de las 36 celdas logra `Mean_VIP0_bps > 0` con `t_NW >= 2.0` en ambos años ni aprueba `Verdict_VIP0 == Pass` en ningún subperiodo.

---

## 4. Auditoría de Capital `$64 USDT` (12 Pares vs. Ex-BTC) y Desglose por Símbolo

### 4.1 Universo Completo (12 pares) vs. Ex-BTC (11 pares viables a `$64 USDT`)

| TF | Hypothesis | Horizon | 12-Pares N | 12-Pares Mean | 12-Pares t_NW | 12-Pares Verdict | Ex-BTC N | Ex-BTC Mean | Ex-BTC t_NW | Ex-BTC DSR | Ex-BTC Verdict | Ex-BTC Global36 |
|---|---|---|---:|---:|---:|---|---:|---:|---:|---:|---|---|
| `15m` | `VolCompressionBreakout` | `1b` | 14,994 | -14.330 | -35.905 | `Fail` | 14,484 | -14.308 | -34.797 | 0.0000 | `Fail` | `Fail` |
| `15m` | `VolCompressionBreakout` | `3b` | 9,582 | -13.587 | -15.751 | `Fail` | 9,203 | -13.508 | -15.106 | 0.0000 | `Fail` | `Fail` |
| `15m` | `VolCompressionBreakout` | `6b` | 7,723 | -13.018 | -10.112 | `Fail` | 7,385 | -12.825 | -9.576 | 0.0000 | `Fail` | `Fail` |
| `15m` | `VolCompressionBreakout` | `12b` | 6,562 | -11.914 | -5.918 | `Fail` | 6,251 | -11.780 | -5.601 | 0.0000 | `Fail` | `Fail` |
| `15m` | `TsmomVolumeWeighted` | `1b` | 434,060 | -14.271 | -159.193 | `Fail` | 412,816 | -14.280 | -152.527 | 0.0000 | `Fail` | `Fail` |
| `15m` | `TsmomVolumeWeighted` | `3b` | 182,912 | -14.425 | -62.320 | `Fail` | 173,078 | -14.428 | -59.400 | 0.0000 | `Fail` | `Fail` |
| `15m` | `TsmomVolumeWeighted` | `6b` | 105,447 | -14.398 | -35.809 | `Fail` | 99,232 | -14.440 | -34.041 | 0.0000 | `Fail` | `Fail` |
| `15m` | `TsmomVolumeWeighted` | `12b` | 59,198 | -13.598 | -18.148 | `Fail` | 55,322 | -13.536 | -17.020 | 0.0000 | `Fail` | `Fail` |
| `15m` | `FundingCrowdingReversal` | `1b` | 15,181 | -14.530 | -31.495 | `Fail` | 13,870 | -14.656 | -29.225 | 0.0000 | `Fail` | `Fail` |
| `15m` | `FundingCrowdingReversal` | `3b` | 15,181 | -12.914 | -17.845 | `Fail` | 13,870 | -12.980 | -16.497 | 0.0000 | `Fail` | `Fail` |
| `15m` | `FundingCrowdingReversal` | `6b` | 15,181 | -11.917 | -11.070 | `Fail` | 13,870 | -11.898 | -10.213 | 0.0000 | `Fail` | `Fail` |
| `15m` | `FundingCrowdingReversal` | `12b` | 15,181 | -8.168 | -5.815 | `Fail` | 13,870 | -7.830 | -5.131 | 0.0000 | `Fail` | `Fail` |
| `1h` | `VolCompressionBreakout` | `1b` | 8,652 | -15.327 | -17.211 | `Fail` | 8,220 | -15.434 | -16.544 | 0.0000 | `Fail` | `Fail` |
| `1h` | `VolCompressionBreakout` | `3b` | 4,880 | -14.880 | -7.308 | `Fail` | 4,591 | -14.404 | -6.702 | 0.0000 | `Fail` | `Fail` |
| `1h` | `VolCompressionBreakout` | `6b` | 3,715 | -14.466 | -4.244 | `Fail` | 3,470 | -14.044 | -3.907 | 0.0000 | `Fail` | `Fail` |
| `1h` | `VolCompressionBreakout` | `12b` | 3,014 | -2.073 | -0.370 | `Fail` | 2,797 | -0.631 | -0.106 | 0.0000 | `Fail` | `Fail` |
| `1h` | `TsmomVolumeWeighted` | `1b` | 153,880 | -13.622 | -48.785 | `Fail` | 144,670 | -13.621 | -46.195 | 0.0000 | `Fail` | `Fail` |
| `1h` | `TsmomVolumeWeighted` | `3b` | 58,495 | -13.578 | -17.455 | `Fail` | 54,728 | -13.519 | -16.386 | 0.0000 | `Fail` | `Fail` |
| `1h` | `TsmomVolumeWeighted` | `6b` | 31,328 | -12.589 | -8.214 | `Fail` | 29,154 | -12.357 | -7.571 | 0.0000 | `Fail` | `Fail` |
| `1h` | `TsmomVolumeWeighted` | `12b` | 16,457 | -13.278 | -4.644 | `Fail` | 15,234 | -12.973 | -4.233 | 0.0000 | `Fail` | `Fail` |
| `1h` | `FundingCrowdingReversal` | `1b` | 4,270 | -14.355 | -8.259 | `Fail` | 3,691 | -14.256 | -7.188 | 0.0000 | `Fail` | `Fail` |
| `1h` | `FundingCrowdingReversal` | `3b` | 4,270 | -11.531 | -3.787 | `Fail` | 3,691 | -11.291 | -3.232 | 0.0000 | `Fail` | `Fail` |
| `1h` | `FundingCrowdingReversal` | `6b` | 4,113 | -13.140 | -3.257 | `Fail` | 3,534 | -12.885 | -2.779 | 0.0000 | `Fail` | `Fail` |
| `1h` | `FundingCrowdingReversal` | `12b` | 2,783 | -6.315 | -0.772 | `Fail` | 2,401 | -6.521 | -0.693 | 0.0000 | `Fail` | `Fail` |
| `4h` | `VolCompressionBreakout` | `1b` | 3,332 | -6.291 | -2.284 | `Fail` | 3,079 | -5.917 | -2.002 | 0.0000 | `Fail` | `Fail` |
| `4h` | `VolCompressionBreakout` | `3b` | 1,722 | 7.276 | 1.013 | `InsufficientSample` | 1,586 | 8.478 | 1.095 | 0.0223 | `InsufficientSample` | `InsufficientSample` |
| `4h` | `VolCompressionBreakout` | `6b` | 1,221 | 41.568 | 3.337 | `InsufficientSample` | 1,114 | 44.706 | 3.305 | 0.7985 | `InsufficientSample` | `InsufficientSample` |
| `4h` | `VolCompressionBreakout` | `12b` | 921 | 7.244 | 0.350 | `InsufficientSample` | 841 | 9.396 | 0.417 | 0.0318 | `InsufficientSample` | `InsufficientSample` |
| `4h` | `TsmomVolumeWeighted` | `1b` | 45,552 | -12.217 | -13.270 | `Fail` | 42,339 | -11.987 | -12.213 | 0.0000 | `Fail` | `Fail` |
| `4h` | `TsmomVolumeWeighted` | `3b` | 16,345 | -10.984 | -4.186 | `Fail` | 15,125 | -10.913 | -3.881 | 0.0000 | `Fail` | `Fail` |
| `4h` | `TsmomVolumeWeighted` | `6b` | 8,443 | -14.524 | -2.728 | `Fail` | 7,782 | -14.732 | -2.568 | 0.0000 | `Fail` | `Fail` |
| `4h` | `TsmomVolumeWeighted` | `12b` | 4,281 | -39.264 | -3.838 | `Fail` | 3,935 | -42.360 | -3.844 | 0.0000 | `Fail` | `Fail` |
| `4h` | `FundingCrowdingReversal` | `1b` | 724 | 0.903 | 0.071 | `InsufficientSample` | 701 | 0.998 | 0.076 | 0.0212 | `InsufficientSample` | `InsufficientSample` |
| `4h` | `FundingCrowdingReversal` | `3b` | 537 | 2.791 | 0.106 | `InsufficientSample` | 516 | 4.071 | 0.148 | 0.0479 | `InsufficientSample` | `InsufficientSample` |
| `4h` | `FundingCrowdingReversal` | `6b` | 469 | 6.150 | 0.159 | `InsufficientSample` | 449 | 10.703 | 0.265 | 0.0617 | `InsufficientSample` | `InsufficientSample` |
| `4h` | `FundingCrowdingReversal` | `12b` | 394 | 46.367 | 1.070 | `InsufficientSample` | 376 | 50.852 | 1.126 | 0.3070 | `InsufficientSample` | `InsufficientSample` |

### 4.2 Conteo de Positividad Individual por Símbolo (36 Celdas × 12 Símbolos = 432 Evaluaciones)

| TF | Hypothesis | Horizon | Pares `Mean > 0` | Pares `Pass` | Mejor Símbolo (`Mean_VIP0_bps`, `t_NW`, `Verdict`) |
|---|---|---|---:|---:|---|
| `15m` | `VolCompressionBreakout` | `1b` | 0/12 | 0/12 | `ORDIUSDT` (-11.042 bps, t=-8.223, `Fail`) |
| `15m` | `VolCompressionBreakout` | `3b` | 0/12 | 0/12 | `ORDIUSDT` (-5.982 bps, t=-1.996, `Fail`) |
| `15m` | `VolCompressionBreakout` | `6b` | 0/12 | 0/12 | `ORDIUSDT` (-4.937 bps, t=-1.145, `Fail`) |
| `15m` | `VolCompressionBreakout` | `12b` | 0/12 | 0/12 | `ORDIUSDT` (-0.352 bps, t=-0.053, `Fail`) |
| `15m` | `TsmomVolumeWeighted` | `1b` | 0/12 | 0/12 | `ORDIUSDT` (-14.017 bps, t=-37.160, `Fail`) |
| `15m` | `TsmomVolumeWeighted` | `3b` | 0/12 | 0/12 | `ORDIUSDT` (-13.286 bps, t=-13.631, `Fail`) |
| `15m` | `TsmomVolumeWeighted` | `6b` | 0/12 | 0/12 | `WIFUSDT` (-10.146 bps, t=-6.142, `Fail`) |
| `15m` | `TsmomVolumeWeighted` | `12b` | 0/12 | 0/12 | `ORDIUSDT` (-6.498 bps, t=-1.815, `Fail`) |
| `15m` | `FundingCrowdingReversal` | `1b` | 0/12 | 0/12 | `TAOUSDT` (-9.084 bps, t=-1.884, `Fail`) |
| `15m` | `FundingCrowdingReversal` | `3b` | 0/12 | 0/12 | `TAOUSDT` (-4.950 bps, t=-0.705, `Fail`) |
| `15m` | `FundingCrowdingReversal` | `6b` | 1/12 | 0/12 | `TAOUSDT` (9.607 bps, t=0.961, `InsufficientSample`) |
| `15m` | `FundingCrowdingReversal` | `12b` | 1/12 | 0/12 | `TAOUSDT` (23.913 bps, t=1.648, `InsufficientSample`) |
| `1h` | `VolCompressionBreakout` | `1b` | 0/12 | 0/12 | `TAOUSDT` (-11.387 bps, t=-2.908, `Fail`) |
| `1h` | `VolCompressionBreakout` | `3b` | 0/12 | 0/12 | `LTCUSDT` (-5.761 bps, t=-1.055, `Fail`) |
| `1h` | `VolCompressionBreakout` | `6b` | 0/12 | 0/12 | `SUIUSDT` (-3.861 bps, t=-0.306, `Fail`) |
| `1h` | `VolCompressionBreakout` | `12b` | 5/12 | 0/12 | `WIFUSDT` (34.458 bps, t=1.700, `InsufficientSample`) |
| `1h` | `TsmomVolumeWeighted` | `1b` | 0/12 | 0/12 | `ORDIUSDT` (-11.284 bps, t=-8.892, `Fail`) |
| `1h` | `TsmomVolumeWeighted` | `3b` | 0/12 | 0/12 | `ORDIUSDT` (-6.036 bps, t=-1.578, `Fail`) |
| `1h` | `TsmomVolumeWeighted` | `6b` | 1/12 | 0/12 | `ORDIUSDT` (1.761 bps, t=0.217, `InsufficientSample`) |
| `1h` | `TsmomVolumeWeighted` | `12b` | 1/12 | 0/12 | `ORDIUSDT` (17.418 bps, t=1.000, `InsufficientSample`) |
| `1h` | `FundingCrowdingReversal` | `1b` | 0/12 | 0/12 | `TAOUSDT` (-3.414 bps, t=-0.351, `Fail`) |
| `1h` | `FundingCrowdingReversal` | `3b` | 2/12 | 0/12 | `TAOUSDT` (12.193 bps, t=0.707, `InsufficientSample`) |
| `1h` | `FundingCrowdingReversal` | `6b` | 1/12 | 0/12 | `ORDIUSDT` (5.412 bps, t=0.173, `InsufficientSample`) |
| `1h` | `FundingCrowdingReversal` | `12b` | 5/12 | 0/12 | `TAOUSDT` (45.064 bps, t=0.842, `InsufficientSample`) |
| `4h` | `VolCompressionBreakout` | `1b` | 4/12 | 0/12 | `NEARUSDT` (10.503 bps, t=1.084, `InsufficientSample`) |
| `4h` | `VolCompressionBreakout` | `3b` | 7/12 | 0/12 | `NEARUSDT` (43.593 bps, t=1.981, `InsufficientSample`) |
| `4h` | `VolCompressionBreakout` | `6b` | 11/12 | 0/12 | `SUIUSDT` (129.498 bps, t=2.912, `InsufficientSample`) |
| `4h` | `VolCompressionBreakout` | `12b` | 5/12 | 0/12 | `LINKUSDT` (131.984 bps, t=2.078, `InsufficientSample`) |
| `4h` | `TsmomVolumeWeighted` | `1b` | 0/12 | 0/12 | `WIFUSDT` (-7.240 bps, t=-1.891, `Fail`) |
| `4h` | `TsmomVolumeWeighted` | `3b` | 2/12 | 0/12 | `SUIUSDT` (2.484 bps, t=0.275, `InsufficientSample`) |
| `4h` | `TsmomVolumeWeighted` | `6b` | 1/12 | 0/12 | `WIFUSDT` (7.381 bps, t=0.321, `InsufficientSample`) |
| `4h` | `TsmomVolumeWeighted` | `12b` | 0/12 | 0/12 | `BTCUSDT` (-4.056 bps, t=-0.237, `Fail`) |
| `4h` | `FundingCrowdingReversal` | `1b` | 3/12 | 0/12 | `NEARUSDT` (19.012 bps, t=0.709, `InsufficientSample`) |
| `4h` | `FundingCrowdingReversal` | `3b` | 6/12 | 0/12 | `NEARUSDT` (61.793 bps, t=1.402, `InsufficientSample`) |
| `4h` | `FundingCrowdingReversal` | `6b` | 3/12 | 0/12 | `NEARUSDT` (88.263 bps, t=1.116, `InsufficientSample`) |
| `4h` | `FundingCrowdingReversal` | `12b` | 6/12 | 0/12 | `ORDIUSDT` (193.519 bps, t=1.605, `InsufficientSample`) |

### 4.3 Desglose Completo por Símbolo para las Celdas `4h` Más Destacadas (`VIP-0 + 1.5 bps`)

Se muestran `4h | VolCompressionBreakout | 6b` (celda de mayor `t_NW` global) y `4h | FundingCrowdingReversal | 12b` (celda de mayor media en bps), demostrando si el comportamiento proviene de difusión transversal o concentración idiosincrática:

#### `4h | VolCompressionBreakout | 6b` (11/12 pares positivos, `0/12 Pass`)

| ID | Símbolo | N | n_min | Mean_VIP0_bps | Mean_DiagZero_bps | Std_bps | t_NW | DSR | Verdict |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---|
| 0 | `BTCUSDT` | 107 | 4,617 | 8.892 | 15.892 | 172.617 | 0.476 | 0.1027 | `InsufficientSample` |
| 1 | `ETHUSDT` | 111 | - | -18.027 | -11.027 | 252.416 | -0.721 | 0.0100 | `Fail` |
| 2 | `SOLUSDT` | 106 | 1,255 | 34.159 | 41.159 | 345.716 | 0.972 | 0.4246 | `InsufficientSample` |
| 3 | `AVAXUSDT` | 98 | 292,723 | 2.286 | 9.286 | 353.437 | 0.057 | 0.1755 | `InsufficientSample` |
| 4 | `TAOUSDT` | 88 | 30,066 | 10.982 | 17.982 | 544.083 | 0.185 | 0.0706 | `InsufficientSample` |
| 5 | `SUIUSDT` | 95 | 110 | 129.498 | 136.498 | 387.841 | 2.912 | 0.8591 | `InsufficientSample` |
| 6 | `ORDIUSDT` | 112 | 3,253 | 34.731 | 41.731 | 565.932 | 0.678 | 0.2452 | `InsufficientSample` |
| 7 | `PENDLEUSDT` | 93 | 815 | 56.851 | 63.851 | 463.429 | 1.277 | 0.4800 | `InsufficientSample` |
| 8 | `WIFUSDT` | 100 | 1,596 | 42.636 | 49.636 | 486.603 | 0.932 | 0.4187 | `InsufficientSample` |
| 9 | `LINKUSDT` | 112 | 431 | 82.627 | 89.627 | 489.764 | 1.906 | 0.1719 | `InsufficientSample` |
| 10 | `NEARUSDT` | 89 | 259 | 89.336 | 96.336 | 410.763 | 2.815 | 0.8028 | `InsufficientSample` |
| 11 | `LTCUSDT` | 110 | 1,239 | 36.770 | 43.770 | 369.758 | 0.933 | 0.2440 | `InsufficientSample` |

#### `4h | FundingCrowdingReversal | 12b` (6/12 pares positivos, `0/12 Pass` — fuerte dispersión y concentración en `TAOUSDT`/`ORDIUSDT`)

| ID | Símbolo | N | n_min | Mean_VIP0_bps | Mean_DiagZero_bps | Std_bps | t_NW | DSR | Verdict |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---|
| 0 | `BTCUSDT` | 18 | - | -47.321 | -40.321 | 319.031 | -0.650 | 0.0840 | `Fail` |
| 1 | `ETHUSDT` | 13 | 25,001 | 11.891 | 18.891 | 537.166 | 0.102 | 0.3234 | `InsufficientSample` |
| 2 | `SOLUSDT` | 25 | 546 | 92.860 | 99.860 | 619.775 | 0.768 | 0.5535 | `InsufficientSample` |
| 3 | `AVAXUSDT` | 21 | 2,353 | 44.629 | 51.629 | 618.494 | 0.292 | 0.4478 | `InsufficientSample` |
| 4 | `TAOUSDT` | 26 | 151 | 323.517 | 330.517 | 1133.625 | 1.600 | 0.7356 | `InsufficientSample` |
| 5 | `SUIUSDT` | 18 | - | -84.388 | -77.388 | 751.657 | -0.584 | 0.0845 | `Fail` |
| 6 | `ORDIUSDT` | 86 | 575 | 193.519 | 200.519 | 1324.847 | 1.605 | 0.6039 | `InsufficientSample` |
| 7 | `PENDLEUSDT` | 63 | - | -27.513 | -20.513 | 714.603 | -0.339 | 0.0966 | `Fail` |
| 8 | `WIFUSDT` | 35 | - | -29.529 | -22.529 | 846.930 | -0.188 | 0.2041 | `Fail` |
| 9 | `LINKUSDT` | 12 | - | -245.229 | -238.229 | 950.410 | -1.245 | 0.0239 | `Fail` |
| 10 | `NEARUSDT` | 52 | 4,812,996 | 1.193 | 8.193 | 748.089 | 0.013 | 0.1676 | `InsufficientSample` |
| 11 | `LTCUSDT` | 25 | - | -87.221 | -80.221 | 596.667 | -0.741 | 0.0635 | `Fail` |

- **Diagnóstico `$64 USDT` / Ex-BTC**: Al excluir `BTCUSDT`, `4h | VolCompressionBreakout | 6b` mejora ligeramente en media (`+44.706` vs `+41.568 bps`), pero cae levemente en `t_NW` (`3.305` vs `3.337`) y sigue en `InsufficientSample` (`N = 1,114 < n_min = 1,149`). En `15m` y `1h`, la exclusión de `BTCUSDT` deja intacta la pérdida sistemática (`12/12` negativas en `15m` y `12/12` en `1h`).
- **Positividad individual**: De las `432` evaluaciones individuales `(símbolo × celda)`, **`0 / 432`** alcanzan `Verdict_VIP0 == Pass` (máximo `t_NW` individual en todo el universo: `SUIUSDT` en `4h VolCompressionBreakout 6b` con `t_NW = 2.912`, `DSR = 0.8591`, `N = 95 < n_min = 110`).

---

## 5. Sensibilidad de Slippage (`0.5` vs `1.5` vs `3.0 bps`) y `VIP-0` vs `DiagZero`

Esta sección separa explícitamente **"cero alpha bruto"** (retorno esperado negativo incluso bajo `DiagZero`, que aplica `2.00 + 2.00 = 4.00 bps` RT taker vs `11.00 bps` RT de `VIP-0`) de **"alpha bruto positivo destruido por tarifa VIP-0"**.

| TF | Hypothesis | Horizon | VIP-0 (0.5 bps) | VIP-0 (1.5 bps*) | VIP-0 (3.0 bps) | DiagZero (0.5 bps) | DiagZero (1.5 bps) | DiagZero (3.0 bps) | Diagnóstico de Fricción (a `1.5 bps`) |
|---|---|---|---:|---:|---:|---:|---:|---:|---|
| `15m` | `VolCompressionBreakout` | `1b` | -12.330 (`-30.89`) | -14.330 (`-35.90`) | -17.330 (`-43.42`) | -5.330 (`-13.35`) | -7.330 (`-18.37`) | -10.330 (`-25.88`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `15m` | `VolCompressionBreakout` | `3b` | -11.587 (`-13.43`) | -13.587 (`-15.75`) | -16.587 (`-19.23`) | -4.587 (`-5.32`) | -6.587 (`-7.64`) | -9.587 (`-11.11`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `15m` | `VolCompressionBreakout` | `6b` | -11.018 (`-8.56`) | -13.018 (`-10.11`) | -16.018 (`-12.44`) | -4.018 (`-3.12`) | -6.018 (`-4.67`) | -9.018 (`-7.00`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `15m` | `VolCompressionBreakout` | `12b` | -9.914 (`-4.92`) | -11.914 (`-5.92`) | -14.914 (`-7.41`) | -2.914 (`-1.45`) | -4.914 (`-2.44`) | -7.914 (`-3.93`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `15m` | `TsmomVolumeWeighted` | `1b` | -12.271 (`-136.88`) | -14.271 (`-159.19`) | -17.271 (`-192.66`) | -5.271 (`-58.80`) | -7.271 (`-81.11`) | -10.271 (`-114.57`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `15m` | `TsmomVolumeWeighted` | `3b` | -12.425 (`-53.68`) | -14.425 (`-62.32`) | -17.425 (`-75.28`) | -5.425 (`-23.44`) | -7.425 (`-32.08`) | -10.425 (`-45.04`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `15m` | `TsmomVolumeWeighted` | `6b` | -12.398 (`-30.83`) | -14.398 (`-35.81`) | -17.398 (`-43.27`) | -5.398 (`-13.43`) | -7.398 (`-18.40`) | -10.398 (`-25.86`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `15m` | `TsmomVolumeWeighted` | `12b` | -11.598 (`-15.48`) | -13.598 (`-18.15`) | -16.598 (`-22.15`) | -4.598 (`-6.14`) | -6.598 (`-8.81`) | -9.598 (`-12.81`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `15m` | `FundingCrowdingReversal` | `1b` | -12.530 (`-27.16`) | -14.530 (`-31.49`) | -17.531 (`-37.99`) | -5.530 (`-11.99`) | -7.530 (`-16.32`) | -10.531 (`-22.82`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `15m` | `FundingCrowdingReversal` | `3b` | -10.914 (`-15.08`) | -12.914 (`-17.84`) | -15.914 (`-21.99`) | -3.914 (`-5.41`) | -5.914 (`-8.17`) | -8.914 (`-12.32`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `15m` | `FundingCrowdingReversal` | `6b` | -9.917 (`-9.21`) | -11.917 (`-11.07`) | -14.918 (`-13.86`) | -2.917 (`-2.71`) | -4.917 (`-4.57`) | -7.918 (`-7.35`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `15m` | `FundingCrowdingReversal` | `12b` | -6.169 (`-4.39`) | -8.168 (`-5.81`) | -11.168 (`-7.95`) | 0.831 (`0.59`) | -1.168 (`-0.83`) | -4.168 (`-2.97`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `1h` | `VolCompressionBreakout` | `1b` | -13.327 (`-14.97`) | -15.327 (`-17.21`) | -18.326 (`-20.58`) | -6.327 (`-7.10`) | -8.327 (`-9.35`) | -11.326 (`-12.72`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `1h` | `VolCompressionBreakout` | `3b` | -12.880 (`-6.33`) | -14.880 (`-7.31`) | -17.879 (`-8.78`) | -5.880 (`-2.89`) | -7.880 (`-3.87`) | -10.879 (`-5.34`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `1h` | `VolCompressionBreakout` | `6b` | -12.467 (`-3.66`) | -14.466 (`-4.24`) | -17.464 (`-5.12`) | -5.467 (`-1.60`) | -7.466 (`-2.19`) | -10.464 (`-3.07`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `1h` | `VolCompressionBreakout` | `12b` | -0.075 (`-0.01`) | -2.073 (`-0.37`) | -5.070 (`-0.90`) | 6.925 (`1.23`) | 4.927 (`0.88`) | 1.930 (`0.34`) | Alpha bruto mínimo (+4.93 bps) destruido por VIP-0 (t_D=0.88) |
| `1h` | `TsmomVolumeWeighted` | `1b` | -11.622 (`-41.62`) | -13.622 (`-48.78`) | -16.622 (`-59.53`) | -4.622 (`-16.55`) | -6.622 (`-23.72`) | -9.622 (`-34.46`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `1h` | `TsmomVolumeWeighted` | `3b` | -11.578 (`-14.88`) | -13.578 (`-17.45`) | -16.578 (`-21.31`) | -4.578 (`-5.89`) | -6.578 (`-8.46`) | -9.578 (`-12.31`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `1h` | `TsmomVolumeWeighted` | `6b` | -10.589 (`-6.91`) | -12.589 (`-8.21`) | -15.588 (`-10.17`) | -3.589 (`-2.34`) | -5.589 (`-3.65`) | -8.588 (`-5.60`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `1h` | `TsmomVolumeWeighted` | `12b` | -11.278 (`-3.94`) | -13.278 (`-4.64`) | -16.277 (`-5.69`) | -4.278 (`-1.50`) | -6.278 (`-2.20`) | -9.277 (`-3.24`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `1h` | `FundingCrowdingReversal` | `1b` | -12.354 (`-7.11`) | -14.355 (`-8.26`) | -17.356 (`-9.99`) | -5.354 (`-3.08`) | -7.355 (`-4.23`) | -10.356 (`-5.96`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `1h` | `FundingCrowdingReversal` | `3b` | -9.530 (`-3.13`) | -11.531 (`-3.79`) | -14.532 (`-4.77`) | -2.530 (`-0.83`) | -4.531 (`-1.49`) | -7.532 (`-2.47`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `1h` | `FundingCrowdingReversal` | `6b` | -11.137 (`-2.76`) | -13.140 (`-3.26`) | -16.143 (`-4.00`) | -4.137 (`-1.03`) | -6.140 (`-1.52`) | -9.143 (`-2.27`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `1h` | `FundingCrowdingReversal` | `12b` | -4.311 (`-0.53`) | -6.315 (`-0.77`) | -9.320 (`-1.14`) | 2.689 (`0.33`) | 0.685 (`0.08`) | -2.320 (`-0.28`) | Alpha bruto mínimo (+0.69 bps) destruido por VIP-0 (t_D=0.08) |
| `4h` | `VolCompressionBreakout` | `1b` | -4.292 (`-1.56`) | -6.291 (`-2.28`) | -9.290 (`-3.37`) | 2.708 (`0.98`) | 0.709 (`0.26`) | -2.290 (`-0.83`) | Alpha bruto mínimo (+0.71 bps) destruido por VIP-0 (t_D=0.26) |
| `4h` | `VolCompressionBreakout` | `3b` | 9.273 (`1.29`) | 7.276 (`1.01`) | 4.279 (`0.60`) | 16.273 (`2.27`) | 14.276 (`1.99`) | 11.279 (`1.57`) | Positivo VIP-0 pero sub-umbral (`InsufficientSample`, t=1.01) |
| `4h` | `VolCompressionBreakout` | `6b` | 43.560 (`3.50`) | 41.568 (`3.34`) | 38.580 (`3.10`) | 50.560 (`4.06`) | 48.568 (`3.90`) | 45.580 (`3.66`) | Positivo VIP-0 pero sub-umbral (`InsufficientSample`, t=3.34) |
| `4h` | `VolCompressionBreakout` | `12b` | 9.226 (`0.45`) | 7.244 (`0.35`) | 4.273 (`0.21`) | 16.226 (`0.78`) | 14.244 (`0.69`) | 11.273 (`0.54`) | Positivo VIP-0 pero sub-umbral (`InsufficientSample`, t=0.35) |
| `4h` | `TsmomVolumeWeighted` | `1b` | -10.217 (`-11.10`) | -12.217 (`-13.27`) | -15.217 (`-16.53`) | -3.217 (`-3.49`) | -5.217 (`-5.67`) | -8.217 (`-8.92`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `4h` | `TsmomVolumeWeighted` | `3b` | -8.985 (`-3.42`) | -10.984 (`-4.19`) | -13.983 (`-5.33`) | -1.985 (`-0.76`) | -3.984 (`-1.52`) | -6.983 (`-2.66`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `4h` | `TsmomVolumeWeighted` | `6b` | -12.526 (`-2.35`) | -14.524 (`-2.73`) | -17.522 (`-3.29`) | -5.526 (`-1.04`) | -7.524 (`-1.41`) | -10.522 (`-1.98`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `4h` | `TsmomVolumeWeighted` | `12b` | -37.267 (`-3.64`) | -39.264 (`-3.84`) | -42.260 (`-4.13`) | -30.267 (`-2.96`) | -32.264 (`-3.15`) | -35.260 (`-3.45`) | Cero alpha bruto (negativo incluso en DiagZero) |
| `4h` | `FundingCrowdingReversal` | `1b` | 2.908 (`0.23`) | 0.903 (`0.07`) | -2.105 (`-0.17`) | 9.908 (`0.78`) | 7.903 (`0.62`) | 4.895 (`0.38`) | Positivo VIP-0 pero sub-umbral (`InsufficientSample`, t=0.07) |
| `4h` | `FundingCrowdingReversal` | `3b` | 4.798 (`0.18`) | 2.791 (`0.11`) | -0.220 (`-0.01`) | 11.798 (`0.45`) | 9.791 (`0.37`) | 6.780 (`0.26`) | Positivo VIP-0 pero sub-umbral (`InsufficientSample`, t=0.11) |
| `4h` | `FundingCrowdingReversal` | `6b` | 8.156 (`0.21`) | 6.150 (`0.16`) | 3.143 (`0.08`) | 15.156 (`0.39`) | 13.150 (`0.34`) | 10.143 (`0.26`) | Positivo VIP-0 pero sub-umbral (`InsufficientSample`, t=0.16) |
| `4h` | `FundingCrowdingReversal` | `12b` | 48.382 (`1.12`) | 46.367 (`1.07`) | 43.345 (`1.00`) | 55.382 (`1.28`) | 53.367 (`1.23`) | 50.345 (`1.16`) | Positivo VIP-0 pero sub-umbral (`InsufficientSample`, t=1.07) |

- **Cero alpha bruto (`26 / 36` celdas a `1.5 bps`)**: En `15m` (`12/12` celdas), `10/12` de `1h` y `4/12` de `4h` (`TsmomVolumeWeighted`), la media ya es negativa bajo `DiagZero` (`7.0 bps` RT totales a `1.5 bps` slip) e incluso `25/36` siguen negativas bajo `DiagZero` a `0.5 bps` (`5.0 bps` RT totales). Esto descarta que el fracaso de `TsmomVolumeWeighted` o de las señales `15m`/`1h` sea un mero artefacto del fee VIP-0: la expectativa bruta por operación es negativa o ~`0 bps` antes de comisiones.
- **Alpha marginal erosionado por tarifa VIP-0 (`3 / 36` celdas a `1.5 bps`)**: `1h VolCompressionBreakout 12b` (`+4.927 bps` DiagZero -> `-2.073 bps` VIP-0), `1h FundingCrowdingReversal 12b` (`+0.685` -> `-6.315`), `4h VolCompressionBreakout 1b` (`+0.709` -> `-6.291`). Ninguna supera `t_NW = 0.88` siquiera en `DiagZero`.
- **Celda límite en sensibilidad (`4h | VolCompressionBreakout | 6b`)**: Incluso bajando el slippage a `0.5 bps` (no vinculante), bajo `VIP-0` obtiene `Mean = +43.560 bps`, `N = 1,221 >= n_min (1,121)`, `t_NW = 3.496 < 3.5000`, `DSR = 0.8535 < 0.95` -> **`Fail`**; y bajo `DiagZero` (`0.5 bps`) obtiene `t_NW = 4.058`, `DSR = 0.9631` (pero `DiagZero` jamás aprueba por diseño, además de sufrir caída del `89.5%` en Año 2 OOS).

---

## 6. Salida Cruda de Compuertas PowerShell + `git status --short`

### 6.1 `cargo fmt --all -- --check`

```text

EXIT_CODE=0
```

### 6.2 `cargo clippy --all-targets --all-features -- -D warnings`

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.16s
EXIT_CODE=0
```

### 6.3 `cargo test --all-targets -- --nocapture`

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.18s
     Running unittests src\lib.rs (target\debug\deps\vortex_alfa-a216a8d1bd8b90ff.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running unittests src\main.rs (target\debug\deps\vortex_alfa-e53f0a43f8dc11a2.exe)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\adversarial_math_tests.rs (target\debug\deps\adversarial_math_tests-1e7b3596f2b1b423.exe)

running 9 tests
test test_acklam_normal_inv_cdf_accuracy ... ok
test test_holm_bonferroni_order_and_monotonicity ... ok
test test_moments4_closed_form_exact ... ok
test test_garman_klass_closed_form_and_flat_bar ... ok
test test_dsr_cross_sectional_std_and_diag_never_passes ... ok
test test_hawkes_o1_three_step_analytical ... ok
test test_newey_west_closed_form_n8 ... ok
test test_exact_funding_boundary_bar_open_vs_bar_close ... ok
test test_vip0_ceil_fee_e8_and_book_roundtrip_asymmetry ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\edge_cases_and_drift_tests.rs (target\debug\deps\edge_cases_and_drift_tests-2a9b90ac347a6026.exe)

running 8 tests
test test_dust_notional_crossed_book_and_invalid_ohlc_rejected ... ok
test test_flat_candle_garman_klass_zero ... ok
test test_flash_crash_minus_30_percent_bar_and_tick ... ok
test test_zero_variance_never_passes_or_nans ... ok
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
```

### 6.4 Conteo de Líneas `src/**/*.rs <= 350`

```text

File                                                              Lines Pass
----                                                              ----- ----
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_meso.rs         330 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_micro.rs        280 True
D:\Desarrollo\VORTEX-ALFA\src\application\mod.rs                      2 True
D:\Desarrollo\VORTEX-ALFA\src\domain\error.rs                        45 True
D:\Desarrollo\VORTEX-ALFA\src\domain\fees.rs                        179 True
D:\Desarrollo\VORTEX-ALFA\src\domain\ledger.rs                      113 True
D:\Desarrollo\VORTEX-ALFA\src\domain\mod.rs                           8 True
D:\Desarrollo\VORTEX-ALFA\src\domain\normal_dist.rs                 217 True
D:\Desarrollo\VORTEX-ALFA\src\domain\ports.rs                        18 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\meso_energy.rs         331 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\micro_detectors.rs     226 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\micro_tail.rs          330 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\mod.rs                   3 True
D:\Desarrollo\VORTEX-ALFA\src\domain\stats.rs                       348 True
D:\Desarrollo\VORTEX-ALFA\src\domain\types.rs                       331 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\binary_codec.rs        342 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\csv_candle_reader.rs   319 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\mod.rs                   4 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\tape_reader.rs         325 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\vkl1_reader.rs         154 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli.rs                     222 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\mod.rs                       1 True
D:\Desarrollo\VORTEX-ALFA\src\lib.rs                                  7 True
D:\Desarrollo\VORTEX-ALFA\src\main.rs                                17 True
```

### 6.5 Búsqueda de Patrones Prohibidos en `src/`

```text
--- Literal glob: Select-String -Path .\src\**\*.rs ---
MATCH_COUNT=0
--- Recursivo completo (24 archivos .rs en .\src): Get-ChildItem -Recurse | Select-String ---
MATCH_COUNT=0
```

### 6.6 Higiene UTF-8 Sin BOM y LF-only (`src/` y `tests/` + Artefactos Fase 1A)

```text

File                          UTF8_NoBOM LF_Only
----                          ---------- -------
falsifier_meso.rs                   True    True
falsifier_micro.rs                  True    True
mod.rs                              True    True
error.rs                            True    True
fees.rs                             True    True
ledger.rs                           True    True
mod.rs                              True    True
normal_dist.rs                      True    True
ports.rs                            True    True
meso_energy.rs                      True    True
micro_detectors.rs                  True    True
micro_tail.rs                       True    True
mod.rs                              True    True
stats.rs                            True    True
types.rs                            True    True
binary_codec.rs                     True    True
csv_candle_reader.rs                True    True
mod.rs                              True    True
tape_reader.rs                      True    True
vkl1_reader.rs                      True    True
cli.rs                              True    True
mod.rs                              True    True
lib.rs                              True    True
main.rs                             True    True
adversarial_math_tests.rs           True    True
edge_cases_and_drift_tests.rs       True    True
zero_lookahead_tests.rs             True    True

--- Artefactos nuevos / modificados de Fase 1A ---

File                                         UTF8_NoBOM LF_Only
----                                         ---------- -------
.gitattributes                                     True    True
.gitignore                                         True    True
DOCS/INDEX.md                                      True    True
DOCS/PHASE_1A_TRACK2_FALSIFICATION_REPORT.md       True    True
scripts/fetch_bybit_history.py                     True    True
scripts/audit_track2_datasets.py                   True    True
scripts/run_track2_sweep.ps1                       True    True
```

### 6.7 `git status --short` (Verificación de exclusión de `data_local/`)

```text
 M .gitignore
 M DOCS/INDEX.md
?? .gitattributes
?? DOCS/PHASE_1A_TRACK2_FALSIFICATION_REPORT.md
?? scripts/
```

---

## 7. Veredicto Frío de Fase 1A

- **Resultado de Falsación Global (`M_global = 36`, `VIP-0 + 1.5 bps` slippage = `14.0 bps` RT)**: **`0 / 36` aprobaciones** (`0 Pass`, `29 Fail`, `7 InsufficientSample`).
- **Estabilidad OOS Temporal (Año 1 vs. Año 2)**: Ninguna celda mantuvo significancia estadística o consistencia positiva fuera de muestra; la única celda cercana en `24m` (`4h | VolCompressionBreakout | 6b`, `t_NW = 3.337`, `DSR = 0.7916`, `InsufficientSample`) perdió el `89.5%` de su media del Año 1 (`+61.270 bps`, `t = 3.181`) al Año 2 (`+6.409 bps`, `t = 0.420`).
- **Veredicto Empírico Final**: **Track 2 con las 3 hipótesis base (`VolCompressionBreakout`, `TsmomVolumeWeighted`, `FundingCrowdingReversal`) sobre `15m / 1h / 4h` queda EMPÍRICAMENTE REFUTADO en Bybit V5 (24 meses reales).** No existe ninguna señal candidata con `Verdict_Global36 == Pass` ni estabilidad OOS Año 1/Año 2 que justifique despliegue con `$64 USDT` (ni en universo 12-pares ni Ex-BTC).
- **Implicación Operativa**: En estricto cumplimiento del diseño de `VORTEX-ALFA`, el núcleo de dominio permanece intacto y no se optimizan parámetros *post-hoc* sobre estos datos. El siguiente paso de falsificación debe dirigirse a **Track 1 (`scan-micro`)** sobre captura L2/trades/liquidaciones de alta resolución en Tokio o a la formulación de nuevas hipótesis ortogonales fuera de esta muestra antes de asignar capital real.

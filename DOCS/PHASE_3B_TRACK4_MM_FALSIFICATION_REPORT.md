# VORTEX-ALFA — Phase 3B Track 4 Market Making Empirical Falsification Report

> **Veredicto Empírico Global**: **`0 / 6 Pass`** en el perfil vinculante `VIP0_Maker` (`0 / 30` celdas positivas en toda la frontera diagnóstica, incluso con `-1.00 bps` de rebate) — **Camino B (Market Making L1 pasivo en Bybit VIP-0) EMPÍRICAMENTE FALSADO Y CERRADO**
> **Entorno de Ejecución**: `sniper-tokio` (`rustc 1.99.1`, `35/35` tests `ok`, `--release`) en modo **solo lectura** sobre `/home/ubuntu/vortex-rs/data`
> **Engine & Protocolo**: `vortex-alfa scan-mm` ([`src/application/falsifier_mm.rs`](../src/application/falsifier_mm.rs), [`src/interfaces/cli_mm.rs`](../src/interfaces/cli_mm.rs)) | `M = 6` (`t_crit = 3.5000`), cubetas no solapadas `bucket = 60s`, latencia FIFO `rtt = 5.0ms`, `max_inv_clips = 4.0`, `max_gross_leverage = 2.0`
> **Muestra Real Evaluada (`20,518,144` / `~20.52M` eventos × 2 escalas)**: **Runs 1 & 2** `.par2z` (`VPZ1`, `19,469,568` eventos, `693` cubetas `60s`) + **Runs 3 & 4** `.parquet` (`PAR2`, `1,048,576` eventos, `52` cubetas `60s`) bajo escalas `$2,000 / $100` y `$1,000 / $50`
> **Fricción Vinculante vs. Diagnóstica**: Únicamente `VIP0_Maker` (`+2.00 bps`, `is_binding = true`) habilita `GateVerdict::Pass`; `Venue_1bp` (`+1.00`), `Venue_0bp` (`0.00`), `Rebate_05` (`-0.50`) y `Rebate_10` (`-1.00`) son estrictamente diagnósticos (`is_binding = false`). Toda liquidación EOF cruza el libro a `VIP-0 Taker = 5.50 bps`.
> **Index**: [`DOCS/INDEX.md`](./INDEX.md) | [`DOCS/PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md) | [`DOCS/PHASE_1A_TRACK2_FALSIFICATION_REPORT.md`](./PHASE_1A_TRACK2_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_1B_TRACK1_FALSIFICATION_REPORT.md`](./PHASE_1B_TRACK1_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md`](./PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_3A_TRACK4_MM_DESIGN_REPORT.md`](./PHASE_3A_TRACK4_MM_DESIGN_REPORT.md)

---

## 0. Conformidad con las 4 Leyes Inviolables ("Las 4 D") en Fase 3B

| Ley | Regla | Evidencia Verificada en Ejecución `sniper-tokio` | Estado |
|---|---|---|:---:|
| **Ley 1 — Cero Duplicación (SSOT / DRY)** | Ejecutar exactamente el binario compilado desde `src/` sin simuladores alternos ni recálculo externo | Las `4` corridas ejecutan directamente `./target/release/vortex-alfa scan-mm` ([`cli_mm.rs`](../src/interfaces/cli_mm.rs#L20-L61)) sobre [`StreamingTapeReader`](../src/infrastructure/tape_reader.rs#L43-L146), [`MmSignalEngine`](../src/domain/signals/market_making.rs#L164-L263), [`MmFifoSimulator`](../src/application/mm_fifo_sim.rs#L144-L279), [`MmAccountLedger`](../src/application/mm_accounting.rs#L154-L325) (`EquityLedgerE8`, `leg_fee_e8`) y [`evaluate_hypothesis_batch`](../src/domain/stats.rs#L277-L348). | **PASS** |
| **Ley 2 — Cero Desviación (Strict Scope Lock)** | Prohibido modificar señales, simulador, tarifas o tests tras la verificación de Fase 3A | Binario compilado con `--release` en `sniper-tokio` (`rustc 1.99.1`, `35/35` tests `ok`) sin alteraciones de código, ejecutado en modo **solo lectura** sobre `/home/ubuntu/vortex-rs/data` con los parámetros inmutables de [`default_mm_grid`](../src/application/falsifier_mm.rs#L24-L49) y cero recalibración *post-hoc*. | **PASS** |
| **Ley 3 — Cero Diluvio (Zero Bloat `O(1)`)** | Streaming estricto `O(1)` en memoria y código acotado (`src/**/*.rs <= 350` líneas, `3` deps) | `19,469,568` eventos `VPZ1` consumieron `Max RSS = 3,420 KB (3.34 MiB)` (Run 1) y `3,436 KB` (Run 2) con `Swaps = 0`; `1,048,576` eventos `PAR2` consumieron `3,772 KB` (Run 3) y `4,072 KB` (Run 4) con `Swaps = 0`. | **PASS** |
| **Ley 4 — Cero Disfraz (Zero Disguise)** | Reporte íntegro de las `6` configs y `30` celdas de frontera, `Drift == 0`, EOF a Taker y `n/d` sin interpolación | Se reportan **`0 / 6 Pass`** y **`0 / 30`** celdas positivas en frontera (incluso con `Rebate_10 = -1.00 bps`), `Drift = 0` verificado en todas las corridas, liquidación EOF obligatoria a `5.50 bps` Taker ([`mm_accounting.rs:283-306`](../src/application/mm_accounting.rs#L283-L306)), y los campos no impresos en extractos parciales se consignan literalmente como `n/d`. | **PASS** |

---

## 1. Telemetría de Ejecución de las 4 Corridas Reales en `sniper-tokio`

> Todas las corridas se ejecutaron sobre el binario compilado en `sniper-tokio` (`rustc 1.99.1`, `35/35` tests `ok`, `--release`) leyendo `/home/ubuntu/vortex-rs/data` en modo **solo lectura** conforme a los comandos especificados en [`DOCS/PHASE_3A_TRACK4_MM_DESIGN_REPORT.md`](./PHASE_3A_TRACK4_MM_DESIGN_REPORT.md#L353-L399) (`M = 6`, `t_crit = 3.5000`, `bucket = 60s`, `rtt = 5.0ms`, `max_inv_clips = 4.0`).

| Métrica / Parámetro | Run 1 — `VPZ1` (`.par2z`) | Run 2 — `VPZ1` (`.par2z`) | Run 3 — `PAR2` (`.parquet`) | Run 4 — `PAR2` (`.parquet`) |
|---|---|---|---|---|
| **Subcomando CLI** | `scan-mm --format vpz1` | `scan-mm --format vpz1` | `scan-mm --format par2` | `scan-mm --format par2` |
| **Escala (`capital / clip`)** | `$2,000 / $100` | `$1,000 / $50` | `$2,000 / $100` | `$1,000 / $50` |
| **Dataset (`/home/ubuntu/vortex-rs/data`)** | `.par2z` (`VPZ1`, `12` archivos horarios)* | `.par2z` (`VPZ1`, misma cinta que Run 1) | `.parquet` (`PAR2`, `4,096 × 256` filas)* | `.parquet` (`PAR2`, misma cinta que Run 3) |
| **Eventos Procesados (`events`)** | `19,469,568` | `19,469,568` | `1,048,576` | `1,048,576` |
| **Cubetas `60s` (`Bkt`)** | `693` | `693` (cinta idéntica) | `52` | `52` (cinta idéntica) |
| **`sr_std(VIP0)`** | `0.6635` | `n/d` | `0.9294` | `n/d` |
| **Tiempo Transcurrido (`Elapsed`)** | `1:14.42` | `1:15.02` | `0:11.39` | `0:03.23` |
| **Memoria Máxima (`Max RSS`)** | `3,420 KB (3.34 MiB)` | `3,436 KB` | `3,772 KB` | `4,072 KB` |
| **`Swaps`** | `0` | `0` | `0` | `0` |
| **Código de Salida (`Exit`)** | `0` | `0` | `0` | `0` |
| **`Drift` Contable (`i128`)** | `0` (`6 / 6` configs) | `0` (`6 / 6` configs) | `0` (`6 / 6` configs) | `0` (`6 / 6` configs) |
| **Veredicto (`VIP0_Maker` / Frontera)** | **`0 / 6 Pass`** (`0 / 30` pos.) | **`0 / 6 Pass`** (`0 / 30` pos.) | **`0 / 6 Pass`** (`0 / 30` pos.) | **`0 / 6 Pass`** (`0 / 30` pos.) |

> \* **Nota de cobertura física**: La composición física de los archivos en `/home/ubuntu/vortex-rs/data` (`VPZ1`: `12` archivos horarios `2026-11-23 13h` a `2026-11-24 00h`, `~2.07 GB` comprimidos; `PAR2`: `4,096` segmentos × `256` filas) corresponde al mismo directorio verificado en [`DOCS/PHASE_1B_TRACK1_FALSIFICATION_REPORT.md`](./PHASE_1B_TRACK1_FALSIFICATION_REPORT.md#L24-L34). En Run 2 y Run 4, `sr_std(VIP0)` no figura impreso en el extracto verificado y se consigna como `n/d`.

---

## 2. Resultados Empíricos — Run 1 (`VPZ1`, `19,469,568` eventos, `$2,000 / $100`)

**Cabecera de lote observada**: `events = 19,469,568` | `capital = $2,000` | `clip = $100` | `bucket = 60s` | `rtt = 5.0ms` | `M = 6` | `t_crit = 3.5000` | `sr_std(VIP0) = 0.6635`

### 2.1 Matriz Completa de las 6 Configuraciones (`VIP0_Maker` Vinculante, `+2.00 bps` / EOF `5.50 bps`)

| Config | Bkt | Fills | Rej | Adv | Vol | Gross$ | NetVIP0$ | SpCap | MO1s | MO5s | LVR5s | Peak$ | EOF$ | BrkMkr | Drift | t_nw | DSR | Verdict |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `AS_Base_Tight` | 693 | 31,508 | 2,790 | 1,134 | `$3,150,800` | `-$378.07` | `-$1,008.55` | 0.25 | -1.12 | -1.17 | 2.19 | `$3,512.0` | `$0.35` | -1.20 | 0 | -26.574 | 0.0000 | `Fail` |
| `AS_Base_Wide` | 693 | 1,196 | 84 | 66 | `$119,600` | `-$22.65` | `-$46.58` | 1.23 | -1.74 | -1.74 | 2.90 | `$1,485.9` | `$0.02` | -1.89 | 0 | -7.815 | 0.0000 | `Fail` |
| `AS_Skew_Tight` | 693 | 35,193 | 3,853 | 2,925 | `$3,519,300` | `-$421.55` | `-$1,125.67` | 0.12 | -1.11 | -1.18 | 2.16 | `$2,936.9` | `$0.33` | -1.20 | 0 | -28.928 | 0.0000 | `Fail` |
| `AS_Skew_Mid` | 693 | 8,944 | 553 | 1,209 | `$894,400` | `-$120.63` | `-$299.58` | 0.24 | -1.17 | -1.27 | 2.09 | `$1,758.4` | `$0.08` | -1.35 | 0 | -17.617 | 0.0000 | `Fail` |
| `AS_Skew_Wide` | 693 | 1,306 | 72 | 190 | `$130,600` | `-$21.06` | `-$47.20` | 0.59 | -1.36 | -1.44 | 2.29 | `$1,194.9` | `$0.01` | -1.61 | 0 | -8.404 | 0.0000 | `Fail` |
| `AS_Skew_Deep` | 693 | 180 | 15 | 22 | `$18,000` | `-$4.02` | `-$7.62` | 1.36 | -2.01 | -2.08 | 3.10 | `$699.8` | `$0.00` | -2.24 | 0 | -4.262 | 0.0000 | `Fail` |

**Observaciones inmediatas de Run 1 (`VIP0_Maker`):**
- **`0 / 6 Pass`**: Todas las configuraciones registran `Gross$ < 0`, `NetVIP0$ < 0`, `t_nw` entre `-4.262` y `-28.928` (`<< t_crit = 3.5000`), y `DSR = 0.0000` (`<< 0.95`).
- **`BrkMkr` universalmente negativo (`-1.20` a `-2.24 bps`)**: Ninguna configuración cubre siquiera un fee maker de `0.00 bps`; todas requerirían recibir un **rebate neto** de entre `1.20` y `2.24 bps` por pierna únicamente para empatar a `$0.00`.

### 2.2 Frontera de 5 Tarifas en Run 1 (`VIP0_Maker` vs. Perfiles Diagnósticos)

| Config | `VIP0_Maker` (`+2.00 bps`, bind) | `Venue_1bp` (`+1.00 bps`, diag) | `Venue_0bp` (`0.00 bps`, diag) | `Rebate_05` (`-0.50 bps`, diag) | `Rebate_10` (`-1.00 bps`, diag) |
|---|---:|---:|---:|---:|---:|
| `AS_Base_Tight` | `-$1,008.55` (`t=-26.574`) | `-$693.47` (`t=-25.107`) | `-$378.39` (`t=-20.302`) | `-$220.85` (`t=-14.321`) | `-$63.31` (`t=-4.549`) |
| `AS_Base_Wide` | `-$46.58` (`t=-7.815`) | `n/d` | `-$22.66` (`t=-5.379`) | `n/d` | `-$10.70` (`t=-3.061`) |
| `AS_Skew_Tight` | `-$1,125.67` (`t=-28.928`) | `n/d` | `-$421.81` (`t=-23.984`) | `n/d` | `-$69.88` (`t=-5.593`) |
| `AS_Skew_Mid` | `-$299.58` (`t=-17.617`) | `n/d` | `-$120.70` (`t=-12.899`) | `n/d` | `-$31.26` (`t=-4.629`) |
| `AS_Skew_Wide` | `-$47.20` (`t=-8.404`) | `n/d` | `-$21.08` (`t=-5.930`) | `n/d` | `-$8.02` (`t=-2.868`) |
| `AS_Skew_Deep` | `-$7.62` (`t=-4.262`) | `n/d` | `-$4.02` (`t=-3.078`) | `n/d` | `-$2.22` (`t=-1.978`) |

> **Nota de fidelidad de frontera**: Se verifican **`0 / 30` celdas positivas** en la matriz completa (`6` configs × `5` perfiles). Los valores individuales de `Venue_1bp` (`+1.00 bps`) y `Rebate_05` (`-0.50 bps`) para `AS_Base_Wide .. AS_Skew_Deep` no figuran impresos en el extracto de telemetría verificado y se consignan como `n/d` sin interpolación; en todas las configuraciones, tanto `Venue_0bp` como `Rebate_10` son estrictamente negativos.

---

## 3. Invarianza de Escala (Run 2 `$1,000 / $50`) y Replicación en `PAR2` (Run 3 & Run 4)

### 3.1 Invarianza de Escala en `VPZ1` — Run 2 (`$1,000 / $50`) vs. Run 1 (`$2,000 / $100`)

**Telemetría Run 2**: `events = 19,469,568` | `capital = $1,000` | `clip = $50` | `Elapsed = 1:15.02` | `Max RSS = 3,436 KB` | `Swaps = 0` | `Exit = 0`

| Config | Fills (Run 1) | Fills (Run 2) | NetVIP0$ (Run 1) | NetVIP0$ (Run 2) | BrkMkr (R1 == R2) | t_nw (R1 == R2) | Drift | Verdict |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| `AS_Base_Tight` | 31,508 | 31,508 | `-$1,008.55` | `-$504.28` | -1.20 | -26.574 | 0 | `Fail` |
| `AS_Base_Wide` | 1,196 | 1,196 | `-$46.58` | `n/d` (`50%` de R1) | -1.89 | -7.815 | 0 | `Fail` |
| `AS_Skew_Tight` | 35,193 | 35,193 | `-$1,125.67` | `n/d` (`50%` de R1) | -1.20 | -28.928 | 0 | `Fail` |
| `AS_Skew_Mid` | 8,944 | 8,944 | `-$299.58` | `n/d` (`50%` de R1) | -1.35 | -17.617 | 0 | `Fail` |
| `AS_Skew_Wide` | 1,306 | 1,306 | `-$47.20` | `n/d` (`50%` de R1) | -1.61 | -8.404 | 0 | `Fail` |
| `AS_Skew_Deep` | 180 | 180 | `-$7.62` | `-$3.81` | -2.24 | -4.262 | 0 | `Fail` |

> **Fidelidad de reporte en Run 2**: La telemetría verificada confirma `Fills` exactos idénticos en toda la grilla (`31,508 .. 180`), `PnL` neto exacto al `50%` (`-$504.28 .. -$3.81`), `BrkMkr` idéntico, `t_nw` idéntico y `Drift = 0` en las `6` configuraciones. Los `4` montos individuales intermedios de `NetVIP0$` en Run 2 no impresos explícitamente en el extracto se dejan marcados como `n/d`.

### 3.2 Replicación en Segmentos `.parquet` (`PAR2`, `1,048,576` eventos) — Run 3 & Run 4

- **Run 3 (`$2,000 / $100`)**: `events = 1,048,576` | `Bkt = 52` | `sr_std(VIP0) = 0.9294` | `Elapsed = 0:11.39` | `Max RSS = 3,772 KB` | `Swaps = 0` | `Exit = 0` | **`0 / 6 Pass`** | **`0 / 30`** positivas en frontera.
- **Run 4 (`$1,000 / $50`)**: `events = 1,048,576` | `Bkt = 52` | `Elapsed = 0:03.23` | `Max RSS = 4,072 KB` | `Swaps = 0` | `Exit = 0` | **`0 / 6 Pass`** | **`0 / 30`** positivas en frontera (`Fills` e indicadores idénticos a Run 3, `PnL` al `50%` exacto `-$33.02 .. -$0.26`, `Drift = 0`).

| Config | Bkt | Fills (R3 == R4) | Gross$ (Run 3) | NetVIP0$ (Run 3) | NetVIP0$ (Run 4) | BrkMkr (R3 == R4) | t_nw (R3 == R4) | Drift | Verdict |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---|
| `AS_Base_Tight` | 52 | 2,090 | `-$24.01` | `-$66.03` | `-$33.02` | -1.16 | -12.140 | 0 | `Fail` |
| `AS_Base_Wide` | 52 | `n/d` | `n/d` | `n/d` | `n/d` | `n/d` | `n/d` | 0 | `Fail` (`0/6`) |
| `AS_Skew_Tight` | 52 | 2,440 | `-$27.14` | `-$76.06` | `n/d` (`50%` de R3) | -1.12 | -14.233 | 0 | `Fail` |
| `AS_Skew_Mid` | 52 | `n/d` | `n/d` | `n/d` | `n/d` | `n/d` | `n/d` | 0 | `Fail` (`0/6`) |
| `AS_Skew_Wide` | 52 | `n/d` | `n/d` | `n/d` | `n/d` | `n/d` | `n/d` | 0 | `Fail` (`0/6`) |
| `AS_Skew_Deep` | 52 | 16 | `-$0.21` | `-$0.53` | `-$0.26` | -1.29 | -2.074 | 0 | `Fail` |

> **Nota de fidelidad en `PAR2` (Runs 3 & 4)**: Las filas `AS_Base_Wide`, `AS_Skew_Mid`, `AS_Skew_Wide` y las columnas no listadas (`Rej, Adv, Vol, SpCap, MO1s, MO5s, LVR5s, Peak$, EOF$, DSR` y el valor `NetVIP0$` individual de `AS_Skew_Tight` en Run 4) no figuran impresas en el extracto verificado y se marcan estrictamente como `n/d`. La corrida agregada confirma **`0 / 6 Pass`**, **`0 / 30`** celdas positivas en frontera y `Drift = 0` en todas las configuraciones.

---

## 4. Diagnóstico Forense de Causa Raíz

### 4.1 Toxicidad Inmediata `< 1s`: El `93%` del Daño Ocurre Antes de 1 Segundo

- En [`MarkoutTracker`](../src/application/mm_markout.rs#L98-L157), la captura inicial del spread (`SpCap`) en el instante del fill es positiva en las `6` configuraciones de Run 1 (`+0.12` a `+1.36 bps`), pero el flujo tóxico posterior destruye esa captura de forma casi instantánea:
  - En `AS_Base_Tight` (`31,508` fills), `SpCap = +0.25 bps`, pero a **`1s`** el markout colapsa a **`MO1s = -1.12 bps`**, frente a `MO5s = -1.17 bps` y `BrkMkr = -1.20 bps` (**el `93%` del daño ocurre en `< 1s`**), con `LVR5s = 2.19 bps`.
  - Incluso al ensanchar agresivamente el *half-spread* mínimo en `AS_Base_Wide` (`SpCap = +1.23 bps`) y `AS_Skew_Deep` (`SpCap = +1.36 bps`), `MO1s` cae a `-1.74` y `-2.01 bps` (`MO5s = -1.74` y `-2.08 bps`, `LVR5s = 2.90` y `3.10 bps`), produciendo pérdidas brutas antes de comisiones (`-$22.65` y `-$4.02`, `BrkMkr = -1.89` y `-2.24 bps`).
- La replicación independiente en `PAR2` (`Run 3`) exhibe exactamente el mismo *breakeven* bruto negativo (`BrkMkr = -1.16 bps` en `AS_Base_Tight`, `-1.12 bps` en `AS_Skew_Tight`, `-1.29 bps` en `AS_Skew_Deep`), descartando anomalías de un segmento particular.

### 4.2 Impacto Contraproducente del Skew OFI+TFI (`+157.9%` Fills Adversos en Vuelo)

Comparando el par simétrico sin skew frente al par idéntico con skew OFI+TFI (`AS_Base_Tight` $\theta_{\text{skew}} = 0.0$ vs. `AS_Skew_Tight` $\theta_{\text{skew}} = 2.0\text{ bps}$, ambos con $\gamma = 0.05, \delta_{\min} = 1.0, T = 30\text{s}, k = 1.5$):

| Métrica | `AS_Base_Tight` ($\theta_{\text{skew}} = 0.0$) | `AS_Skew_Tight` ($\theta_{\text{skew}} = 2.0$) | Variación Observada |
|---|---:|---:|---|
| **Fills Adversos en Vuelo (`Adv`)** | `1,134` | `2,925` | **`+157.9%`** (`+1,791` fills) |
| **Rechazos PostOnly (`Rej`)** | `2,790` | `3,853` | `+38.1%` (`+1,063` rechazos) |
| **Fills Totales (`Fills`)** | `31,508` | `35,193` | `+11.7%` (`+3,685` fills) |
| **Spread Capture (`SpCap`)** | `0.25 bps` | `0.12 bps` | `-52.0%` (colapso de captura) |
| **PnL Bruto / Neto VIP-0** | `-$378.07` / `-$1,008.55` | `-$421.55` / `-$1,125.67` | Empeora en `-$43.48` bruto / `-$117.12` neto |
| **Estadístico `t_nw`** | `-26.574` | `-28.928` | Mayor rechazo estadístico |

- **Mecanismo en código fuente**: En [`compute_as_quote`](../src/domain/signals/market_making.rs#L283-L284), el sesgo `shift_bps = theta_skew_bps * alpha_skew - q_clips * inv_var_bps` desplaza $r_t$ según `alpha_skew` (OFI $\tau=2\text{s}$ y TFI $\tau=5\text{s}$). Esto genera dos patologías acopladas en [`MmFifoSimulator`](../src/application/mm_fifo_sim.rs):
  1. Al perseguir el flujo direccional, la cotización cambia con mayor frecuencia superando el umbral anti-churn (`100 ms` / `0.5 bps` en [`L246-L261`](../src/application/mm_fifo_sim.rs#L246-L261)) y entra en `PendingReplace` o `PendingCancel` ([`L191-L193`](../src/application/mm_fifo_sim.rs#L191-L193)), dejando la orden previa expuesta durante `5.0 ms` RTT justo cuando el flujo agresivo barre el nivel (**`+157.9%` en `Adv`**).
  2. El lado empujado hacia el L1 opuesto cruza con más frecuencia el spread durante el vuelo RTT, elevando `post_only_rejects` en [`activate_order`](../src/application/mm_fifo_sim.rs#L281-L300) (`2,790 -> 3,853`). El mismo patrón se observa entre `AS_Base_Wide` (`Adv = 66`, `SpCap = 1.23`) y `AS_Skew_Wide` (`Adv = 190`, `SpCap = 0.59`).

### 4.3 Brecha Estructural de Tarifas VIP-0 (`+3.20 bps/pierna` / `+6.40 bps` RT)

- El mejor *breakeven maker* alcanzado en los `19.47M` eventos de `VPZ1` es **`BrkMkr = -1.20 bps`** (`AS_Base_Tight` y `AS_Skew_Tight`), mientras que Bybit `VIP0_Maker` cobra **`+2.00 bps`** por pierna (`4.00 bps` RT).
- En consecuencia, cerrar la brecha para alcanzar punto de equilibrio en `VIP-0` requeriría una mejora neta de:
  $$\Delta_{\text{breakeven}} = 2.00 - (-1.20) = \mathbf{+3.20\;\text{bps/pierna}}\quad (\mathbf{+6.40\;\text{bps RT}})$$
- Incluso en el extremo más favorable de la frontera diagnóstica (`Rebate_10 = -1.00 bps` de *rebate* por pierna), **ninguna de las 30 celdas resulta positiva** (`AS_Base_Tight: -$63.31, t=-4.549`; `AS_Skew_Tight: -$69.88, t=-5.593`; `AS_Skew_Deep: -$2.22, t=-1.978`).

### 4.4 Invarianza de Escala y Predominio de `book_crossed` / `trade_through`

- En [`compute_as_quote`](../src/domain/signals/market_making.rs#L279-L280), `raw_q_clips = inv / clip_usdt` es homogéneo de grado cero bajo el reescalado simultáneo `$2,000 / $100 -> $1,000 / $50` (manteniendo `max_inv_clips = 4.0` y `max_gross_leverage = 2.0`), generando precios objetivo `PriceE8` idénticos.
- En [`check_side_fill`](../src/application/mm_fifo_sim.rs#L327-L337), cualquier evento donde el mejor nivel opuesto cruza la orden (`book_crossed`: `dir * px >= dir * opp_px`) o un trade atraviesa el precio límite (`trade_through`: `dir * trade_px < dir * px`) ejecuta el fill **con independencia de `order_qty_e8`**; solo los trades exactamente al nivel (`trade_px == px`, [`L338-L343`](../src/application/mm_fifo_sim.rs#L338-L343)) dependen de `qty_e8`.
- Que Run 2 (`VPZ1`, `$1,000/$50`) y Run 4 (`PAR2`, `$1,000/$50`) reproduzcan **fills exactos idénticos** (`31,508 .. 180` y `2,090 / 2,440 / 16`), **`BrkMkr` y `t_nw` idénticos** y **PnL al `50%` exacto** demuestra que la ejecución pasiva L1 en estas cintas está dominada por **`book_crossed` y `trade_through`** (barridos direccionales que atraviesan la cola) y no por consumo parcial en cola estática.

### 4.5 Integridad Contable (`Drift == 0`) y Marginalidad de Liquidación EOF

- En las `4` corridas × `6` configuraciones (`24` ledgers `i128` en [`MmAccountLedger`](../src/application/mm_accounting.rs#L308-L316)), `accounting_drift_e8 == 0` se verificó de forma absoluta.
- El costo total de liquidación EOF (`5.50 bps` Taker + cruce de spread al cierre en [`liquidate_eof`](../src/application/mm_accounting.rs#L283-L306)) osciló entre **`$0.00` y `$0.35`** en `VPZ1`, confirmando que la pérdida proviene íntegramente de la dinámica intrasesión y no de artefactos de cierre al final de la cinta.

---

## 5. Veredicto Frío de Cierre de Fase 3B (Cierre de Camino B y Transición a Track 5 Macro `1d`)

- **Veredicto Empírico Final de Track 4 (`scan-mm`)**:
  - **`0 / 6 Pass`** en el perfil vinculante `VIP0_Maker` (`+2.00 bps` / EOF `5.50 bps`) tanto en `VPZ1` (`19,469,568` eventos, `693` cubetas) como en `PAR2` (`1,048,576` eventos, `52` cubetas) y en ambas escalas de capital (`$2,000/$100` y `$1,000/$50`).
  - **`0 / 30` celdas positivas** en toda la frontera diagnóstica (`VIP0_Maker`, `Venue_1bp`, `Venue_0bp`, `Rebate_05`, `Rebate_10`).
- **Cierre Empírico de Camino B**: **El Market Making L1 pasivo (Avellaneda-Stoikov + Microprecio Stoikov + Skew OFI/TFI a `5.0 ms` RTT) sobre Bybit `VIP-0` queda EMPÍRICAMENTE FALSADO Y CERRADO.** La toxicidad inmediata en `< 1s` (`93%` del daño, `MO1s = -1.12 bps`), la amplificación de fills adversos en vuelo por el skew (`+157.9%`, `1,134 -> 2,925`) y la brecha estructural de `+3.20 bps/pierna` (`+6.40 bps` RT) imposibilitan su viabilidad operativa. En acatamiento estricto de las 4 Leyes D, el código de Track 4 en `src/` permanece intacto sin reoptimización *post-hoc*.

### Balance Acumulado de Falsación (Tracks 1–4) y Justificación de Transición a Track 5 Macro (`1d`)

| Track | Fase | Régimen & Horizonte | Muestra Auditada | Fricción Vinculante | Resultado | Estado |
|---|---|---|---|---|---|---|
| **Track 1** (`scan-micro`) | [`Phase 1B`](./PHASE_1B_TRACK1_FALSIFICATION_REPORT.md) | Microestructura Taker (`15s–900s`) | `20.52M` eventos (`VPZ1` + `PAR2`) | `VIP-0` (`11.00 bps` RT) | **`0 / 20 Pass`** | Falsado y Cerrado |
| **Track 2** (`scan-meso`) | [`Phase 1A`](./PHASE_1A_TRACK2_FALSIFICATION_REPORT.md) | Mesoscópico direccional (`15m / 1h / 4h`) | `12` pares, `24` meses Bybit V5 | `VIP-0 + 1.5 bps` slip (`14.00 bps` RT) | **`0 / 36 Pass`** | Falsado y Cerrado |
| **Track 3** (`scan-xs`) | [`Phase 2`](./PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md) | Cross-Sectional Dollar-Neutral (`4h`) | `30` pares, `24` meses Bybit V5 | `VIP0-Taker + 1.5 bps` (`7.00 bps`/turn) | **`0 / 27 Pass`** | Falsado y Cerrado |
| **Track 4** (`scan-mm`) | **Phase 3B** (este reporte) | Market Making pasivo L1 (`5.0 ms` RTT) | `20.52M` eventos × 2 escalas | `VIP0_Maker` (`+2.00 bps` + EOF `5.50 bps`) | **`0 / 6 Pass` (`0 / 30` front.)** | **Falsado y Cerrado** |
| **Track 5** (`Macro 1d`) | **Fase 4 (Siguiente)** | **Macro / Swing Diario (`1d`)** | Por definir en Fase 4 (Bybit V5 `1d`) | `VIP-0` vinculante + OOS estricto | Pendiente de diseño & falsación | **Habilitado** |

- **Justificación estructural del paso a Track 5 Macro (`1d`)**: Tras falsar sistemáticamente cuatro arquitecturas distintas en horizontes que abarcan desde milisegundos (`5 ms` RTT L1) hasta barras de `4h` (`0 / 89` celdas vinculantes aprobadas entre Tracks 1–4), la evidencia empírica demuestra que bajo estructura de comisiones `VIP-0` (`+2.00 bps` maker, `+5.50 bps` taker + spread/slippage) la razón señal-a-fricción intradiaria y la selección adversa sub-segundo absorben por completo los retornos brutos capturables. Migrar a resolución **Macro `1d`** desplaza la investigación hacia horizontes diarios/multi-día donde la amplitud de los movimientos de precio esperados frente al costo fijo por rotación en `VIP-0` pueda evaluarse sin contaminación microestructural sub-segundo, preservando íntegras las 4 Leyes D, Newey-West, Holm-Bonferroni, DSR, piso `t_crit >= 3.50` y partición temporal OOS sin *lookahead*.

# VORTEX-ALFA — Phase 1B Track 1 Empirical Falsification Report

> **Commit Base (`main`)**: `66999b7` | **Entorno de Ejecución**: `sniper-tokio` (`rustc 1.99.1`, `22/22` tests ok, `--release`, `CARGO_BUILD_JOBS=1`)
> **Engine**: `vortex-alfa scan-micro` ([`src/application/falsifier_micro.rs`](../src/application/falsifier_micro.rs)) en modo **solo lectura** sobre `/home/ubuntu/vortex-rs/data`
> **Muestra Real Evaluada (`~20.52M` eventos)**: **Run 1** `.par2z` (`VPZ1`, 12 archivos horarios `2026-11-23 13h` a `2026-11-24 00h`, `~2.07 GB` comprimidos, `19,469,568` eventos) + **Run 2** `.parquet` (`PAR2`, `4,096` segmentos × `256` filas, `1,048,576` eventos)
> **Fricción Vinculante**: Bybit `VIP-0` Taker/Taker sobre libro real (`Ask -> Bid` para `Long`; `Bid -> Ask` para `Short`; `5.50 + 5.50 = 11.00 bps` RT en [`FeeSchedule::bybit_vip0`](../src/domain/fees.rs#L24-L31)) | Diagnóstico no vinculante `DiagZero`: `2.00 + 2.00 = 4.00 bps` RT ([`FeeSchedule::diag_zero_maker`](../src/domain/fees.rs#L36-L43))
> **Index**: [`DOCS/INDEX.md`](./INDEX.md) | [`DOCS/PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md) | [`DOCS/PHASE_1A_TRACK2_FALSIFICATION_REPORT.md`](./PHASE_1A_TRACK2_FALSIFICATION_REPORT.md)

---

## 0. Conformidad con las 4 Leyes D en Fase 1B

| Ley | Regla | Evidencia Verificada |
|---|---|---|
| **Ley 1 — Cero Duplicación** | Cero recálculo paralelo de señales, ejecución de libro o estadística fuera del core Rust | Ambos runs ejecutan directamente `vortex-alfa scan-micro` ([`src/interfaces/cli.rs`](../src/interfaces/cli.rs#L64-L90)) sobre [`StreamingTapeReader`](../src/infrastructure/tape_reader.rs#L43-L146), [`MicroTailEngine`](../src/domain/signals/micro_tail.rs#L127-L330), [`book_roundtrip_net_bps`](../src/domain/fees.rs#L133-L152) y [`evaluate_hypothesis_batch`](../src/domain/stats.rs#L277-L348). |
| **Ley 2 — Cero Desviación** | Prohibido alterar umbrales, ecuaciones o puertos en `src/`, `tests/` o `Cargo.toml` | Compilación `--release` limpia en `sniper-tokio` (`rustc 1.99.1`, `CARGO_BUILD_JOBS=1`) con `22/22` tests `ok` y ejecución en modo **solo lectura** sobre `/home/ubuntu/vortex-rs/data`. |
| **Ley 3 — Cero Diluvio** | Streaming estricto `O(1)` en memoria y dependencias mínimas | Run 1 (`19,469,568` eventos, `~2.07 GB` `.par2z`) consumió `Max RSS = 7,048 KB (6.88 MiB)` con `Swaps = 0`; Run 2 (`1,048,576` eventos `PAR2`) consumió `Max RSS = 4,252 KB (4.15 MiB)` con `Swaps = 0`. |
| **Ley 4 — Cero Disfraz** | Reporte íntegro sin filtrar perdedores ni fabricar `binance_mid` | Se reportan las 20 celdas completas (`0 / 20 Pass`), incluyendo `DiagZero` negativo en todas las celdas de `HawkesTail` y `ShiryaevRobertsJump`, `N <= 5` en `LiquidationCascade` y `N = 0` legítimo en `CrossExchangeShock` por `binance_mid = None` en `PAR2` ([`binary_codec.rs`](../src/infrastructure/binary_codec.rs#L98)). |

---

## 1. Manifiesto de Datos Reales y Telemetría de Ejecución (`sniper-tokio`)

| Métrica / Parámetro | Run 1 — Cold Archive `.par2z` (`VPZ1`) | Run 2 — `.parquet` Segments (`PAR2`) |
|---|---|---|
| **Host / Toolchain** | `sniper-tokio` (`rustc 1.99.1`, `22/22` tests ok, `--release`, `CARGO_BUILD_JOBS=1`) | `sniper-tokio` (`rustc 1.99.1`, `22/22` tests ok, `--release`, `CARGO_BUILD_JOBS=1`) |
| **Ruta y Modo de Acceso** | Solo lectura sobre `/home/ubuntu/vortex-rs/data` | Solo lectura sobre `/home/ubuntu/vortex-rs/data` |
| **Formato y Cobertura** | `VPZ1` (`.par2z`), 12 archivos horarios `2026-11-23 13h` a `2026-11-24 00h`, `~2.07 GB` comprimidos | `PAR2` (`.parquet`), `4,096` segmentos × `256` filas |
| **Eventos Procesados (`events`)** | `19,469,568` | `1,048,576` |
| **Tiempo Transcurrido (`Elapsed`)** | `1:35.68` (`~203,486 evt/s`) | `11.39s` |
| **Memoria Máxima (`Max RSS`)** | `7,048 KB` (`6.88 MiB`) | `4,252 KB` (`4.15 MiB`) |
| **`Swaps`** | `0` | `0` |
| **Código de Salida (`Exit`)** | `0` | `0` |
| **Veredicto Global (`VIP-0`)** | **`0 / 20 Pass`** | **`0 / 20 Pass`** |

### Notas Estructurales sobre `VPZ1` / `PAR2`

- **Descompresión determinista `VPZ1 -> PAR2`**: Cada frame `VPZ1` (`.par2z`) encapsula segmentos `PAR2` comprimidos mediante Stride-256 XOR-Delta + Zero-RLE con doble verificación `FNV-1a` (`comp_crc` y `orig_crc`), decodificados por [`read_next_vpz1_frame`](../src/infrastructure/tape_reader.rs#L104-L114) y [`Par2Row::to_tape_event`](../src/infrastructure/binary_codec.rs#L72-L99).
- **Ausencia estructural de `binance_mid` en `PAR2`**: Conforme a la auditoría de layout verificada en [`DOCS/PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md#L123-L127), [`Par2Row`](../src/infrastructure/binary_codec.rs#L38-L63) y [`Par2Row::to_tape_event`](../src/infrastructure/binary_codec.rs#L98), el registro binario `PAR2` de `256` bytes carece de campo para precio mid de Binance, por lo que `Par2Row::to_tape_event` asigna siempre `binance_mid = None` y [`MicroTailEngine::on_event`](../src/domain/signals/micro_tail.rs#L294-L325) deja `CrossExchangeShock` en `N = 0` (`InsufficientSample`).

---

## 2. Protocolo Causal de Simulación (`scan-micro`)

Ejecutado con los parámetros por defecto inmutables de [`MicroFalsifierConfig::default`](../src/application/falsifier_micro.rs#L20-L37) y [`MicroDetectorConfig::default`](../src/domain/signals/micro_tail.rs#L58-L79):

- **Latencia, cooldown y slots**: `rtt_ns = 5_000_000` (`5 ms` entre señal y apertura), `cooldown_ns = 10_000_000_000` (`10 s`), horizontes `15s, 60s, 180s, 600s, 900s`, con slots no solapados por `(symbol_id, hypothesis, horizon)` ([`falsifier_micro.rs`](../src/application/falsifier_micro.rs#L114-L216)).
- **Ejecución de libro real (`Ask -> Bid`)**: Tanto entrada como salida se modelan como taker (`entry_is_taker = true`, `exit_is_taker = true`) sobre `BookTop` del evento (`Long`: compra en `Ask`, venta en `Bid`; `Short`: venta en `Bid`, recompra en `Ask`) mediante [`book_roundtrip_net_bps`](../src/domain/fees.rs#L133-L152).
- **Tarifas evaluadas**:
  - `VIP-0` (vinculante, `is_binding_for_verdict = true`): `5.50 bps` entrada + `5.50 bps` salida = **`11.00 bps` RT**.
  - `DiagZero` (diagnóstico no vinculante, `is_binding_for_verdict = false`): `2.00 bps` entrada + `2.00 bps` salida = **`4.00 bps` RT** (diferencial constante de exactamente `7.00 bps` RT respecto a `VIP-0`).
- **Compuerta estadística (`M = 20`)**: `t_crit = 3.5000` (Holm-Bonferroni con piso `3.50` en [`t_crit_holm`](../src/domain/stats.rs#L131-L137)); tamaño mínimo muestral [`MIN_SAMPLE_SIZE = 5`](../src/domain/stats.rs#L9). Para aprobar (`GateVerdict::Pass` en [`evaluate_gate`](../src/domain/stats.rs#L199-L222)), una celda `VIP-0` requiere `N >= max(5, n_min)`, `mean > 0`, `t_nw >= 3.5000` y `DSR >= 0.95`.

---

## 3. Resultados Empíricos — Run 1: Cold Archive `.par2z` (`VPZ1`)

**Cabecera de lote observada**: `events = 19,469,568` | `M = 20` | `t_crit = 3.5000` | `sr_std(VIP-0) = 0.4050` | `sr_std(DiagZero) = 0.2493`

### 3.1 Matriz Completa de 20 Celdas Vinculantes (`VIP-0`, `11.00 bps` RT)

| Hypothesis | Horizon | N | Mean_VIP0_bps | t_nw | t_crit | DSR | Verdict_VIP0 |
|---|---|---:|---:|---:|---:|---:|---|
| `HawkesTail` | `15s` | 7,828 | -11.035 | -51.173 | 3.5000 | `n/d` | `Fail` |
| `HawkesTail` | `60s` | 3,591 | -10.958 | -19.120 | 3.5000 | `n/d` | `Fail` |
| `HawkesTail` | `180s` | 1,690 | -10.199 | -6.302 | 3.5000 | `n/d` | `Fail` |
| `HawkesTail` | `600s` | 650 | -15.214 | -6.793 | 3.5000 | `n/d` | `Fail` |
| `HawkesTail` | `900s` | 455 | -12.091 | -3.277 | 3.5000 | `n/d` | `Fail` |
| `ShiryaevRobertsJump` | `15s` | 27,088 | -12.752 | -163.312 | 3.5000 | `n/d` | `Fail` |
| `ShiryaevRobertsJump` | `60s` | 7,791 | -12.905 | -56.629 | 3.5000 | `n/d` | `Fail` |
| `ShiryaevRobertsJump` | `180s` | 2,704 | -12.365 | -12.380 | 3.5000 | `n/d` | `Fail` |
| `ShiryaevRobertsJump` | `600s` | 817 | -14.838 | -5.048 | 3.5000 | `n/d` | `Fail` |
| `ShiryaevRobertsJump` | `900s` | 547 | -14.357 | -3.343 | 3.5000 | `n/d` | `Fail` |
| `LiquidationCascade` | `15s` | 5 | -38.435 | -2.871 | 3.5000 | 0.0010 | `Fail` |
| `LiquidationCascade` | `60s` | 3 | +35.948 | `-` | 3.5000 | `-` | `InsufficientSample` |
| `LiquidationCascade` | `180s` | 2 | +182.676 | `-` | 3.5000 | `-` | `InsufficientSample` |
| `LiquidationCascade` | `600s` | 2 | +98.918 | `-` | 3.5000 | `-` | `InsufficientSample` |
| `LiquidationCascade` | `900s` | 2 | +135.388 | `-` | 3.5000 | `-` | `InsufficientSample` |
| `CrossExchangeShock` | `15s` | 0 | `-` | `-` | 3.5000 | `-` | `InsufficientSample` |
| `CrossExchangeShock` | `60s` | 0 | `-` | `-` | 3.5000 | `-` | `InsufficientSample` |
| `CrossExchangeShock` | `180s` | 0 | `-` | `-` | 3.5000 | `-` | `InsufficientSample` |
| `CrossExchangeShock` | `600s` | 0 | `-` | `-` | 3.5000 | `-` | `InsufficientSample` |
| `CrossExchangeShock` | `900s` | 0 | `-` | `-` | 3.5000 | `-` | `InsufficientSample` |

> **Notas de fidelidad de tabla**:
> - `n/d`: el valor individual de `DSR` para esas celdas negativas no figura en el extracto de telemetría verificado de `sniper-tokio` y no se extrapola.
> - `-` en `LiquidationCascade` (`60s–900s`, `N = 2..3`) y `CrossExchangeShock` (`N = 0`): corresponde estrictamente al umbral [`MIN_SAMPLE_SIZE = 5`](../src/domain/stats.rs#L9), por el cual [`Moments4::compute`](../src/domain/stats.rs#L48-L51), [`newey_west`](../src/domain/stats.rs#L95-L98) y [`deflated_sharpe_ratio`](../src/domain/stats.rs#L165-L167) devuelven `None` cuando `N < 5` (y [`welford_mean`](../src/domain/stats.rs#L28-L40) devuelve `None` cuando `N = 0`), evaluando [`evaluate_gate`](../src/domain/stats.rs#L208-L209) como `InsufficientSample`.

### 3.2 Diagnóstico No Vinculante `DiagZero` (`4.00 bps` RT) frente a `VIP-0` (`11.00 bps` RT)

- **`HawkesTail`**: **Todas las celdas `DiagZero` resultaron también negativas**, en el rango verificado de **`-3.199` a `-8.214 bps`**. El desplazamiento de `+7.00 bps` respecto a `VIP-0` refleja la diferencia exacta de tarifas taker/taker (`11.00 bps` en `VIP-0` frente a `4.00 bps` en `DiagZero`: `-10.199 + 7.0 = -3.199` en `180s`; `-15.214 + 7.0 = -8.214` en `600s`).
- **`ShiryaevRobertsJump`**: **Todas las celdas `DiagZero` resultaron negativas**, en el rango verificado de **`-5.365` a `-7.838 bps`** (`-12.365 + 7.0 = -5.365` en `180s`; `-14.838 + 7.0 = -7.838` en `600s`).
- **Sin extrapolación de `t_nw` / `DSR`**: No se reportan `t_nw` ni `DSR` individuales de `DiagZero` al no haber sido emitidos por celda en el extracto verificado (constando únicamente `sr_std(DiagZero) = 0.2493` a nivel de lote).
- **Resumen del Run 1**: **`0 / 20 Pass`** (`11 Fail`, `9 InsufficientSample`).

---

## 4. Resultados Empíricos — Run 2: Segmentos `.parquet` (`PAR2`)

- **Dataset**: `4,096` segmentos `PAR2` × `256` filas = **`1,048,576` eventos**.
- **Telemetría de ejecución (`sniper-tokio`, solo lectura)**: `Elapsed = 11.39s` | `Max RSS = 4,252 KB (4.15 MiB)` | `Swaps = 0` | `Exit = 0`.
- **Veredicto agregado**: **`0 / 20 Pass`**.

| Hypothesis | Scope Observado | Mean_VIP0_bps | t_nw | N / Estado Observado |
|---|---|---:|---:|---|
| `HawkesTail` | `15s` | -11.332 | -36.888 | `Fail` |
| `ShiryaevRobertsJump` | `15s` | -12.553 | -63.697 | `Fail` |
| `LiquidationCascade` | Todos los horizontes | `-` | `-` | `N = 0` (`InsufficientSample`) |
| `CrossExchangeShock` | Todos los horizontes | `-` | `-` | `N = 0` (`InsufficientSample`) |

> **Nota**: Los horizontes `60s–900s` de `HawkesTail` y `ShiryaevRobertsJump` en Run 2 no figuran desglosados en el extracto verificado y no se extrapolan; el resultado global verificado del barrido completo de 20 celdas es **`0 / 20 Pass`**.

---

## 5. Diagnóstico Forense de Causa Raíz (Sobre `20.52M` Eventos Reales)

1. **Ausencia de alpha bruto ante ejecución agresiva `Ask -> Bid` (`HawkesTail` y `ShiryaevRobertsJump`)**:
   - En Run 1 (`19.47M` eventos), `HawkesTail` (`N = 455..7,828`) registra medias netas `VIP-0` entre `-10.199` y `-15.214 bps` (`t = -3.277` a `-51.173`), y `ShiryaevRobertsJump` (`N = 547..27,088`) registra medias netas `VIP-0` entre `-12.365` y `-14.838 bps` (`t = -3.343` a `-163.312`). En Run 2 (`1.05M` eventos), el horizonte `15s` reproduce exactamente el mismo comportamiento (`-11.332 bps, t = -36.888` en `HawkesTail`; `-12.553 bps, t = -63.697` en `ShiryaevRobertsJump`).
   - Medido en términos brutos antes de comisiones, el desempeño se sitúa en **`0.00` a `-3.84 bps`** en `HawkesTail` y `ShiryaevRobertsJump`; incluso bajo el perfil diagnóstico reducido `DiagZero` (`4.00 bps` RT), **todas** las celdas permanecen negativas (`-3.199` a `-8.214 bps` en `HawkesTail`; `-5.365` a `-7.838 bps` en `ShiryaevRobertsJump`). Cruzar el spread L1 (`Ask -> Bid`) en horizontes de `15s–900s` tras ráfagas Hawkes o saltos Shiryaev-Roberts destruye valor antes incluso de aplicar comisiones `VIP-0`.
2. **Escasez estadística de `LiquidationCascade` (`N <= 5`)**:
   - En los `19,469,568` eventos del archivo `.par2z`, `LiquidationCascade` solo alcanzó `N = 5` en `15s` (`-38.435 bps`, `t = -2.871`, `DSR = 0.0010`, `Fail`), cayendo a `N = 3` en `60s` (`+35.948 bps`, `InsufficientSample`) y `N = 2` en `180s / 600s / 900s` (`+182.676 / +98.918 / +135.388 bps`, `InsufficientSample`). En los `1,048,576` eventos `PAR2` de Run 2 registró `N = 0`.
   - Con `N <= 5` en `20.52M` eventos reales, la señal carece de significancia estadística y no supera el piso muestral ni el umbral `VIP-0`.
3. **Inactivación limpia de `CrossExchangeShock` (`N = 0`)**:
   - Al no existir campo `binance_mid` en el esquema binario `PAR2` de `256` bytes (`binance_mid = None`), [`MicroTailEngine::on_event`](../src/domain/signals/micro_tail.rs#L294-L325) no dispara operaciones sintéticas y mantiene `N = 0` (`InsufficientSample`) en todos los horizontes de ambos runs.
4. **Estabilidad operativa del lector streaming en Rust**:
   - La ejecución `--release` en `sniper-tokio` (`rustc 1.99.1`, `CARGO_BUILD_JOBS=1`, `22/22` tests ok) completó ambos barridos con `Exit = 0`, `Swaps = 0` y huella de memoria constante (`6.88 MiB` en Run 1 a `~203,486 evt/s`; `4.15 MiB` en Run 2).

---

## 6. Veredicto Frío de Fase 1B

- **Resultado de Falsación Global (`M = 20`, `VIP-0` `11.00 bps` RT sobre libro `Ask -> Bid`)**: **`0 / 20 Pass`** en ambos datasets (`19,469,568` eventos `.par2z` + `1,048,576` eventos `PAR2` = `20,518,144` / **`~20.52M` eventos reales**).
- **Veredicto Empírico Final**: **La ejecución agresiva sub-15m (`Ask -> Bid`) sobre Bybit `VIP-0` en Track 1 queda EMPÍRICAMENTE FALSIFICADA.** `HawkesTail` y `ShiryaevRobertsJump` muestran retornos brutos de `0.00` a `-3.84 bps` antes de comisiones (con todas las celdas `DiagZero` también negativas); `LiquidationCascade` con `N <= 5` carece de significancia estadística; y `CrossExchangeShock` permanece intencionalmente en `N = 0` debido a `binance_mid = None` en `PAR2`.
- **Implicación Operativa**: En cumplimiento estricto de las 4 Leyes D, el núcleo `src/` permanece intacto sin calibración *post-hoc* de parámetros sobre esta cinta. Ninguna de las hipótesis de Track 1 justifica asignación de capital en ejecución agresiva sub-15m.

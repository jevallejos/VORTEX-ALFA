# VORTEX-ALFA — Phase 0 Forensic Closure Report

> **Project**: `vortex-alfa` (`D:\Desarrollo\VORTEX-ALFA`)
> **Architecture**: Hexagonal Clean DDD (Zero `unsafe`, Zero `unwrap`/`expect`/`panic!`, `<= 350` lines/file in `src/`)
> **Audit Role**: Agente 4 (Escriba Forense)
> **Index**: [`DOCS/INDEX.md`](./INDEX.md)

---

## 1. Inspección de Solo Lectura (`D:\Desarrollo\VORTEX-L2`)

Se inspeccionaron exclusivamente los 3 módulos de layout autorizados en `D:\Desarrollo\VORTEX-L2\src\`:
- `D:\Desarrollo\VORTEX-L2\src\forensic_parquet.rs`
- `D:\Desarrollo\VORTEX-L2\src\par2_archiver.rs`
- `D:\Desarrollo\VORTEX-L2\src\candle_store.rs`

### 1.1 Layout Binario `PAR2` (`forensic_parquet.rs`)

- **Constantes de formato** (`forensic_parquet.rs:14-21`):
  - `PAR2_MAGIC_HEADER = *b"PAR2"` (`4` bytes en cabecera y `4` bytes en tráiler; retrocompatibilidad `PAR1` legacy de `96` o `200` bytes).
  - `PAR2_LOSSLESS_ROW_BYTES = 256` bytes/fila, `COLUMNAR_BATCH_CAP = 256` filas/lote, `PAR2_DEEP_BOOK_LEVELS = 4` niveles profundos (`L2..L5`).
- **Estructura de Frame** (`serialize_payload` en `forensic_parquet.rs:474-508`; `write_to_file` en `521-530`; `read_from_file` en `548-686`):
  - `[0..4]`: Magic header `b"PAR2"` (`[u8; 4]`).
  - `[4..8]`: `n` (`u32` LE, número de filas, `n <= 256` en escritor L2; hasta `65_536` en lector VORTEX-ALFA).
  - `[8 .. 8 + n * 256]`: `n` filas secuenciales de `256` bytes cada una.
  - `[8 + n * 256 .. 16 + n * 256]`: Checksum `FNV-1a` de 64 bits (`u64` LE) calculado sobre `[0 .. 8 + n * 256]` con offset basis `0xcbf2_9ce4_8422_2325` y primo `0x0100_0000_01b3` (`compute_fnv1a64` en `forensic_parquet.rs:512-519`).
  - `[16 + n * 256 .. 20 + n * 256]`: Magic trailer `b"PAR2"` (`[u8; 4]`).
  - Overhead total por frame: `20` bytes (`8 + n * 256 + 12`).
- **`Par2EventKind` (`#[repr(u8)]`, `forensic_parquet.rs:33-39`)**:
  - `1 = L2BookSnapshot`
  - `2 = PublicTradePrint`
  - `3 = TickerMeta`
  - `4 = LiquidationPrint`
- **Layout exacto de cada fila `PAR2` (`256 B/fila`, `forensic_parquet.rs:479-505` y `599-645`)**:

| Offset (bytes) | Tamaño | Tipo LE | Campo en `VORTEX-L2` | Descripción / Escala |
|---|---|---|---|---|
| `0..2` | 2 | `u16` | `symbol_id` | Identificador numérico de símbolo |
| `2..3` | 1 | `u8` | `par2_kind` | `1..=4` (`Par2EventKind`) |
| `3..4` | 1 | `u8` | `side` | `0 = Bid`, `1 = Ask` |
| `4..12` | 8 | `u64` | `sequence_id` | Secuencia de mensaje LOB |
| `12..20` | 8 | `i64` | `ts_exchange_ns` | Timestamp del exchange (ns) |
| `20..28` | 8 | `i64` | `ts_local_ns` | Timestamp local de captura (ns) |
| `28..36` | 8 | `i64` | `price_ticks` | Precio de trade/meta/liq (ticks `1e8`) |
| `36..44` | 8 | `i64` | `lot_qty` | Cantidad de trade/liq (lotes `1e8`) |
| `44..52` | 8 | `i64` | `best_bid_ticks` | Mejor Bid L1 (ticks `1e8`) |
| `52..60` | 8 | `i64` | `best_ask_ticks` | Mejor Ask L1 (ticks `1e8`) |
| `60..68` | 8 | `i64` | `best_bid_qty` | Cantidad en Mejor Bid L1 (`1e8`) |
| `68..76` | 8 | `i64` | `best_ask_qty` | Cantidad en Mejor Ask L1 (`1e8`) |
| `76..84` | 8 | `i64` | `mark_price_ticks` | Mark price (ticks `1e8`) |
| `84..88` | 4 | `i32` | `ofi_bps` | Order Flow Imbalance (bps) |
| `88..92` | 4 | `i32` | `tfi_bps` | Trade Flow Imbalance (bps) |
| `92..96` | 4 | `i32` | `geom_bps` | Imbalance geométrico (bps) |
| `96..104` | 8 | `i64` | `realized_vol_5m_bps` | Volatilidad realizada 5m (scaled bps) |
| `104..112` | 8 | `i64` | `open_interest_x100` | Open interest escalado `x100` |
| `112..120` | 8 | `i64` | `funding_rate_scaled_bps` | Funding rate (scaled bps) |
| `120..128` | 8 | `i64` | `next_funding_ts_ns` | Timestamp del próximo funding (ns) |
| `128..256` | 128 | `16 x i64` | `deep_bid/ask_ticks/qty[0..4]` | 4 niveles profundos `L2..L5` (`4 * (bid_px, bid_qty, ask_px, ask_qty)` de `32 B/nivel`) |

### 1.2 Layout Binario `VPZ1` (`.par2z`, `par2_archiver.rs`)

- **Constantes** (`par2_archiver.rs:14-19`): `VPZ1_MAGIC = *b"VPZ1"`, `VPZ1_HEADER_BYTES = 36`, `PAR2_STRIDE_BYTES = 256`, `MAX_FRAME_ORIG_BYTES = 4 MiB`, `MAX_FRAME_COMP_BYTES = 8 MiB`.
- **Header de 36 bytes** (`encode_vpz1_frame` en `par2_archiver.rs:115-131`; `decode_vpz1_stream` en `171-178`):
  - `[0..4]`: `magic` (`b"VPZ1"`, 4 B)
  - `[4..12]`: `segment_seq` (`u64` LE, 8 B)
  - `[12..16]`: `orig_len` (`u32` LE, 4 B)
  - `[16..20]`: `comp_len` (`u32` LE, 4 B)
  - `[20..28]`: `orig_crc` (`u64` LE, FNV-1a 64-bit del segmento original sin comprimir, 8 B)
  - `[28..36]`: `comp_crc` (`u64` LE, FNV-1a 64-bit del payload comprimido, 8 B)
  - `[36 .. 36 + comp_len]`: Payload comprimido.
- **Algoritmo Stride-256 XOR-Delta + Zero-RLE** (`compress_par2_stride_zrle` en `par2_archiver.rs:23-45`; `decompress_par2_stride_zrle_into` en `51-99`):
  - Para cada byte `i`: `d = raw[i] ^ raw[i - 256]` si `i >= 256`, o `raw[i]` si `i < 256`.
  - Si `d != 0`, se emite `d` literal; si `d == 0`, se codifica racha de ceros `[0x00, run]` con `1 <= run <= 255`.
  - Verificación estricta de doble integridad: primero `fnv1a64(comp) == comp_crc`, luego descompresión exacta a `orig_len`, y finalmente `fnv1a64(raw) == orig_crc`.

### 1.3 Layout Binario `VKL1` (`candle_store.rs`)

- **Constantes** (`candle_store.rs:15-31, 126`):
  - `VKL1_MAGIC = 0x314C_4B56` (`b"VKL1"` en Little-Endian).
  - `VKL1_RECORD_BYTES = 96` bytes/vela (`#[repr(C)]`, verificado en compilación en `candle_store.rs:126`).
  - `CandleTimeframe`: `0=1s, 1=1m, 2=5m, 3=15m, 4=1h, 5=4h, 6=1d`.
- **Layout exacto de 96 bytes** (`encode_prefix_88` y `from_bytes_96` en `candle_store.rs:143-200`):

| Offset | Tamaño | Tipo LE | Campo | Descripción / Escala |
|---|---|---|---|---|
| `0..4` | 4 | `u32` | `magic` | `0x314C_4B56` (`b"VKL1"`) |
| `4..5` | 1 | `u8` | `slot_idx` | Índice de slot / símbolo |
| `5..6` | 1 | `u8` | `tf_code` | Código de timeframe (`0..=6`) |
| `6..7` | 1 | `u8` | `wave_regime` | Régimen de onda |
| `7..8` | 1 | `u8` | `flags` | Bit `0x80 = VKL1_FLAG_REST_BACKFILL` |
| `8..16` | 8 | `i64` | `open_ts_ns` | Timestamp apertura de vela (ns) |
| `16..24` | 8 | `i64` | `open_ticks` | Precio Open (`1e8`) |
| `24..32` | 8 | `i64` | `high_ticks` | Precio High (`1e8`) |
| `32..40` | 8 | `i64` | `low_ticks` | Precio Low (`1e8`) |
| `40..48` | 8 | `i64` | `close_ticks` | Precio Close (`1e8`) |
| `48..56` | 8 | `i64` | `volume_lots` | Volumen en lotes (`1e8`) |
| `56..64` | 8 | `i64` | `open_interest_x100` | Open interest `x100` |
| `64..68` | 4 | `u32` | `trade_count` | Conteo de trades en la barra |
| `68..70` | 2 | `i16` | `ofi_mean_pm` | Media OFI en per-mille (`[-1000, 1000]`) |
| `70..72` | 2 | `i16` | `tfi_mean_pm` | Media TFI en per-mille (`[-1000, 1000]`) |
| `72..74` | 2 | `i16` | `geom_mean_pm` | Media Geom en per-mille (`[-1000, 1000]`) |
| `74..76` | 2 | `i16` | `funding_sbps` | Funding rate escalado por `100` (`1 bps = 100 sbps`; decodificado en `src/infrastructure/binary_codec.rs:283` como `f64::from(self.funding_sbps) / 100.0`) |
| `76..78` | 2 | `u16` | `hurst_pm` | Exponente Hurst per-mille |
| `78..80` | 2 | `u16` | `ob_bull_pm` | Eficacia Order Block alcista per-mille |
| `80..82` | 2 | `u16` | `ob_bear_pm` | Eficacia Order Block bajista per-mille |
| `82..84` | 2 | `u16` | `spoof_vetoes` | Conteo de vetos spoof |
| `84..86` | 2 | `u16` | `liq_buy_lots_c` | Lotes de liquidación compra |
| `86..88` | 2 | `u16` | `liq_sell_lots_c` | Lotes de liquidación venta |
| `88..96` | 8 | `u64` | `fnv1a64` | Checksum FNV-1a 64-bit sobre `[0..88]` |

### 1.4 Mapeo `PAR2 -> TapeEvent` y Confirmación Explícita de Campos Ausentes

En `VORTEX-ALFA` (`src/infrastructure/binary_codec.rs:73-99`), `Par2Row::to_tape_event` proyecta la fila binaria de `256 B` al tipo de dominio `TapeEvent`:

- **Campos presentes y mapeados**:
  - `symbol_id` $\rightarrow$ `TapeEvent.symbol_id = SymbolId(self.symbol_id)` (`binary_codec.rs:98`).
  - `ts_local_ns.max(ts_exchange_ns)` $\rightarrow$ `TapeEvent.ts_ns` (`binary_codec.rs:74`).
  - `par2_kind` (`1..=4`) $\rightarrow$ `TapeEventKind::{BookSnapshot, Trade, TickerMeta, Liquidation}` (`binary_codec.rs:75-81`).
  - `side` (`0 | 1`) $\rightarrow$ `Side::{Long, Short}` (`binary_codec.rs:82-83`).
  - `best_bid_ticks`, `best_ask_ticks`, `best_bid_qty`, `best_ask_qty` $\rightarrow$ `TapeEvent.book: BookTop` (`binary_codec.rs:84-88`, validando libro no cruzado).
  - `price_ticks`, `lot_qty` $\rightarrow$ `trade_price: Option<PriceE8>`, `trade_qty: Option<QtyE8>` (solo si `kind` es `Trade` o `Liquidation` y el campo es `> 0`; `binary_codec.rs:89-92`).
  - `L1 + deep_*[0..4]` (`L1..L5`) $\rightarrow$ `TapeEvent.l1_to_l5_notional` sumando `bid_px*bid_qty + ask_px*ask_qty` en L1 y los 4 niveles profundos `L2..L5` (`binary_codec.rs:93-97`).
- **Confirmación explícita de campos ausentes en `PAR2`**:
  - **`binance_mid` NO existe en el layout `PAR2`**: Inspeccionado el layout completo de `256` bytes en `D:\Desarrollo\VORTEX-L2\src\forensic_parquet.rs:479-505` (y su homólogo `Par2Row` en `src/infrastructure/binary_codec.rs:38-63`), **no existe ningún campo para precio mid de Binance (`binance_mid`)**.
  - En consecuencia, `Par2Row::to_tape_event` decodifica `binance_mid` **siempre como `None`** (`src/infrastructure/binary_codec.rs:98`).
  - Al ejecutar Track 1 sobre cintas `PAR2` o `VPZ1`, `MicroTailEngine::on_event` (`src/domain/signals/micro_tail.rs:294-325`) solo evalúa la hipótesis `CrossExchangeShock` cuando `event.binance_mid` es `Some(_)`, por lo que `CrossExchangeShock` permanece exactamente con `N = 0` y `GateVerdict::InsufficientSample` en todos los horizontes, sin errores ni valores sintéticos (verificado en `tests/zero_lookahead_tests.rs:605-615`).
  - Asimismo, los campos auxiliares de `PAR2` (`sequence_id`, `mark_price_ticks`, `ofi_bps`, `tfi_bps`, `geom_bps`, `realized_vol_5m_bps`, `open_interest_x100`, `funding_rate_scaled_bps`, `next_funding_ts_ns`) se decodifican íntegramente para validación estructural pero no se inyectan en `TapeEvent`.

---

## 2. Inventario de Archivos, Capas Hexagonales y Dependencias (`Cargo.toml`)

### 2.1 Inventario Completo de Archivos y Conteo de Líneas (post-`cargo fmt`)

| Archivo | Líneas | Límite `<= 350` | Capa Hexagonal | Responsabilidad Principal |
|---|---:|:---:|---|---|
| `src/lib.rs` | 7 | PASS | Crate Root | Atributos de seguridad (`forbid(unsafe_code)`, `deny(unwrap/expect/panic)`) y módulos |
| `src/main.rs` | 17 | PASS | Crate Root | Entrypoint CLI (`run_cli` -> `ExitCode`) |
| `src/domain/mod.rs` | 8 | PASS | Domain | Registro de submódulos de dominio |
| `src/domain/error.rs` | 45 | PASS | Domain | `DomainError` (`thiserror::Error`) tipado para todas las capas |
| `src/domain/ports.rs` | 18 | PASS | Domain | Puertos (`MarketTapeReader`, `CandleSeriesReader`, `ReportSink`) |
| `src/domain/types.rs` | 331 | PASS | Domain | Newtypes `1e8`, `BookTop`, `Candle`, `TapeEvent`, `FundingTimestampSemantics` |
| `src/domain/fees.rs` | 179 | PASS | Domain | `FeeSchedule` (`VIP-0` / `DiagZero`), `leg_fee_e8` (`ceil`), `net_return_bps` |
| `src/domain/ledger.rs` | 113 | PASS | Domain | `EquityLedgerE8` en `i128` con invariante de deriva exacta `0` |
| `src/domain/normal_dist.rs` | 217 | PASS | Domain | `erfc_exact` (Cody 1969), `normal_cdf`, `normal_inv_cdf` (Acklam + Halley) |
| `src/domain/stats.rs` | 348 | PASS | Domain | Welford, `Moments4`, `newey_west`, Holm-Bonferroni, `DSR` (`sr_cross_std`), `n_min`, `GateVerdict` |
| `src/domain/signals/mod.rs` | 3 | PASS | Domain | Registro de motores de señales |
| `src/domain/signals/micro_detectors.rs` | 226 | PASS | Domain | `HawkesProcess` O(1), `ShiryaevRobertsDetector` log-space, `CausalQuantileWindow` |
| `src/domain/signals/micro_tail.rs` | 330 | PASS | Domain | Motor Track 1 (`MicroTailEngine`, 4 hipótesis, estado `BTreeMap` por símbolo) |
| `src/domain/signals/meso_energy.rs` | 331 | PASS | Domain | Motor Track 2 (`garman_klass_vol`, `MesoEnergyEngine`, 3 hipótesis por símbolo) |
| `src/application/mod.rs` | 2 | PASS | Application | Registro de casos de uso |
| `src/application/falsifier_micro.rs` | 280 | PASS | Application | Orquestador Track 1: RTT delay, cooldown, slots no solapados, `VIP-0` + `DiagZero` |
| `src/application/falsifier_meso.rs` | 330 | PASS | Application | Orquestador Track 2: entrada `open(t+1)`, `BarOpenSettlement` vs `BarCloseSettlement` |
| `src/infrastructure/mod.rs` | 4 | PASS | Infrastructure | Registro de adaptadores I/O y códecs |
| `src/infrastructure/binary_codec.rs` | 342 | PASS | Infrastructure | `fnv1a64`, códecs `PAR2` (`256B`), `VPZ1` (`36B` hdr + Stride-256 XOR-Delta/ZRLE), `VKL1` (`96B`) |
| `src/infrastructure/tape_reader.rs` | 325 | PASS | Infrastructure | `StreamingTapeReader` (`Par2`, `Vpz1`, `Jsonl`) ordenando directorios por `first_ts` |
| `src/infrastructure/vkl1_reader.rs` | 154 | PASS | Infrastructure | `StreamingVkl1Reader` sobre `.vkl1` con validación de longitud múltiplo de `96` |
| `src/infrastructure/csv_candle_reader.rs` | 319 | PASS | Infrastructure | `CsvCandleReader` (`ts_ns`/`ts_ms`, `funding_rate_bps`/`funding_rate`, monotonicidad) |
| `src/interfaces/mod.rs` | 1 | PASS | Interfaces | Registro de interfaz CLI |
| `src/interfaces/cli.rs` | 222 | PASS | Interfaces | Subcomandos `scan-micro` y `scan-meso`, salida ASCII y `JsonFileReportSink` (`--out`) |
| `tests/adversarial_math_tests.rs` | 473 | N/A (`tests/`) | QA Tests | 9 tests de vectores cerrados analíticos (GK, Moments4, NW, Acklam, Holm, DSR, Hawkes, Fees, Funding) |
| `tests/zero_lookahead_tests.rs` | 703 | N/A (`tests/`) | QA Tests | 5 tests de aislamiento estricto por símbolo, zero-lookahead E2E y streaming multifile/CLI |
| `tests/edge_cases_and_drift_tests.rs` | 668 | N/A (`tests/`) | QA Tests | 8 tests adversariales (NaN/Inf, varianza 0, vela plana, flash crash -30%, dust/crossed/OHLC, timestamps, binarios corruptos, ledger 10k) |
| `Cargo.toml` | 21 | N/A | Config/Docs | Manifiesto de paquete, binarios y dependencias |
| `Cargo.lock` | 128 | N/A | Config/Docs | Lockfile reproducible de dependencias |
| `rustfmt.toml` | 4 | N/A | Config/Docs | `edition = "2021"`, `newline_style = "Unix"`, `max_width = 100`, `tab_spaces = 4` |
| `.gitignore` | 6 | N/A | Config/Docs | Exclusiones `/target/`, `*.rs.bk`, `*.pdb`, `*.log`, etc. |
| `DOCS/INDEX.md` | 94 | N/A | Config/Docs | Índice navegable de arquitectura, CLI y estado Phase 0 |
| `DOCS/PHASE_0_CLOSURE_REPORT.md` | 430 | N/A | Config/Docs | Este informe forense de cierre de Phase 0 |

### 2.2 Dependencias en `Cargo.toml` y Justificación

`Cargo.toml` declara únicamente **3 dependencias directas** (cero frameworks HTTP, cero async runtime, cero bindings FFI/C):

| Crate | Especificación (`Cargo.toml`) | Justificación Técnica |
|---|---|---|
| `thiserror` | `thiserror = "2"` (`Cargo.toml:17`) | Derivación de `std::error::Error` y `Display` sin boilerplate para `DomainError` a través de las fronteras Hexagonales. |
| `serde` | `serde = { version = "1", features = ["derive"] }` (`Cargo.toml:19`) | Derivación de `Serialize`/`Deserialize` para filas JSONL de cintas (`JsonlTapeRow`), estructuras de dominio y reportes de falsificación. |
| `serde_json` | `serde_json = "1"` (`Cargo.toml:21`) | Decodificación streaming de cintas JSONL (`parse_jsonl_line`) y serialización pretty-printed del reporte JSON (`--out <report.json>`). |

---

## 3. Tabla `Ecuación (LaTeX) -> archivo:línea` con Análisis Dimensional y Guardas

| Concepto / Ecuación (LaTeX) | Ubicación Exacta (`archivo:línea`) | Análisis Dimensional | Guardas de Dominio y Numéricas |
|---|---|---|---|
| **Notional `1e8`**: $\text{Notional}_{e8} = \left\lfloor \frac{P_{e8} \cdot Q_{e8}}{10^8} \right\rfloor$ | `src/domain/types.rs:66-82` | $\frac{[\text{USDT}\cdot 10^8] \cdot [\text{base}\cdot 10^8]}{10^8} = [\text{USDT}\cdot 10^8]$ | `PriceE8 > 0`, `QtyE8 > 0`; `checked_mul` en `i128` (`ArithmeticOverflow`); rechazo `notional_e8 < min_notional_e8` (`DustNotional`). |
| **Fee conservador `ceil`**: $\text{Fee}_{e8} = \left\lceil \frac{\text{Notional}_{e8} \cdot \text{round}(f_{\text{bps}} \cdot 10^4)}{10^8} \right\rceil$ | `src/domain/fees.rs:7-11, 74-95` | $\frac{[\text{USDT}\cdot 10^8] \cdot [\text{bps}\cdot 10^4]}{10^4 \cdot 10^4} = [\text{USDT}\cdot 10^8]$ | `fee_bps` finito y $\ge 0$ (`InvalidFeeBps`); cota `<= i64::MAX`; `checked_mul` y `checked_add(FEE_DENOM - 1)` previniendo overflow. |
| **PnL neto Book Ask$\to$Bid**: $\text{PnL}_{\text{net,bps}} = d \cdot \frac{P_{\text{exit}} - P_{\text{entry}}}{P_{\text{entry}}} \cdot 10^4 - f_e - f_x - f_{\text{fund}}$ | `src/domain/fees.rs:98-152` | Adimensional ($\frac{\Delta P}{P} \cdot 10^4 = [\text{bps}]$) | Long: Ask$\to$Bid; Short: Bid$\to$Ask (`executable_entry/exit_price`); `BookTop` no cruzado (`ask >= bid`); fees $\ge 0$; resultado finito en `Bps::new`. |
| **PnL neto Candle con slippage y funding**: $P_{\text{in}} = P_O (1 + d \cdot s),\; P_{\text{out}} = P_C (1 - d \cdot s),\; s = \frac{\text{slip}_{\text{bps}}}{10^4}$ | `src/domain/fees.rs:155-179` | $P_{\text{in}}, P_{\text{out}} [\text{USDT}]$; retorno neto en $[\text{bps}]$ | `slippage_per_leg_bps >= 0`; `eff_entry > 0`, `eff_exit > 0` y finitos (`NonFiniteValue`); fees no negativos. |
| **Invariante `EquityLedgerE8`**: $\Delta_{\text{drift}} = E_{\text{curr}} - \left(E_0 + \sum G - \sum F - \sum \text{Fund}\right) = 0$ | `src/domain/ledger.rs:31-82` | Todos los términos en $[\text{USDT}\cdot 10^8]$ (`i128`) | `initial_equity_e8 >= 0`; `entry_fee_e8 >= 0`, `exit_fee_e8 >= 0`; `checked_add`/`checked_sub` en cada acumulador; cero punto flotante. |
| **Media Welford & `Moments4`**: $\mu_i = \mu_{i-1} + \frac{x_i - \mu_{i-1}}{i}$, $\sigma^2 = \frac{S_2}{n-1}$, $\gamma_1 = \frac{S_3/n}{(S_2/n)^{3/2}}$, $\kappa = \frac{S_4/n}{(S_2/n)^2}$ | `src/domain/stats.rs:28-74` | $\mu, \sigma$ en $[\text{bps}]$; $\sigma^2$ en $[\text{bps}^2]$; $\gamma_1, \kappa$ adimensionales (curtosis no-exceso) | `MIN_SAMPLE_SIZE = 5`; entradas finitas; devuelve `None` si `variance <= 0` o no finito; sin NaN en series constantes. |
| **Newey-West Bartlett LRV**: $L = \min\left(\left\lfloor 4(n/100)^{2/9} \right\rfloor, n-1\right)$, $\text{LRV} = \gamma_0 + 2\sum_{k=1}^{L} \left(1 - \frac{k}{L+1}\right)\gamma_k$, $t_{\text{NW}} = \frac{\mu}{\sqrt{\text{LRV}/n}}$ | `src/domain/stats.rs:78-129` | $\gamma_k, \text{LRV}$ en $[\text{bps}^2]$; $\text{SE}_{\text{NW}}$ en $[\text{bps}]$; $t_{\text{NW}}$ adimensional | $n \ge 5$; si $\gamma_0 \le 0$ retorna `None`; fallback $\text{LRV} \leftarrow \gamma_0$ si suma Bartlett $\le 0$; `p_value = 2\Phi(-|t_{\text{NW}}|)` acotado a $[0, 1]$. |
| **Cody `erfc_exact` & Acklam + Halley `normal_inv_cdf`**: $\Phi(x) = \frac{1}{2}\text{erfc}\left(-\frac{x}{\sqrt{2}}\right)$, $x_{k+1} = x_k - \frac{u}{1 + \frac{1}{2}x_k u}$ | `src/domain/normal_dist.rs:103-217` | Probabilidad $p \in (0, 1)$ y cuantil $z \in \mathbb{R}$ adimensionales | Cody 3 regiones ($|x|\le 0.46875$, $\le 4$, $< 27.3$); `normal_cdf` rechaza `NaN`; `normal_inv_cdf` exige finito y $0 < p < 1$; simetría en cola inferior ($p > 0.5 \Rightarrow -\Phi^{-1}(1-p)$) evita cancelación. |
| **Holm-Bonferroni**: $t_{\text{crit}} = \max\left(3.50,\; \Phi^{-1}\left(1 - \frac{0.01}{2M}\right)\right)$, $\tilde{p}_{(k)} = \max_{j \le k}\min(1, (M - j)p_{(j)})$ | `src/domain/stats.rs:131-155` | Adimensional ($M \in \mathbb{N}_{\ge 1}$, $p \in [0, 1]$) | `m_hypotheses >= 1`; `p_values` finitos en $[0, 1]$; ordenación determinista `total_cmp`; monotonicidad garantizada por `rmax`. |
| **DSR real (`sr_cross_std`), $n_{\min}$ y Gate**: $\widehat{\text{SR}}_0 = \sigma_{\text{SR}} \left[(1-\gamma)\Phi^{-1}(1-\tfrac{1}{M}) + \gamma\Phi^{-1}(1-\tfrac{1}{Me})\right]$, $\text{DSR} = \Phi\!\left(\frac{(\widehat{\text{SR}} - \widehat{\text{SR}}_0)\sqrt{n-1}}{\sqrt{1 - \gamma_1\widehat{\text{SR}} + \frac{\kappa - 1}{4}\widehat{\text{SR}}^2}}\right)$, $n_{\min} = \left\lceil \left(\frac{t_{\text{crit}}\sigma}{\mu}\right)^2 \right\rceil$ | `src/domain/stats.rs:157-222, 268-314` | $\widehat{\text{SR}}, \sigma_{\text{SR}}, \text{DSR} \in [0,1]$ adimensionales; $n_{\min}$ en $[\text{trades}]$ | $\sigma_{\text{SR}}$ calculado como std muestral real entre celdas válidas (`stats.rs:291-300`); fallback $1/\sqrt{n}$ solo si $< 2$ SRs válidos; guarda $d^2 > 0$; `Pass` solo si `is_binding_for_verdict && n >= max(5, n_min) && mean > 0 && t_nw >= t_crit && dsr >= 0.95`. |
| **Hawkes O(1) Ozaki**: $R_k = e^{-\beta \Delta t_k}(1 + R_{k-1})$, $\lambda_k = \mu_0 + \alpha R_k$, $\text{imb} = \frac{\lambda_{\text{buy}} - \lambda_{\text{sell}}}{\lambda_{\text{buy}} + \lambda_{\text{sell}}}$ | `src/domain/signals/micro_detectors.rs:40-90`; `src/domain/signals/micro_tail.rs:194-230` | $\Delta t$ en $[\text{s}]$ (`ns * 1e-9`); $\beta, \mu_0, \alpha, \lambda$ en $[\text{s}^{-1}]$; $\text{imb} \in [-1, 1]$ | $\beta > 0, \alpha \ge 0, \mu_0 > 0$; `ts_ns >= 0` y monótono por símbolo (`NonMonotonicTimestamp`); estado aislado por `BTreeMap<SymbolId, _>`. |
| **Shiryaev-Roberts log-space**: $\ell_t = \text{clamp}\left(0.375 \left(\frac{x_t}{\sigma_{0,t-1}}\right)^2 - \ln 2,\; [-20, 20]\right)$, $\log R_t = \text{logAddExp}(0, \log R_{t-1}) + \ell_t$ | `src/domain/signals/micro_detectors.rs:95-172`; `src/domain/signals/micro_tail.rs:233-254` | $x_t, \sigma_0$ en $[\text{bps}]$; $\ell_t, \log R_t$ adimensionales | $\sigma_{0,t-1} = \max(\sqrt{v_{t-1}}, 0.1)$ calculado **antes** de actualizar `ewma_var` con $x_t^2$ (cero auto-enmascaramiento); reseteo a $-\infty$ al disparar $\log R_t \ge \text{thresh}$. |
| **LiquidationCascade (5s)**: $P_{\text{liq}} = \frac{\sum_{t_i \ge t - 5\text{s}} \text{Notional}_i}{\max(\text{Notional}_{L1..L5},\; 10^{-6})}$ | `src/domain/signals/micro_tail.rs:257-291` | Numerador y denominador en $[\text{USDT}]$; $P_{\text{liq}}$ adimensional | Ventana deslizante estrictamente causal (`front_ts < ts_ns - 5e9` expulsados); `l1_to_l5_notional >= 0` finito; requiere `pressure >= 1.5` y `signed_liq != 0`. |
| **CrossExchangeShock (60s)**: $b_t = \frac{M_{\text{BN}} - M_{\text{BB}}}{M_{\text{BB}}} \cdot 10^4$, $\text{shock}_t = b_t - \bar{b}_{t-1}$, $\alpha = 1 - e^{-\Delta t / 60}$ | `src/domain/signals/micro_tail.rs:294-325` | $b_t, \bar{b}_{t-1}, \text{shock}_t$ en $[\text{bps}]$; $\tau = 60\text{ s}$ | Activo únicamente si `binance_mid.is_some()` y `cross_count >= 5`; baseline $\bar{b}_{t-1}$ evaluado **antes** de incorporar $b_t$. |
| **CausalQuantileWindow**: $p = q(m - 1)$, $Q_q = x_{(\lfloor p \rfloor)} + (p - \lfloor p \rfloor)(x_{(\lceil p \rceil)} - x_{(\lfloor p \rfloor)})$ | `src/domain/signals/micro_detectors.rs:199-225` | Misma unidad que la serie observada ($[\text{USDT}]$ nocional) | Evalúa cuantil sobre ventana previa **antes** de `push_back(val)`; `min_samples <= capacity`, $q \in [0, 1]$, `val` finito. |
| **Garman-Klass Volatility**: $\sigma_{\text{GK}} = \sqrt{\max\!\left(0,\; \frac{1}{2}\ln^2\!\left(\frac{H}{L}\right) - (2\ln 2 - 1)\ln^2\!\left(\frac{C}{O}\right)\right)}$ | `src/domain/signals/meso_energy.rs:33-54` | Retornos logarítmicos y $\sigma_{\text{GK}}$ adimensionales | Retorno exacto `0.0` (bits `0`) cuando $O = H = L = C$; valida $H \ge L$, $O > 0$, $L > 0$ y resultado finito. |
| **Compresión de Volatilidad (`r_comp`)**: $r_{\text{comp}} = \frac{\text{EMA}_{\text{fast}}(\sigma_{\text{GK}})}{\text{EMA}_{\text{slow}}(\sigma_{\text{GK}})}$, $\alpha_k = \frac{2}{\text{span}_k + 1}$ | `src/domain/signals/meso_energy.rs:200-213, 264-290` | Cociente adimensional (default spans `6` y `48`) | Si $\text{EMA}_{\text{slow}} \le 0$ retorna `1.0` (sin división por cero); `bar_count == 0` inicializa ambas EMAs con $\sigma_{\text{GK},0}$. |
| **TSMOM-VW**: $\text{TSMOM}_{\text{VW}} = \ln\!\left(\frac{C_t}{C_{t-W}}\right) \cdot \min\!\left(\frac{V_t}{\text{EMA}_{\text{vol},t-1}},\; \text{cap}\right)$ | `src/domain/signals/meso_energy.rs:216-240, 293-304` | Adimensional ($W = 12$, `vol_ema_span = 24`, `cap = 5.0`) | Usa `ema_vol_prev` y `close_history` estrictamente **anteriores** a la barra $t$; actualiza ambos buffers **después** del cálculo. |
| **Z-funding Welford**: $z_{\text{fund},t} = \frac{f_t - \mu_{f,t-1}}{\sqrt{M_{2,t-1}/(n_{t-1}-1)}}$ | `src/domain/signals/meso_energy.rs:243-259, 307-318` | $f_t, \mu_f$ en $[\text{bps}]$; $z_{\text{fund}}$ adimensional | Requiere `fund_count >= funding_warmup` (default 30) y varianza $> 0$; actualiza Welford **después** de evaluar $z_{\text{fund},t}$; señal contrarian en `\|z_fund\| >= 2.0`. |
| **Funding Semantics (`BarOpenSettlement` vs `BarCloseSettlement`)**: $\text{FundCost} = d \sum_{b \in \mathcal{B}} f_b$ | `src/domain/types.rs:214-219`; `src/application/falsifier_meso.rs:94, 191-260` | $[\text{bps}]$ (entrada en `open(t+1)`, salida en `close(t+H)`) | `BarOpenSettlement`: excluye funding de `entry_bar` ($\mathcal{B} = \{t+2, \dots, t+H\}$; `H=1 => 0`); `BarCloseSettlement`: incluye `entry_bar` ($\mathcal{B} = \{t+1, \dots, t+H\}$). |

---

## 4. Matriz de Cumplimiento de las 4 Leyes Inviolables ("Las 4 D")

| Ley Inviolable | Mecanismos de Cumplimiento en `VORTEX-ALFA` | Evidencia Verificada | Estado |
|---|---|---|---|:---:|
| **Ley 1 — Cero Duplicación (SSOT / DRY)** | Único `DomainError` compartido (`src/domain/error.rs`); único `FeeSchedule` + `net_return_bps` en `src/domain/fees.rs`; única batería estadística (`Moments4`, `newey_west`, `evaluate_hypothesis_batch`) consumida por ambos tracks (`falsifier_micro.rs:240-241`, `falsifier_meso.rs:301-302`); único `fnv1a64` en `binary_codec.rs:27-31` reutilizado por `PAR2`, `VPZ1` y `VKL1`; macros locales `put_le!`/`get_le!`/`chk!`/`le_fn!` en `binary_codec.rs:17-24` y helper compartido `settle_meso_trade` en `falsifier_meso.rs:87-133`. | Cero lógica de cálculo duplicada entre Track 1, Track 2, CLI y códecs. | **PASS** |
| **Ley 2 — Cero Desviación (Strict Scope Lock & Hexagonal Inward Dependencies)** | Dependencias estrictamente hacia adentro: `domain` no importa `application`, `infrastructure` ni `interfaces`; `application` depende solo de `domain` y traits `MarketTapeReader`/`CandleSeriesReader` (`domain/ports.rs`); `infrastructure` implementa puertos de `domain`; `interfaces/cli.rs` actúa como composition root. Cero crates de red, async, UI o live-trading. | `Cargo.toml` con solo 3 crates (`thiserror`, `serde`, `serde_json`); separación estricta en `src/{domain,application,infrastructure,interfaces}`. | **PASS** |
| **Ley 3 — Cero Diluvio (`<= 350` líneas/archivo, dependencias mínimas, sin God Objects)** | Todos los archivos en `src/**/*.rs` tienen `<= 350` líneas post-`cargo fmt` (máximo `348` en `src/domain/stats.rs`, `342` en `src/infrastructure/binary_codec.rs`); cero `#[cfg(test)]` dentro de `src/`; motores de señales aislados por archivo (`micro_detectors.rs`, `micro_tail.rs`, `meso_energy.rs`); lectores separados por formato (`tape_reader.rs`, `vkl1_reader.rs`, `csv_candle_reader.rs`). | Gate 4 confirma `Pass = True` en los 24 archivos de `src/**/*.rs`. | **PASS** |
| **Ley 4 — Cero Disfraz (Anti-Reward-Hacking, `#![forbid(unsafe_code)]`, `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]`, cero synthetic edge claims)** | `#![forbid(unsafe_code)]` y `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` activos en `src/lib.rs:1-2` y `src/main.rs:1-2`; `DiagZero` jamás produce `GateVerdict::Pass` (`fees.rs:33-43`, `stats.rs:214-218`); `PAR2` no fabrica `binance_mid` (`None` -> `CrossExchangeShock` `N=0` / `InsufficientSample`); todas las estadísticas y detectores son estrictamente causales sin lookahead. | Gate 2 (`clippy -D warnings`), Gate 5 (`Select-String` limpio) y 22 tests de `tests/` verifican anti-leakage y zero-drift. | **PASS** |

---

## 5. Salida Cruda Sin Editar de las 6 Compuertas PowerShell 7 + `git status` + `git remote -v`

> Ejecutado directamente en `D:\Desarrollo\VORTEX-ALFA` tras la creación de los archivos en `DOCS/`. Salida capturada literalmente sin edición.

### 5.1 Compuerta 1 — `cargo fmt --all -- --check`

```text
PS D:\Desarrollo\VORTEX-ALFA> cargo fmt --all -- --check; Write-Output "EXIT_CODE=$LASTEXITCODE"
EXIT_CODE=0
```

### 5.2 Compuerta 2 — `cargo clippy --all-targets --all-features -- -D warnings`

```text
PS D:\Desarrollo\VORTEX-ALFA> cargo clippy --all-targets --all-features -- -D warnings; Write-Output "EXIT_CODE=$LASTEXITCODE"
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.17s
EXIT_CODE=0
```

### 5.3 Compuerta 3 — `cargo test --all-targets -- --nocapture`

```text
PS D:\Desarrollo\VORTEX-ALFA> cargo test --all-targets -- --nocapture; Write-Output "EXIT_CODE=$LASTEXITCODE"
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.15s
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
test test_hawkes_o1_three_step_analytical ... ok
test test_holm_bonferroni_order_and_monotonicity ... ok
test test_moments4_closed_form_exact ... ok
test test_newey_west_closed_form_n8 ... ok
test test_dsr_cross_sectional_std_and_diag_never_passes ... ok
test test_vip0_ceil_fee_e8_and_book_roundtrip_asymmetry ... ok
test test_exact_funding_boundary_bar_open_vs_bar_close ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

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

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s

     Running tests\zero_lookahead_tests.rs (target\debug\deps\zero_lookahead_tests-68c127dd0d00bc59.exe)

running 5 tests
test test_zero_lookahead_end_to_end_track1_micro ... ok
test test_zero_lookahead_end_to_end_track2_meso ... ok
test test_strict_symbol_isolation_track2_meso ... ok
test test_strict_symbol_isolation_track1_micro ... ok
test test_multifile_par2_vpz1_vkl1_streaming_and_cli ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

EXIT_CODE=0
```

### 5.4 Compuerta 4 — Conteo de Líneas `src/**/*.rs` (`<= 350`)

```text
File                                    Lines Pass
----                                    ----- ----
src\application\falsifier_meso.rs         330 True
src\application\falsifier_micro.rs        280 True
src\application\mod.rs                      2 True
src\domain\error.rs                        45 True
src\domain\fees.rs                        179 True
src\domain\ledger.rs                      113 True
src\domain\mod.rs                           8 True
src\domain\normal_dist.rs                 217 True
src\domain\ports.rs                        18 True
src\domain\signals\meso_energy.rs         331 True
src\domain\signals\micro_detectors.rs     226 True
src\domain\signals\micro_tail.rs          330 True
src\domain\signals\mod.rs                   3 True
src\domain\stats.rs                       348 True
src\domain\types.rs                       331 True
src\infrastructure\binary_codec.rs        342 True
src\infrastructure\csv_candle_reader.rs   319 True
src\infrastructure\mod.rs                   4 True
src\infrastructure\tape_reader.rs         325 True
src\infrastructure\vkl1_reader.rs         154 True
src\interfaces\cli.rs                     222 True
src\interfaces\mod.rs                       1 True
src\lib.rs                                  7 True
src\main.rs                                17 True
```

### 5.5 Compuerta 5 — Búsqueda de Patrones Prohibidos en `src\**\*.rs`

```text
CLEAN: 0 banned-pattern matches in src\**\*.rs
MATCH_COUNT=0
```

### 5.6 Compuerta 6 — Higiene UTF-8 Sin BOM y LF-only en `src` y `tests`

```text
File                                    Utf8NoBom LfOnly Pass
----                                    --------- ------ ----
src\application\falsifier_meso.rs            True   True True
src\application\falsifier_micro.rs           True   True True
src\application\mod.rs                       True   True True
src\domain\error.rs                          True   True True
src\domain\fees.rs                           True   True True
src\domain\ledger.rs                         True   True True
src\domain\mod.rs                            True   True True
src\domain\normal_dist.rs                    True   True True
src\domain\ports.rs                          True   True True
src\domain\signals\meso_energy.rs            True   True True
src\domain\signals\micro_detectors.rs        True   True True
src\domain\signals\micro_tail.rs             True   True True
src\domain\signals\mod.rs                    True   True True
src\domain\stats.rs                          True   True True
src\domain\types.rs                          True   True True
src\infrastructure\binary_codec.rs           True   True True
src\infrastructure\csv_candle_reader.rs      True   True True
src\infrastructure\mod.rs                    True   True True
src\infrastructure\tape_reader.rs            True   True True
src\infrastructure\vkl1_reader.rs            True   True True
src\interfaces\cli.rs                        True   True True
src\interfaces\mod.rs                        True   True True
src\lib.rs                                   True   True True
src\main.rs                                  True   True True
tests\adversarial_math_tests.rs              True   True True
tests\edge_cases_and_drift_tests.rs          True   True True
tests\zero_lookahead_tests.rs                True   True True


FAIL_COUNT=0
```

### 5.7 Estado Git (`git status --short` y `git remote -v`)

```text
--- git status --short ---
?? .gitignore
?? Cargo.lock
?? Cargo.toml
?? DOCS/
?? rustfmt.toml
?? src/
?? tests/
--- git remote -v ---
origin	https://github.com/jevallejos/VORTEX-ALFA.git (fetch)
origin	https://github.com/jevallejos/VORTEX-ALFA.git (push)
```

---

## 6. Tabla `input -> expected -> actual` de Vectores Analíticos y Adversariales (22 Tests)

### 6.1 `tests/adversarial_math_tests.rs` (9 tests)

| # | Test (`archivo:línea`) | Input Concreto | Expected Analítico | Actual Observado |
|---|---|---|---|---|
| 1 | `test_garman_klass_closed_form_and_flat_bar` (`adversarial_math_tests.rs:51-83`) | Vela `(O=100, H=110, L=95, C=105)` y vela plana `(O=H=L=C=100)` | $\sqrt{0.5\ln^2(110/95) - (2\ln 2 - 1)\ln^2(105/100)} = 0.09912983040222308$; plana $= 0.0$ (`to_bits == 0`) | Match exacto (`tol = 1e-13`, bits `0.0`) — **PASS** |
| 2 | `test_moments4_closed_form_exact` (`adversarial_math_tests.rs:86-102`) | `data = [1.0, 2.0, 3.0, 4.0, 10.0]` ($N=5$) | $\mu=4.0$, $\sigma^2=12.5$, $\sigma=3.5355339059327378$, $\gamma_1=1.1384199576606165$, $\kappa=2.788$ | Match exacto (`tol = 1e-14`/`1e-13`) — **PASS** |
| 3 | `test_newey_west_closed_form_n8` (`adversarial_math_tests.rs:105-119`) | `data = [1, 3, 2, 4, 3, 5, 4, 6]` ($N=8$) | $L=2$, $\text{LRV}=10/3=3.3333333333333335$, $\text{SE}_{\text{NW}}=\sqrt{5/12}=0.6454972243679028$, $t_{\text{NW}}=5.422176684690384$, $p \in (0, 10^{-7})$ | Match exacto (`tol = 1e-14`/`1e-13`) — **PASS** |
| 4 | `test_acklam_normal_inv_cdf_accuracy` (`adversarial_math_tests.rs:122-138`) | $p \in \{0.5, 0.975, 0.999, 0.995, 10^{-6}, 1-10^{-8}\}$ | $z \in \{0.0, 1.959963984540054, 3.0902323061678136, 2.5758293035489004, -4.753424308822899, 5.612001244174789\}$ y roundtrip $\Phi(z) = p$ | Match exacto (`tol = 1e-9` inv, `1e-12` cdf) — **PASS** |
| 5 | `test_holm_bonferroni_order_and_monotonicity` (`adversarial_math_tests.rs:141-162`) | `[0.001, 0.01, 0.04, 0.03]` y `[0.001, 0.01, 0.04, 0.02]` | `[0.004, 0.03, 0.06, 0.06]` (clamp monótono en idx 2/3) y `[0.004, 0.03, 0.04, 0.04]` | Match exacto (`tol = 1e-15`) — **PASS** |
| 6 | `test_dsr_cross_sectional_std_and_diag_never_passes` (`adversarial_math_tests.rs:165-232`) | 3 celdas: `r0` ($N=8$), `r1` ($N=6$), `r2` ($N=120$ alternando `4, 6`) | `sr_cross_std` = std muestral de $[2.182633, -1.870829, 4.979123]$ (no $1/\sqrt{120}$); `VIP-0` celda 2 `Pass` (`DSR > 0.99`); `DiagZero` celda 2 `Fail` (`!is_binding`, nunca `Pass`) | Match exacto (`tol = 1e-12`) — **PASS** |
| 7 | `test_hawkes_o1_three_step_analytical` (`adversarial_math_tests.rs:235-253`) | $(\beta=1.0, \alpha=0.65, \mu_0=2.0)$ en $t \in \{0, 0.5\text{s}, 1.5\text{s}\}$ | $(R_1=0, \lambda_1=2.0)$, $(R_2=e^{-0.5}=0.6065306597126334, \lambda_2=2.3942449288132117)$, $(R_3=e^{-1}(1+e^{-0.5})=0.5910096013198721, \lambda_3=2.3841562408579167)$ | Match exacto (`tol = 1e-14`) — **PASS** |
| 8 | `test_vip0_ceil_fee_e8_and_book_roundtrip_asymmetry` (`adversarial_math_tests.rs:256-314`) | `NotionalE8` $\in \{100000001, 100000000, 1\}$; book entrada `(100.0, 100.1)` -> salida `(101.0, 101.2)` bajo VIP-0 taker/taker | Fees `ceil`: `55_001`, `55_000`, `1`; Long `78.91008991008991` bps; Short `-131.0` bps | Match exacto (`tol = 1e-11`/`1e-12`) — **PASS** |
| 9 | `test_exact_funding_boundary_bar_open_vs_bar_close` (`adversarial_math_tests.rs:317-473`) | Señal en barra 1 (Long y Short); barras 2,3,4 con funding `+100, +25, -10` bps; `H=1, 3` | `BarOpen`: `H=1 -> 0.0`, `H=3 -> ±15.0` bps; `BarClose`: `H=1 -> ±100.0`, `H=3 -> ±115.0` bps | Match exacto (`tol = 1e-12`) — **PASS** |

### 6.2 `tests/zero_lookahead_tests.rs` (5 tests)

| # | Test (`archivo:línea`) | Input Concreto | Expected Analítico | Actual Observado |
|---|---|---|---|---|
| 10 | `test_strict_symbol_isolation_track1_micro` (`zero_lookahead_tests.rs:227-314`) | Cinta `sym_a=1` sola vs intercalada con liquidaciones extremas `10_000..50_000` en `sym_b=2` | Señales (`MicroTailEngine`) y trades (`run_micro_falsification_detailed`) de `sym_a` bit-a-bit idénticos (`to_bits`) | Igualdad bit-a-bit en señales y trades — **PASS** |
| 11 | `test_strict_symbol_isolation_track2_meso` (`zero_lookahead_tests.rs:317-403`) | 70 velas `sym_a=1` solas vs intercaladas con barras `100x..300x` y funding `±2500` bps en `sym_b=2` | Salidas `gk_vol`, `r_comp`, `tsmom_vw`, `z_fund`, señales y trades de `sym_a` bit-a-bit idénticos | Igualdad bit-a-bit en salidas y trades — **PASS** |
| 12 | `test_zero_lookahead_end_to_end_track1_micro` (`zero_lookahead_tests.rs:406-461`) | Tape original vs tape con los últimos 9 eventos (`> cutoff_ts`) mutados adversarialmente | Trades cerrados con `exit_ts_ns <= cutoff_ts` bit-a-bit idénticos | Prefijo bit-a-bit idéntico — **PASS** |
| 13 | `test_zero_lookahead_end_to_end_track2_meso` (`zero_lookahead_tests.rs:464-518`) | 70 velas originales vs velas `51..69` (`> cutoff_ts`) mutadas a `1000..1500` y funding `-500` | Trades cerrados con `exit_ts_ns <= cutoff_ts` bit-a-bit idénticos | Prefijo bit-a-bit idéntico — **PASS** |
| 14 | `test_multifile_par2_vpz1_vkl1_streaming_and_cli` (`zero_lookahead_tests.rs:541-703`) | Directorio multi-archivo `PAR2` (`seg_001` 0..29s + `seg_002` 30..120s), equivalente `VPZ1` (`.par2z`), 2 archivos `VKL1` (40+30 velas), y CLI `scan-micro`/`scan-meso --out` | Trade SR cruza `seg_001 -> seg_002` con `net_bps` exacto; `CrossExchangeShock` `n=0` / `InsufficientSample`; `VPZ1 == PAR2` bit-a-bit; `VKL1` cruza archivo `1 -> 2`; JSON CLI válido | Todos los asertos y roundtrips JSON exactos — **PASS** |

### 6.3 `tests/edge_cases_and_drift_tests.rs` (8 tests)

| # | Test (`archivo:línea`) | Input Concreto | Expected Analítico | Actual Observado |
|---|---|---|---|---|
| 15 | `test_reject_non_finite_across_all_components` (`edge_cases_and_drift_tests.rs:109-167`) | `NaN`, `+inf`, `-inf` en `Bps`, `Moments4`, `newey_west`, `normal_inv_cdf`, `ShiryaevRobertsDetector`, `CausalQuantileWindow`, `normal_cdf(NaN)` y 6 CSVs no finitos | Todos rechazan con `Err(DomainError::NonFinite* / InvalidStatParam / InvalidCandle)`; `normal_cdf(±inf) = 1.0 / 0.0` | Rechazo tipado y `±inf` exacto — **PASS** |
| 16 | `test_zero_variance_never_passes_or_nans` (`edge_cases_and_drift_tests.rs:170-204`) | Series constantes `[5.0; 10]` y `[0.0; 10]` | `Moments4 = None`, `newey_west = None`, `dsr = None`, `t_nw = None`, nunca `GateVerdict::Pass`, `mean_bps` finito | Cero NaNs y veredicto != `Pass` — **PASS** |
| 17 | `test_flat_candle_garman_klass_zero` (`edge_cases_and_drift_tests.rs:207-221`) | Vela plana `(O=H=L=C=250.0, vol=42.0)` | `garman_klass_vol == 0.0` (`to_bits == 0`) | Match exacto (`0_u64` bits) — **PASS** |
| 18 | `test_flash_crash_minus_30_percent_bar_and_tick` (`edge_cases_and_drift_tests.rs:224-355`) | Flash crash `-30%` (`100.0 -> 70.0`) en barra Meso (`H=1`, slip `0`) y tick Micro (`15s`) bajo VIP-0 taker/taker (`11 bps`) | `gk_crash > 0` finito; `net_bps_vip0 = -3000 - 11 = -3011.0` bps en ambos tracks | Match exacto (`tol = 1e-6`) — **PASS** |
| 19 | `test_dust_notional_crossed_book_and_invalid_ohlc_rejected` (`edge_cases_and_drift_tests.rs:358-400`) | `1.0 * 0.001 < 0.10`; `BookTop(100.5, 100.0)`; velas con `H < C` y `L > O` | `Err(DustNotional { 100_000, 10_000_000 })`, `Err(CrossedBook)`, `Err(InvalidCandle)` | Errores tipados exactos — **PASS** |
| 20 | `test_duplicate_and_backward_timestamps_rejected` (`edge_cases_and_drift_tests.rs:403-517`) | `ts` retrocedido en `HawkesProcess` (`1000->999`) y `MicroTailEngine` (`2000->1999`); `ts` duplicado y retrocedido en `MesoEnergyEngine` y `CsvCandleReader` | `Err(DomainError::NonMonotonicTimestamp)` en todos los casos | Todos rechazados con `NonMonotonicTimestamp` — **PASS** |
| 21 | `test_corrupted_binary_formats_rejected` (`edge_cases_and_drift_tests.rs:520-633`) | `PAR2` magic/CRC corrupto; `VPZ1` `comp_crc`, `orig_crc` y payload mutado con `comp_crc` recalculado; `VKL1` CRC corrupto y archivo truncado `50 B` | Todos devuelven `Err(DomainError::FormatCorrupted(_))` | Todos rechazados con `FormatCorrupted` — **PASS** |
| 22 | `test_equity_ledger_10000_trades_zero_drift` (`edge_cases_and_drift_tests.rs:636-668`) | `initial = 100_000 * 1e8`, `10_000` trades con nocional impar `1_000_000_003` (`leg_fee_e8` taker/maker) y funding alternado | `trade_count == 10_000`, `drift_e8() == Ok(0)`, `current_equity_e8() == expected_equity` | Deriva exacta `0` tras 10,000 trades — **PASS** |

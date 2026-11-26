# VORTEX-ALFA — Phase 3A Track 4 Market Making Design & Verification Report

> **Estado de Cierre**: Diseño e Implementación Fase 3A Verificados (`35/35` tests de integración `ok`, `32/32` archivos `src/**/*.rs <= 350` líneas, `0` warnings en Clippy, `0` patrones prohibidos, listo para barrido empírico en `sniper-tokio`)
> **Engine**: `vortex-alfa scan-mm` ([`src/application/falsifier_mm.rs`](../src/application/falsifier_mm.rs), [`src/interfaces/cli_mm.rs`](../src/interfaces/cli_mm.rs))
> **Motor de Señal**: [`src/domain/signals/market_making.rs`](../src/domain/signals/market_making.rs) (Microprecio Stoikov + OFI CKS L1 + TFI + Volatilidad EWMA + Avellaneda-Stoikov homogéneo en `[bps]` + `tick_e8` GCD causal + PostOnly)
> **Simulador & Contabilidad**: [`src/application/mm_fifo_sim.rs`](../src/application/mm_fifo_sim.rs) (`5 ms` RTT FIFO L1 + anti-churn) | [`src/application/mm_markout.rs`](../src/application/mm_markout.rs) (`SpreadCapture`, Markouts `1s/5s/15s`, `LVR`) | [`src/application/mm_accounting.rs`](../src/application/mm_accounting.rs) (Ledger `i128` `Drift == 0`, EOF Taker `5.50 bps`, frontera de 5 tarifas)
> **Fricción Vinculante vs. Diagnóstica**: Únicamente `VIP0_Maker` (`+2.00 bps`, `is_binding = true`) habilita `GateVerdict::Pass`; `Venue_1bp` (`+1.00`), `Venue_0bp` (`0.00`), `Rebate_05` (`-0.50`) y `Rebate_10` (`-1.00`) son estrictamente diagnósticos (`is_binding = false`). Toda liquidación EOF paga `VIP-0 Taker = 5.50 bps`.
> **Index**: [`DOCS/INDEX.md`](./INDEX.md) | [`DOCS/PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md) | [`DOCS/PHASE_1A_TRACK2_FALSIFICATION_REPORT.md`](./PHASE_1A_TRACK2_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_1B_TRACK1_FALSIFICATION_REPORT.md`](./PHASE_1B_TRACK1_FALSIFICATION_REPORT.md) | [`DOCS/PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md`](./PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md)

---

## 0. Conformidad con las 4 Leyes Inviolables ("Las 4 D") en Fase 3A

| Ley | Regla | Evidencia Verificada en Código Fuente | Estado |
|---|---|---|---|:---:|
| **Ley 1 — Cero Duplicación (SSOT / DRY)** | Reutilizar el núcleo `domain` y adaptadores `infrastructure`/`interfaces` sin duplicar aritmética de fees, ledger, estadística o lectura de cintas | [`src/application/mm_accounting.rs`](../src/application/mm_accounting.rs) reutiliza [`EquityLedgerE8`](../src/domain/ledger.rs#L5-L113) ([L172](../src/application/mm_accounting.rs#L172), [L258-L259](../src/application/mm_accounting.rs#L258-L259)), [`leg_fee_e8`](../src/domain/fees.rs#L74-L95) (`FEE_BPS_SCALE`, `FEE_DENOM`, [L245](../src/application/mm_accounting.rs#L245), [L330](../src/application/mm_accounting.rs#L330)) y tipos `1e8` de [`src/domain/types.rs`](../src/domain/types.rs). [`src/application/falsifier_mm.rs`](../src/application/falsifier_mm.rs) reutiliza [`FeeSchedule::bybit_vip0`](../src/domain/fees.rs#L24-L31) ([L93](../src/application/falsifier_mm.rs#L93)), [`evaluate_hypothesis_batch`](../src/domain/stats.rs#L277-L348) ([L225](../src/application/falsifier_mm.rs#L225)) y [`MarketTapeReader`](../src/domain/ports.rs#L6-L8). [`src/interfaces/cli_mm.rs`](../src/interfaces/cli_mm.rs) reutiliza [`StreamingTapeReader`](../src/infrastructure/tape_reader.rs#L43-L146) ([L55](../src/interfaces/cli_mm.rs#L55)) y los helpers `pub(crate)` de [`cli.rs`](../src/interfaces/cli.rs#L8) (`fmt_opt`, `parse_tape_format`, `take_arg`, `write_json_pretty`). | **PASS** |
| **Ley 2 — Cero Desviación (Strict Scope Lock)** | Prohibido modificar Track 1, Track 2, Track 3, `stats.rs`, `fees.rs`, `ledger.rs`, `types.rs`, `Cargo.toml` o los `29` tests previos | `0` modificaciones en Track 1 (`micro_detectors.rs`, `micro_tail.rs`, `falsifier_micro.rs`), Track 2 (`meso_energy.rs`, `falsifier_meso.rs`), Track 3 (`cross_sectional.rs`, `falsifier_xs.rs`), [`stats.rs`](../src/domain/stats.rs), [`fees.rs`](../src/domain/fees.rs), [`ledger.rs`](../src/domain/ledger.rs), [`types.rs`](../src/domain/types.rs), [`Cargo.toml`](../Cargo.toml) (`3` deps intactas) y los `29` tests preexistentes (`9 + 7 + 8 + 5`). Track 4 es estrictamente aditivo (`7` archivos nuevos) con solo `4` registros mínimos de módulo/subcomando en `src/{application/mod.rs,domain/signals/mod.rs,interfaces/mod.rs,interfaces/cli.rs}` (`35/35` tests `ok`). | **PASS** |
| **Ley 3 — Cero Diluvio (Zero Bloat `<= 350` líneas)** | Todos los archivos en `src/**/*.rs <= 350` líneas post-`cargo fmt`, sin God Objects ni dependencias nuevas | Reparto SRP limpio de Track 4: [`market_making.rs`](../src/domain/signals/market_making.rs) (`310`), [`mm_markout.rs`](../src/application/mm_markout.rs) (`158`), [`mm_fifo_sim.rs`](../src/application/mm_fifo_sim.rs) (`346`), [`mm_accounting.rs`](../src/application/mm_accounting.rs) (`345`), [`falsifier_mm.rs`](../src/application/falsifier_mm.rs) (`337`), [`cli_mm.rs`](../src/interfaces/cli_mm.rs) (`136`), [`cli.rs`](../src/interfaces/cli.rs) (`327`). Los `32/32` archivos en `src/**/*.rs` pasan `<= 350` líneas (máximo global `348` en `stats.rs`). | **PASS** |
| **Ley 4 — Cero Disfraz (Zero Disguise)** | Cero *phantom fills*, latencia RTT real, liquidación EOF a taker, único perfil vinculante y `Drift == 0` | Conservación de cola FIFO L1 ([`mm_fifo_sim.rs:302-346`](../src/application/mm_fifo_sim.rs#L302-L346)); RTT `5 ms` con fills adversos en vuelo ([`L191-L212`](../src/application/mm_fifo_sim.rs#L191-L212)); liquidación obligatoria EOF cruzando `Bid`/`Ask` a `5.50 bps` VIP-0 Taker ([`mm_accounting.rs:283-306`](../src/application/mm_accounting.rs#L283-L306), [`falsifier_mm.rs:176`](../src/application/falsifier_mm.rs#L176)); `is_binding = true` **exclusivamente** en `VIP0_Maker` ([`mm_accounting.rs:36-42`](../src/application/mm_accounting.rs#L36-L42)); contabilidad exacta `i128` verificando `accounting_drift_e8 == 0` ([`L308-L316`](../src/application/mm_accounting.rs#L308-L316)). | **PASS** |

---

## 1. Arquitectura Hexagonal y Reparto SRP de Track 4

La integración de Track 4 preserva intacta la regla de dependencias hacia adentro (`Interfaces -> Application -> Domain <- Infrastructure`), evitando monolitos mediante la partición en cuatro unidades SRP entre dominio, aplicación e interfaces:

### 1.1 Inventario de Archivos Track 4 (Conteo Post-`cargo fmt`)

| Archivo | Líneas | Límite `<= 350` | Capa | Responsabilidad Única (SRP) |
|---|---:|:---:|---|---|
| [`src/domain/signals/market_making.rs`](../src/domain/signals/market_making.rs) | 310 | PASS | `Domain` | `stoikov_microprice`, `MmSignalEngine` causal por `SymbolId` (OFI CKS L1, TFI, EWMA $\sigma$, `tick_e8` GCD) y `compute_as_quote` homogéneo en `[bps]` con guardas PostOnly y de autocruce |
| [`src/domain/signals/mod.rs`](../src/domain/signals/mod.rs) | 5 | PASS | `Domain` | Exporta `pub mod market_making;` junto a los motores de Tracks 1–3 |
| [`src/application/mm_markout.rs`](../src/application/mm_markout.rs) | 158 | PASS | `Application` | `MarkoutTracker`, `MmFill`, `MmSimMetrics`: registro causal de `spread_capture_bps`, markouts a `{1s, 5s, 15s}` y `LVR = max(0, -Markout)` |
| [`src/application/mm_fifo_sim.rs`](../src/application/mm_fifo_sim.rs) | 346 | PASS | `Application` | `MmFifoSimulator`: máquina de estados FIFO L1 (`Idle`, `PendingPlace`, `Resting`, `PendingReplace`, `PendingCancel`), latencia RTT `5 ms`, `PostOnlyReject` y anti-churn (`100 ms` / `0.5 bps`) |
| [`src/application/mm_accounting.rs`](../src/application/mm_accounting.rs) | 345 | PASS | `Application` | `MmAccountLedger` en `i128` (`Drift == 0`), `MM_FEE_FRONTIER` (5 perfiles), `signed_leg_fee_e8`, `flush_bucket`, `liquidate_eof`, `breakeven_maker_bps` y DTOs `MmFalsificationReport` |
| [`src/application/falsifier_mm.rs`](../src/application/falsifier_mm.rs) | 337 | PASS | `Application` | Orquestador causal sobre `R: MarketTapeReader`: grid `M = 6` (`default_mm_grid`), cubetas no solapadas de `60s`, tope global `max_gross_leverage` y evaluación Holm/NW/DSR vía `evaluate_hypothesis_batch` |
| [`src/application/mod.rs`](../src/application/mod.rs) | 7 | PASS | `Application` | Exporta `falsifier_mm`, `mm_accounting`, `mm_fifo_sim`, `mm_markout` |
| [`src/interfaces/cli_mm.rs`](../src/interfaces/cli_mm.rs) | 136 | PASS | `Interfaces` | `run_scan_mm` (`--format`, `--input`, `--capital-usdt`, `--clip-usdt`, `--max-inv-clips`, `--out`) y `format_mm_report_ascii` |
| [`src/interfaces/cli.rs`](../src/interfaces/cli.rs) | 327 | PASS | `Interfaces` | Enrutamiento de `"scan-mm"` ([L53](../src/interfaces/cli.rs#L53)), `usage_text` ([L63](../src/interfaces/cli.rs#L63)) y helpers compartidos `pub(crate)` |
| [`src/interfaces/mod.rs`](../src/interfaces/mod.rs) | 2 | PASS | `Interfaces` | Exporta `pub mod cli_mm;` |
| [`tests/market_making_tests.rs`](../tests/market_making_tests.rs) | 1,394 | N/A (`tests/`) | QA Tests | Suite de `6` tests analíticos, FIFO/RTT, contables `i128`, frontera no vinculante, zero-lookahead/aislamiento y CLI `PAR2` |

### 1.2 Grafo de Dependencias de Track 4

```text
interfaces::cli (cli.rs:53)
  └── interfaces::cli_mm (run_scan_mm)
        ├── infrastructure::tape_reader::StreamingTapeReader (implements MarketTapeReader)
        └── application::falsifier_mm (run_mm_falsification)
              ├── application::mm_fifo_sim (MmFifoSimulator) ──> application::mm_markout (MarkoutTracker)
              ├── application::mm_accounting (MmAccountLedger, MM_FEE_FRONTIER)
              │     ├── domain::ledger::EquityLedgerE8
              │     └── domain::fees::{leg_fee_e8, FEE_BPS_SCALE, FEE_DENOM}
              └── domain::signals::market_making (MmSignalEngine, compute_as_quote)
                    └── domain::{types, error}   [Cero dependencias salientes]
```

---

## 2. Trazabilidad `Ecuación (LaTeX) -> archivo:línea` + Análisis Dimensional en `[bps]`

| Concepto / Ecuación (LaTeX) | Ubicación Exacta (`archivo:línea`) | Análisis Dimensional | Guardas de Dominio y Numéricas |
|---|---|---|---|
| **Microprecio Stoikov L1**: $S_{\text{micro}} = \frac{P_{\text{ask}} Q_{\text{bid}} + P_{\text{bid}} Q_{\text{ask}}}{Q_{\text{bid}} + Q_{\text{ask}}}$ | [`src/domain/signals/market_making.rs:140-148`](../src/domain/signals/market_making.rs#L140-L148) (invocado en [L186-L188](../src/domain/signals/market_making.rs#L186-L188)) | $\frac{[\text{USDT}]\cdot[\text{base}]}{[\text{base}]} = [\text{USDT}]$ | Si $Q_{\text{bid}} + Q_{\text{ask}} \le 0$ o no finito, degrada limpiamente a `book.mid_f64()`; `on_event` valida `s_mid, s_micro > 0` y finitos (`mm_book_px`). |
| **OFI Causal CKS L1 ($\tau_{\text{OFI}} = 2\,\text{s}$)**: $e_b = \begin{cases} q_b & P_b > P_b' \\ q_b - q_b' & P_b = P_b' \\ -q_b' & P_b < P_b' \end{cases},\quad e_a = \begin{cases} -q_a & P_a < P_a' \\ -(q_a - q_a') & P_a = P_a' \\ q_a' & P_a > P_a' \end{cases}$<br>$k_{\text{OFI}} = e^{-\Delta t / \tau_{\text{OFI}}},\quad \text{OFI}_{\text{norm}} = \text{clamp}\!\left(\frac{N_{\text{OFI},t}}{D_{\text{OFI},t}}, -1, 1\right)$ | [`src/domain/signals/market_making.rs:26-32, 37-48, 64, 194-212`](../src/domain/signals/market_making.rs#L194-L212) (`l1_delta` `L26-32`, `flow_step` `L38-48`, `eb` `L205`, `ea` `L206`, `ofi_d` `L207`, `ofi_norm` `L212`) | $e_b, e_a, N_{\text{OFI}}, D_{\text{OFI}}$ en $[\text{base}]$; $\Delta t, \tau_{\text{OFI}}$ en $[\text{s}]$ (`ns * 1e-9`); $\text{OFI}_{\text{norm}} \in [-1, 1]$ adimensional | Actualizado **solo** si cambia `event.book` o `BookSnapshot` (`L195`), nunca en `Trade` con libro idéntico; $\Delta t = \max(0, (t - t_{\text{prev}})\cdot 10^{-9})$; retorna `0.0` si $D_{\text{OFI}} \le 10^{-12}$. |
| **TFI Causal ($\tau_{\text{TFI}} = 5\,\text{s}$) & $\alpha_{\text{skew}}$**: $k_{\text{TFI}} = e^{-\Delta t / \tau_{\text{TFI}}},\quad \text{TFI}_{\text{norm}} = \text{clamp}\!\left(\frac{\sum k_{\text{TFI}} s_i q_i}{\sum k_{\text{TFI}} q_i}, -1, 1\right)$<br>$\alpha_{\text{skew}} = \text{clamp}\!\left(0.5\,\text{OFI}_{\text{norm}} + 0.5\,\text{TFI}_{\text{norm}}, -1, 1\right)$ | [`src/domain/signals/market_making.rs:38-48, 65, 214-220`](../src/domain/signals/market_making.rs#L214-L220) (`tfi_d` `L215-218`, `tfi_norm` `L219`, `alpha_skew` `L220`) | $q_i$ en $[\text{base}]$; $\tau_{\text{TFI}} = 5\,\text{s}$; $\text{TFI}_{\text{norm}}, \alpha_{\text{skew}} \in [-1, 1]$ adimensionales | Alimentado **estrictamente** por `TapeEventKind::Trade` con `trade_qty: Some(q)`; `BookSnapshot` sin trade no altera `tfi_norm`; $\alpha_{\text{skew}}$ acotado a $[-1, 1]$. |
| **Volatilidad EWMA ($\tau_{\text{vol}} = 30\,\text{s}$, cap y clamp)**: $v_{\text{inst}} = \min\!\left(\frac{\ln^2(S_{\text{mid},t}/S_{\text{mid},t-1})}{\max(\Delta t, 10^{-3})},\; v_{\max}\right)$<br>$\hat{v}_t = (1 - w_t)\hat{v}_{t-1} + w_t v_{\text{inst}},\quad \sigma_t = \text{clamp}\!\left(\sqrt{\hat{v}_t},\; 10^{-6},\; 0.05\right)$ | [`src/domain/signals/market_making.rs:66-69, 222-235`](../src/domain/signals/market_making.rs#L222-L235) (`defaults` `L66-69`, `iv` `L225`, `w` `L227`, `ewma_var` `L228`, `clamp` `L234-235`) | $v_{\text{inst}}, \hat{v}_t \in [\text{frac}^2 / \text{s}]$; $\sigma_t \in [\text{frac} / \sqrt{\text{s}}]$; $v_{\max} = 0.05^2 = 0.0025$ | Actualiza solo ante movimiento real de mid (`|s_mid - pm| > 0` y `ts > pts`); $w_t = 1$ en el primer desplazamiento (`!vol_init`), luego $1 - e^{-\Delta t / 30}$; antes de inicializar retorna `min_sigma = 1e-6`. |
| **Avellaneda-Stoikov Homogéneo en `[bps]`**: $\sigma_{\text{bps}} = \sigma_t \cdot 10^4$<br>$\text{inv\_var\_bps} = \gamma\,\sigma_{\text{bps}}^2\,T$<br>$r_t = S_{\text{micro}}\left(1 + \frac{\theta_{\text{skew,bps}}\,\alpha_{\text{skew}} - q_{\text{clips}}\,\text{inv\_var\_bps}}{10^4}\right)$<br>$\delta_t^{\text{half}} = \max\!\left(\delta_{\min,\text{bps}},\; \frac{1}{2}\text{inv\_var\_bps} + \frac{1}{\gamma}\ln\!\left(1 + \frac{\gamma}{k}\right)\right)$ | [`src/domain/signals/market_making.rs:266-288`](../src/domain/signals/market_making.rs#L266-L288) (`q_clips` `L279-280`, `sigma_bps` `L281`, `inv_var_bps` `L282`, `shift_bps` `L283`, `res` `L284`, `as_h` `L285`, `dh_bps` `L286`, `df` `L288`) | $\gamma, k \in [\text{bps}^{-1}]$; $\sigma_{\text{bps}} \in [\text{bps}/\sqrt{\text{s}}]$; $T \in [\text{s}]$; $\text{inv\_var\_bps}, \theta_{\text{skew,bps}}, \delta_t^{\text{half}} \in [\text{bps}]$; $q_{\text{clips}} \in [\text{clips}]$ (ver Sección 2.1) | `params.validate()` exige $\gamma, k, T, \text{clip}, \text{max\_inv} > 0$, $\theta, \delta_{\min} \ge 0$; `q_clips` acotado a `[-max_inv, max_inv]`; `shift_bps / 1e4` acotado a `[-0.5, 0.5]`; `df` acotado a `<= 0.5`. |
| **Inferencia Causal `tick_e8` (GCD) y Guardas PostOnly / Autocruce**: $\text{tick}_{e8} = \gcd\!\left(a_t - b_t,\; \lvert b_t - b_{t-1} \rvert,\; \lvert a_t - a_{t-1} \rvert\right)$<br>$P_{\text{bid}}^{\text{raw}} = \min\!\left(\lfloor r_t(1-d_f) \rfloor_{\text{tick}},\; a_{\text{book}} - \text{tick}\right)$<br>$P_{\text{ask}}^{\text{raw}} = \max\!\left(\lceil r_t(1+d_f) \rceil_{\text{tick}},\; b_{\text{book}} + \text{tick}\right)$ | [`src/domain/signals/market_making.rs:17-25, 33-36, 197-202, 211, 289-301`](../src/domain/signals/market_making.rs#L289-L301) (`gcd_pos` `L17-25`, `round_px` `L33-36`, inferencia `L197, 201-202, 211`, PostOnly/autocruce `L289-301`) | `tick_e8`, precios `PriceE8` en $[\text{USDT}\cdot 10^8]$ (`i64`) | Inferido causalmente solo de `BookTop` previos/actuales (`tick_e8_override` opcional); si `raw_q_clips >= max_inv_clips` suprime `bid`, si `<= -max_inv_clips` suprime `ask`; si `bid >= ask`, ensancha 1 tick preservando PostOnly estricto (`b2 < ba && a2 > bb && b2 < a2`). |
| **Spread Capture, Markout ($h \in \{1,5,15\}\text{s}$) y LVR**: $\text{SpCap}_{\text{bps}} = d \cdot \frac{S_{\text{mid}}(t_{\text{fill}}) - P_{\text{fill}}}{P_{\text{fill}}} \cdot 10^4$<br>$\text{MO}_{h,\text{bps}} = d \cdot \frac{S_{\text{mid}}(t_{\text{fill}} + h) - P_{\text{fill}}}{P_{\text{fill}}} \cdot 10^4$<br>$\text{LVR}_{h,\text{bps}} = \max(0,\; -\text{MO}_{h,\text{bps}})$ | [`src/application/mm_markout.rs:5-8, 98-157`](../src/application/mm_markout.rs#L98-L157) (`MARKOUT_*_NS` `L5-8`, `spread_capture_bps` `L109`, `markout_bps` `L139-140`, `lvr_bps` `L141`) | Todos en $[\text{bps}]$ adimensionales; $d \in \{+1, -1\}$ (`Side::sign_f64()`) | Evalúa cada horizonte `1s, 5s, 15s` **solo** al observar un evento posterior con `ts_ns - fill_ts_ns >= h` (cero anticipación); drena `VecDeque` cuando `done == [true, true, true]`. |
| **Contabilidad Exacta `i128` y `Drift == 0`**: $\Delta_{\text{drift}} = \lvert \text{ledger.drift}_{e8} \rvert + \lvert (\text{Cash}_{\text{VIP0},e8} + \sum_s \text{OpenCost}_{s,e8}) - E_{\text{curr},e8} \rvert = 0$ | [`src/application/mm_accounting.rs:204-261, 308-316, 327-337`](../src/application/mm_accounting.rs#L204-L261) (`record_fill` `L204-261`, `accounting_drift_e8` `L308-316`, `signed_leg_fee_e8` `L327-337`) | Todos los montos en $[\text{USDT}\cdot 10^8]$ (`i128`); aritmética verificada `checked_*` (`ovf`) | Soporta cierres parciales y cruces por cero mediante costo promedio entero `released = open_cost * close_qty / pos_abs`; fees $\ge 0$ con `ceil` (`leg_fee_e8`), rebates $< 0$ exactos en `i128`; sincronizado con `EquityLedgerE8`. |
| **Liquidación Obligatoria EOF (`5.50 bps` Taker) y `breakeven_maker_bps`**: $P_{\text{EOF}} = \begin{cases} P_{\text{bid}}^{\text{last}} & Q > 0 \\ P_{\text{ask}}^{\text{last}} & Q < 0 \end{cases}$<br>$\text{BrkMkr}_{\text{bps}} = \frac{\text{GrossPnL}_{e8} - \text{EOFTakerFee}_{e8}}{\text{TotalMakerVol}_{e8}} \cdot 10^4$ | [`src/application/mm_accounting.rs:283-306, 319-324`](../src/application/mm_accounting.rs#L283-L306) (`liquidate_eof` `L283-306`, `breakeven_maker_bps` `L319-324`; llamada en [`falsifier_mm.rs:176`](../src/application/falsifier_mm.rs#L176)) | $\text{EOFCost}_{e8} = \text{EOFSpread}_{e8} + \text{EOFTakerFee}_{e8}$ en $[\text{USDT}\cdot 10^8]$; $\text{BrkMkr}_{\text{bps}} \in [\text{bps}]$ | Cierra todo inventario residual al final de la cinta cruzando el libro real a `taker_bps = 5.50 bps` (`FeeSchedule::bybit_vip0`); retorna `None` en `breakeven` si `total_maker_volume_e8 == 0`. |
| **Cubetas `60s` No Solapadas, `MaxDD` y Lote Estadístico (`M = 6` × 5 Tarifas)**: $r_{k,\text{bps}}^{(p)} = \frac{\text{NetEquity}_{e8}^{(p)}(t_k) - \text{NetEquity}_{e8}^{(p)}(t_{k-1})}{E_{0,e8}} \cdot 10^4$ | [`src/application/mm_accounting.rs:263-281`](../src/application/mm_accounting.rs#L263-L281); [`src/application/falsifier_mm.rs:117-129, 208-226, 304-316`](../src/application/falsifier_mm.rs#L208-L226) | $r_{k,\text{bps}}^{(p)}, \text{MaxDD} \in [\text{bps}]$ sobre capital inicial $E_0$; `bucket_ns = 60_000_000_000` (`60s`) | Marcado a `book_mid_e8` solo en cierre de cubeta (`ts_ns >= bucket_end_ns`) y tras `liquidate_eof`; cada perfil de `MM_FEE_FRONTIER` pasa a `evaluate_hypothesis_batch` con `is_binding_for_verdict = spec.is_binding`. |

### 2.1 Análisis Dimensional Explícito de Avellaneda-Stoikov en `[bps]`

En la formulación clásica de Avellaneda-Stoikov (2008), $\sigma$ suele expresarse en precio absoluto o retorno fraccional por raíz de segundo ($[\text{frac}/\sqrt{\text{s}}]$), lo que exige parametrizar $\gamma$ y $\kappa$ en unidades fraccionales inversas ($[\text{frac}^{-1}] \sim 10^4\,\text{bps}^{-1}$). Dado que en VORTEX-ALFA los parámetros operativos del grid están definidos naturalmente en espacio de puntos básicos ($\gamma \in [0.05, 0.30]\;\text{bps}^{-1}$ y $k \in [0.6, 1.5]\;\text{bps}^{-1}$ en [`default_mm_grid`](../src/application/falsifier_mm.rs#L41-L48)), convertir primero $\sigma$ a `[bps]` antes de multiplicar garantiza consistencia dimensional exacta en [`compute_as_quote`](../src/domain/signals/market_making.rs#L281-L288):

1. **Conversión previa de volatilidad ([L281](../src/domain/signals/market_making.rs#L281))**:
   $$\sigma_{\text{bps}} = \sigma_{[\text{frac}/\sqrt{\text{s}}]} \cdot 10^4\;[\text{bps}/\text{frac}] \implies \left[\sigma_{\text{bps}}\right] = \frac{\text{bps}}{\sqrt{\text{s}}}$$
2. **Término de aversión al inventario ([L282](../src/domain/signals/market_making.rs#L282))**:
   $$\left[\gamma\,\sigma_{\text{bps}}^2\,T\right] = [\text{bps}^{-1}] \cdot \left[\frac{\text{bps}^2}{\text{s}}\right] \cdot [\text{s}] = [\text{bps}]$$
   *(Nota crítica de falsación matemática)*: Si se usara $\sigma$ fraccional ($\sim 2\times 10^{-4}$) directamente con $\gamma \in [\text{bps}^{-1}]$, $\gamma\sigma^2 T$ tendría unidades inconsistentes $[\text{bps}^{-1}\cdot \text{frac}^2]$ y estaría subestimado por un factor de **$(10^4)^2 = 10^8$**, anulando tanto el sesgo de inventario como el ensanchamiento cuadrático del spread!
3. **Término de llegada de órdenes ([L285](../src/domain/signals/market_making.rs#L285))**:
   $$\left[\frac{\gamma}{k}\right] = \frac{[\text{bps}^{-1}]}{[\text{bps}^{-1}]} = [1]\;(\text{adimensional}) \implies \left[\frac{1}{\gamma}\ln\!\left(1 + \frac{\gamma}{k}\right)\right] = \frac{[1]}{[\text{bps}^{-1}]} = [\text{bps}]$$
4. **Conversión final a fracción de precio ([L284, L288](../src/domain/signals/market_making.rs#L284-L288))**:
   Únicamente **después** de sumar todos los términos homogéneos en $[\text{bps}]$ (`shift_bps` y `dh_bps`), el código divide por `BPS_PER_UNIT = 10_000.0` ($10^{-4}\;[\text{frac}/\text{bps}]$) para proyectar a precio en `USDT` sobre $S_{\text{micro}}$.

---

## 3. Máquina de Estados FIFO `5 ms` RTT, Grid de 6 Configuraciones (`M = 6`) y Frontera de 5 Tarifas

### 3.1 Máquina de Estados FIFO L1 con Latencia RTT de `5 ms`

El simulador [`MmFifoSimulator`](../src/application/mm_fifo_sim.rs#L144-L279) mantiene dos slots independientes por símbolo (`SideSlot` bid y ask en [`L92-L99`](../src/application/mm_fifo_sim.rs#L92-L99)):

1. **Estados de ciclo de vida (`OrderState`, [`L46-L74`](../src/application/mm_fifo_sim.rs#L46-L74))**:
   - `Idle`: sin orden activa ni en vuelo.
   - `PendingPlace { target_px, qty_e8, active_at_ns }`: orden enviada en `ts_ns`, inactiva hasta `active_at_ns = ts_ns + 5_000_000` (`DEFAULT_RTT_NS`, [`L12, L230-L232`](../src/application/mm_fifo_sim.rs#L230-L232)).
   - `Resting { px, qty_e8, q_ahead_e8, placed_ts_ns }`: orden viva en el exchange. Al activarse vía [`activate_order`](../src/application/mm_fifo_sim.rs#L281-L300) + [`evaluate_book_queue`](../src/application/mm_fifo_sim.rs#L302-L318):
     - Si `dir * px >= dir * opp_px`, el exchange la rechaza como PostOnly (`post_only_rejects += 1`) y vuelve a `Idle`.
     - Si mejora L1 (`dir * px > dir * own_px`): `q_ahead_e8 = 0`.
     - Si iguala L1 (`px == own_px`): `q_ahead_e8 = own_qty` (cola entera delante).
     - Si es más pasiva que L1: `q_ahead_e8 = BEHIND_L1_QUEUE_E8` (`i64::MAX / 4`).
   - `PendingReplace { current_px, current_qty_e8, current_q_ahead_e8, new_px, new_qty_e8, active_at_ns }`: el reemplazo solo se emite si supera la guarda **anti-churn** doble ([`L246-L261`](../src/application/mm_fifo_sim.rs#L246-L261)): `new_px != live.px`, edad `ts_ns - live.placed_ts_ns >= 100 ms` (`DEFAULT_MIN_ORDER_LIFE_NS`, [`L13`](../src/application/mm_fifo_sim.rs#L13)) y distancia `>= 0.5 bps` (`requote_threshold_bps`).
   - `PendingCancel { current_px, current_qty_e8, current_q_ahead_e8, active_at_ns }`: cancelación en vuelo hasta `ts_ns >= active_at_ns`.
2. **Fills adversos en vuelo (`is_adverse_in_flight = true`)**: mientras un slot está en `PendingCancel` o `PendingReplace` con `ts_ns < active_at_ns` ([`L191-L193`](../src/application/mm_fifo_sim.rs#L191-L193)), la orden original sigue expuesta en el libro del exchange; si un trade o cruce de libro la ejecuta antes de cumplirse los `5 ms` RTT, el fill se contabiliza normalmente y se marca como adverso en vuelo (`adverse_in_flight_fills += 1`).
3. **Regla conservadora de fill FIFO ([`check_side_fill`, `L320-L346`](../src/application/mm_fifo_sim.rs#L320-L346))**:
   - Si el libro cruza el precio de la orden (`book_crossed`, `L328-L330`) o un trade ocurre estrictamente a peor precio para el agresor (`dir * trade_px < dir * px`, `L335-L337`), la orden se llena inmediatamente.
   - Si el trade ocurre exactamente al nivel `px` por el lado contrario (`trade_px == px && event.side != side`, `L338-L343`), primero se recorta la cola por el tamaño visible actual (`*q_ahead_e8 = (*q_ahead_e8).min(l1_queue_cap)` en `L331`), se descuenta `trade_qty`, y la orden se llena **únicamente** cuando `q_ahead_e8 - trade_qty <= -order_qty_e8` (cero fills parciales fantasma).

### 3.2 Grid de 6 Configuraciones Avellaneda-Stoikov (`M = 6`)

Definido en [`default_mm_grid`](../src/application/falsifier_mm.rs#L24-L49) con defaults de [`MmFalsifierConfig::default`](../src/application/falsifier_mm.rs#L51-L64) (`capital_usdt = 2_000.0`, `clip_usdt = 100.0`, `max_inv_clips = 4.0`, `max_gross_leverage = 2.0`, `bucket_ns = 60s`, umbral Holm-Bonferroni para `M = 6`: $t_{\text{crit}}(6) = \max(3.50,\; \Phi^{-1}(1 - 0.01/12)) = 3.5000$):

| # | `config_name` | $\gamma\;[\text{bps}^{-1}]$ | $\theta_{\text{skew}}\;[\text{bps}]$ | $\delta_{\min}\;[\text{bps}]$ | $T\;[\text{s}]$ | $k\;[\text{bps}^{-1}]$ | Perfil Estratégico |
|---:|---|---:|---:|---:|---:|---:|---|
| 1 | `AS_Base_Tight` | 0.05 | 0.0 | 1.0 | 30.0 | 1.5 | Base simétrica agresiva sin skew |
| 2 | `AS_Base_Wide` | 0.15 | 0.0 | 2.5 | 60.0 | 1.0 | Base simétrica conservadora sin skew |
| 3 | `AS_Skew_Tight` | 0.05 | 2.0 | 1.0 | 30.0 | 1.5 | Tight + skew moderado OFI/TFI |
| 4 | `AS_Skew_Mid` | 0.10 | 3.0 | 2.0 | 45.0 | 1.2 | Configuración balanceada media |
| 5 | `AS_Skew_Wide` | 0.20 | 4.0 | 3.5 | 60.0 | 0.8 | Wide + aversión y skew altos |
| 6 | `AS_Skew_Deep` | 0.30 | 5.0 | 5.0 | 90.0 | 0.6 | Profunda defensiva anti-selección adversa |

### 3.3 Frontera de 5 Tarifas (`MM_FEE_FRONTIER`)

Definida en [`MM_FEE_FRONTIER`](../src/application/mm_accounting.rs#L36-L42) y evaluada por lote `M = 6` en [`build_mm_report`](../src/application/falsifier_mm.rs#L208-L250):

| # | Perfil | `maker_bps` | `eof_taker_bps` (`Bybit VIP-0`) | `is_binding` | Rol en el Laboratorio |
|---:|---|---:|---:|:---:|---|
| 1 | `VIP0_Maker` | `+2.00` | `5.50` | **`true`** | **Único perfil vinculante** capaz de emitir `GateVerdict::Pass` (`t_nw >= 3.50`, `DSR >= 0.95`, `N >= max(5, n_min)`) |
| 2 | `Venue_1bp` | `+1.00` | `5.50` | `false` | Diagnóstico de sensibilidad (tier intermedio `+1.0 bp`) |
| 3 | `Venue_0bp` | `0.00` | `5.50` | `false` | Diagnóstico de sensibilidad (maker cero) |
| 4 | `Rebate_05` | `-0.50` | `5.50` | `false` | Diagnóstico de sensibilidad (rebate maker `-0.5 bp`) |
| 5 | `Rebate_10` | `-1.00` | `5.50` | `false` | Diagnóstico de sensibilidad (rebate maker `-1.0 bp`) |

---

## 4. Catálogo Forense `input -> expected -> actual` de [`tests/market_making_tests.rs`](../tests/market_making_tests.rs) (`6` Suites)

| # | Test (`archivo:línea`) | Input Concreto | Expected Analítico | Actual Observado |
|---|---|---|---|---|
| 1 | `test_mm_stoikov_ofi_tfi_as_and_postonly_analytical` (`market_making_tests.rs:156-471`) | `BookTop(100,101,3,1)` y `(100,101,2,2)`; OFI 4 pasos (`min_book_updates=4`, $\tau_{\text{OFI}}=2\text{s}$); TFI `t=4s` Buy `4.0`, `t=5s` snapshot sin trade, `t=6.5s` Sell `6.0` ($\tau_{\text{TFI}}=5\text{s}$); EWMA vol `100.0 -> 100.1 -> 120.0` ($\tau_{\text{vol}}=30\text{s}$, `max_inst_var=0.0025`); AS `[bps]` (`wide_book(99.60,100.20)`, `s_micro=100.02`, `alpha=0.4`, $\sigma=2\times 10^{-4}$, `inv=150`, `tick=0.01`); PostOnly en `tight_book(100.00,100.01)` (`alpha=±1, theta=50`), `locked_book(100,100)`, `wider_book(99.90,100.10)`, `inv=±400, ±500`; 7 inputs inválidos | $S_{\text{micro}} \in \{100.75, 100.50\}$; `is_warm=false` en pasos `0..2`, `true` en `3`; $\text{OFI}_3 = \frac{\text{num}_2 e^{-0.5} - 9}{\text{den}_2 e^{-0.5} + 9}$; $\text{TFI}_5 = 1.0$ (sin trade), $\text{TFI}_6 = \frac{4e^{-0.5}-6}{4e^{-0.5}+6}$; $\sigma_{\text{pre}}=10^{-6}$, $\sigma_2=\sqrt{k_{\text{vol}}iv_1 + (1-k_{\text{vol}})0.0025}$; AS: $\sigma_{\text{bps}}=2.0$, $\text{inv\_var}=18.0$, `shift=-25.8 bps`, $r_t=99.7619484$, $\delta_h=9.0+10\ln(1+1/12)$, `bid/ask` redondeados a `0.01`; PostOnly/locked siempre `bid < ask_book`, `ask > bid_book`, `bid < ask`; `inv>=400 => bid=None`, `inv<=-400 => ask=None`; errores tipados exactos | Match exacto (`tol = 1e-12` / `1e-15`) y todos los errores tipados (`NonMonotonicTimestamp`, `InvalidStatParam`, `NonFiniteValue`) verificados — **PASS** |
| 2 | `test_mm_fifo_queue_conservation_and_5ms_adverse_latency` (`market_making_tests.rs:473-666`) | **A**: bid `1.0 @ 100.00` sobre `bq=10.0`, activa `5ms`, trades sell `6.0` (`10ms`), `4.5` (`15ms`), `0.5` (`20ms`). **B**: bid `1.0 @ 100.00` sobre `bq=0.5`, cancel en `t0=35ms` + trade `99.90` en `t0+3ms`; segundo cancel en `t1=60ms` + trade `99.90` en `t1+6ms`. **C**: bid `100.00`, libro baja a `99.90/99.95` en `t+5ms`. **D**: orden resting en `105ms`, requotes a `+50ms` (`99.50`), `+120ms` (`100.004`, qty `1.25`), `+125ms` (`100.006`, qty `1.25`) | **A**: `q_ahead` pasa `10 -> 4 -> -0.5` (`0` fills) `-> -1.0` (`1` fill exacto `1.0 @ 100.00`, `is_adverse=false`, `Idle`). **B**: `t0+3ms < 5ms` produce fill adverso (`is_adverse_in_flight=true`, `adverse_in_flight_fills=1`); `t1+6ms >= 5ms` produce `0` fills. **C**: `post_only_rejects=1`, `0` fills, `Idle`. **D**: `+50ms` (< `100ms`) ignorado; `+120ms` (`0.4 bps < 0.5 bps`) ignorado; `+125ms` (`0.6 bps >= 0.5 bps`) pasa a `PendingReplace` | Todas las transiciones `OrderState`, `q_ahead_e8`, contadores y fills coinciden exactamente — **PASS** |
| 3 | `test_mm_markout_lvr_eof_liquidation_breakeven_and_zero_drift` (`market_making_tests.rs:668-839`) | `MM_Exact_Test` (`capital=1000`, `clip=100`, `min_half=60 bps`, `tick=0.01`): `b_init(99.91, 101.31)` (`mid=100.61`), trade sell `1.0 @ 100.00` en `t=3s`, snapshots a `+1s` (`mid=99.50`), `+5s` (`mid=99.80`), `+15s` (`100.10/100.30`, `mid=100.20`); además test directo `MmAccountLedger`: `+2.0@100 -> -0.5@101 -> -2.5@99.50 -> +1.5@99.00 -> liquidate_eof(5.50 bps)` en `(99.20, 99.40)` | `SpCap = +61.00 bps`, `MO1s = -50.00`, `MO5s = -20.00`, `MO15s = +20.00`, `LVR5s = +20.00 bps`; EOF de `+1.0` en `bid=100.10`: `eof_fee_e8 = ceil(100.10e8 * 55_000 / 1e8) = 5_505_500 e8` (`0.055055 USDT`), `eof_spread = 0.10`, `eof_cost = 0.155055`, `gross = +0.10`, `breakeven_maker_bps = +4.4945 bps`; frontera estrictamente monótona; `accounting_drift_e8 == 0` en todo momento | Match exacto (`tol = 1e-9 / 1e-12`), orden estricto de los 5 puntos de frontera y `drift == 0` — **PASS** |
| 4 | `test_mm_diagnostic_frontier_never_passes_binding_gate` (`market_making_tests.rs:841-944`) | `AS_Tight_Diag` (`min_half=1.45 bps`, `tick=$0.10` en `mid=$10,000` -> quotes `9998.50/10001.50`, `+3.0 bps` bruto RT), `20` cubetas `60s` con `[4..6]` RTs/cubeta (`clip=1000`), evaluado sobre `MM_FEE_FRONTIER` | `VIP0_Maker` (`4.0 bps` RT > `3.0 bps` bruto): `is_binding=true`, `net_pnl < 0`, `mean < 0`, `GateVerdict::Fail`; los `4` perfiles diagnósticos obtienen `t_nw >= 3.5` y `dsr >= 0.95` pero `is_binding=false` y **nunca** producen `GateVerdict::Pass`; `drift == 0` | `buckets_n=20`, `VIP0_Maker=Fail`, `4/4` perfiles diagnósticos cumplen `t_nw>=3.5 && dsr>=0.95` con `verdict != Pass`, `drift == 0` — **PASS** |
| 5 | `test_mm_zero_lookahead_and_strict_symbol_isolation` (`market_making_tests.rs:992-1217`) | **(a)** Cinta base `150s` vs copia mutada para `ts > 90s` (`10x` precio, `100x` qty, lados invertidos). **(b)** `sym1=1` sola vs intercalada con `sym2=2` con saltos `50x` (`100 <-> 5000`) y liquidaciones `500_000 USDT` en `MmSignalEngine`, `MmFifoSimulator` y `run_mm_falsification_detailed` | **(a)** Fills con `fill_ts_ns <= 90s` y `bucket 0 [0, 60s)` idénticos bit-a-bit (`to_bits()`) en las `6` configs; última cubeta diverge. **(b)** Features, quotes, fills y `OrderState` de `sym1` idénticos bit-a-bit con `drift == 0` | Igualdad exacta bit-a-bit en prefijo pre-cutoff, cubeta `0` y aislamiento completo por `SymbolId` — **PASS** |
| 6 | `test_cli_scan_mm_multifile_par2_and_json_report` (`market_making_tests.rs:1247-1394`) | 2 archivos `PAR2` (`seg_001.parquet` + `seg_002.parquet`) abarcando warmup + `3` cubetas `60s`, invocado vía `run_cli scan-mm --format par2 --capital-usdt 2000 --clip-usdt 100 --max-inv-clips 4 --out ...`; más `5` invocaciones inválidas (`--capital-usdt 0`, `-100`, `--unknown-flag`, sin `--format`, sin `--input`) | ASCII incluye `"Track4-MarketMaking"`, `"FEE FRONTIER DIAGNOSTICS"`, `"VIP0_Maker"`, `"AS_Base_Tight"`, `"AS_Skew_Deep"`; JSON deserializa a `MmFalsificationReport` con `m_hypotheses=6`, `buckets_n=3`, `fills_total>=4`, `frontier.len()=5`, `drift=0`; `5/5` rechazos retornan `Err(DomainError::Cli(_))` | Reporte ASCII, roundtrip JSON y `5/5` rechazos CLI exactos — **PASS** |

---

## 5. Salida Cruda Sin Editar de las 6 Compuertas Bloqueantes en PowerShell 7 + `git status --short`

> Salida capturada literalmente en `D:\Desarrollo\VORTEX-ALFA` tras completar la implementación y formateo (`cargo fmt`) del código Track 4. Los archivos de documentación en `DOCS/` (`DOCS/PHASE_3A_TRACK4_MM_DESIGN_REPORT.md` y `DOCS/INDEX.md`) fueron redactados inmediatamente después de capturar esta corrida de 6 compuertas, sin modificar ningún archivo `.rs` ni `Cargo.toml`.

```text
=== GATE 1: cargo fmt --all -- --check ===
EXIT_CODE=0
=== GATE 2: cargo clippy --all-targets --all-features -- -D warnings ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.14s
EXIT_CODE=0
=== GATE 3: cargo test --all-targets -- --nocapture ===
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
test test_garman_klass_closed_form_and_flat_bar ... ok
test test_moments4_closed_form_exact ... ok
test test_newey_west_closed_form_n8 ... ok
test test_holm_bonferroni_order_and_monotonicity ... ok
test test_hawkes_o1_three_step_analytical ... ok
test test_vip0_ceil_fee_e8_and_book_roundtrip_asymmetry ... ok
test test_dsr_cross_sectional_std_and_diag_never_passes ... ok
test test_exact_funding_boundary_bar_open_vs_bar_close ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests\cross_sectional_tests.rs (target\debug\deps\cross_sectional_tests-a086dcf4759e346e.exe)

running 7 tests
test test_xs_momentum_vol_adj_skip_and_gk_analytical ... ok
test test_xs_residual_reversion_ols_beta_and_clamp_analytical ... ok
test test_xs_funding_carry_closed_form_and_ranking ... ok
test test_xs_edge_cases_zero_var_flat_bar_ties_insufficient_symbols_and_non_finite ... ok
test test_exact_turnover_funding_bar_open_eof_and_maker_never_passes ... ok
test test_zero_lookahead_end_to_end_track3_xs ... ok
test test_cli_scan_xs_csv_and_global27_report ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s

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

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s

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
test test_zero_lookahead_end_to_end_track2_meso ... ok
test test_zero_lookahead_end_to_end_track1_micro ... ok
test test_strict_symbol_isolation_track2_meso ... ok
test test_strict_symbol_isolation_track1_micro ... ok
test test_multifile_par2_vpz1_vkl1_streaming_and_cli ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s

EXIT_CODE=0
=== GATE 4: src/**/*.rs <= 350 lines ===

File                                                              Lines Pass
----                                                              ----- ----
D:\Desarrollo\VORTEX-ALFA\src\lib.rs                                  7 True
D:\Desarrollo\VORTEX-ALFA\src\main.rs                                17 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_meso.rs         330 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_micro.rs        280 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_mm.rs           337 True
D:\Desarrollo\VORTEX-ALFA\src\application\falsifier_xs.rs           334 True
D:\Desarrollo\VORTEX-ALFA\src\application\mm_accounting.rs          345 True
D:\Desarrollo\VORTEX-ALFA\src\application\mm_fifo_sim.rs            346 True
D:\Desarrollo\VORTEX-ALFA\src\application\mm_markout.rs             158 True
D:\Desarrollo\VORTEX-ALFA\src\application\mod.rs                      7 True
D:\Desarrollo\VORTEX-ALFA\src\domain\error.rs                        45 True
D:\Desarrollo\VORTEX-ALFA\src\domain\fees.rs                        179 True
D:\Desarrollo\VORTEX-ALFA\src\domain\ledger.rs                      113 True
D:\Desarrollo\VORTEX-ALFA\src\domain\mod.rs                           8 True
D:\Desarrollo\VORTEX-ALFA\src\domain\normal_dist.rs                 217 True
D:\Desarrollo\VORTEX-ALFA\src\domain\ports.rs                        18 True
D:\Desarrollo\VORTEX-ALFA\src\domain\stats.rs                       348 True
D:\Desarrollo\VORTEX-ALFA\src\domain\types.rs                       331 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\cross_sectional.rs     306 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\market_making.rs       310 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\meso_energy.rs         331 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\micro_detectors.rs     226 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\micro_tail.rs          330 True
D:\Desarrollo\VORTEX-ALFA\src\domain\signals\mod.rs                   5 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\binary_codec.rs        342 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\csv_candle_reader.rs   319 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\mod.rs                   4 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\tape_reader.rs         325 True
D:\Desarrollo\VORTEX-ALFA\src\infrastructure\vkl1_reader.rs         154 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli.rs                     327 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\cli_mm.rs                  136 True
D:\Desarrollo\VORTEX-ALFA\src\interfaces\mod.rs                       2 True


=== GATE 5: banned patterns in src/**/*.rs ===
MATCH_COUNT=0
=== GATE 6: UTF-8 NoBOM + LF-Only across src & tests ===

File                          UTF8_NoBOM LF_Only
----                          ---------- -------
lib.rs                              True    True
main.rs                             True    True
falsifier_meso.rs                   True    True
falsifier_micro.rs                  True    True
falsifier_mm.rs                     True    True
falsifier_xs.rs                     True    True
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
market_making.rs                    True    True
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
cli_mm.rs                           True    True
mod.rs                              True    True
adversarial_math_tests.rs           True    True
cross_sectional_tests.rs            True    True
edge_cases_and_drift_tests.rs       True    True
market_making_tests.rs              True    True
zero_lookahead_tests.rs             True    True


=== GIT STATUS ===
 M src/application/mod.rs
 M src/domain/signals/mod.rs
 M src/interfaces/cli.rs
 M src/interfaces/mod.rs
?? src/application/falsifier_mm.rs
?? src/application/mm_accounting.rs
?? src/application/mm_fifo_sim.rs
?? src/application/mm_markout.rs
?? src/domain/signals/market_making.rs
?? src/interfaces/cli_mm.rs
?? tests/market_making_tests.rs
```

---

## 6. Comandos Listos para Barrido Empírico en `sniper-tokio` (Fase 3B)

> **Modo estricto de solo lectura** sobre `/home/ubuntu/vortex-rs/data`: [`StreamingTapeReader`](../src/infrastructure/tape_reader.rs#L53-L69) recorre recursivamente el directorio indicado filtrando exclusivamente archivos `.par2z` (`--format vpz1`) o `.par2`/`.parquet` (`--format par2`) y los ordena cronológicamente por primer timestamp. Todas las salidas (`--out` y logs de `/usr/bin/time -v`) se escriben bajo `/tmp/`.

### 6.1 Compilación Release y Verificación en `sniper-tokio`

```bash
cd ~/VORTEX-ALFA
CARGO_BUILD_JOBS=1 cargo test --all-targets
CARGO_BUILD_JOBS=1 cargo build --release
```

### 6.2 Barrido 1 — Cold Archive `.par2z` (`VPZ1`, `19.47M` eventos reales)

```bash
# Escenario A: Capital $2,000 | Clip $100 | Máx 4 clips
/usr/bin/time -v ./target/release/vortex-alfa scan-mm \
  --format vpz1 \
  --input /home/ubuntu/vortex-rs/data \
  --capital-usdt 2000 \
  --clip-usdt 100 \
  --max-inv-clips 4 \
  --out /tmp/mm_vpz1_cap2000.json \
  2>&1 | tee /tmp/mm_vpz1_cap2000.log

# Escenario B: Capital $1,000 | Clip $50 | Máx 4 clips
/usr/bin/time -v ./target/release/vortex-alfa scan-mm \
  --format vpz1 \
  --input /home/ubuntu/vortex-rs/data \
  --capital-usdt 1000 \
  --clip-usdt 50 \
  --max-inv-clips 4 \
  --out /tmp/mm_vpz1_cap1000.json \
  2>&1 | tee /tmp/mm_vpz1_cap1000.log
```

### 6.3 Barrido 2 — Segmentos `.parquet` (`PAR2`, `1.05M` eventos reales)

```bash
# Escenario A: Capital $2,000 | Clip $100 | Máx 4 clips
/usr/bin/time -v ./target/release/vortex-alfa scan-mm \
  --format par2 \
  --input /home/ubuntu/vortex-rs/data \
  --capital-usdt 2000 \
  --clip-usdt 100 \
  --max-inv-clips 4 \
  --out /tmp/mm_par2_cap2000.json \
  2>&1 | tee /tmp/mm_par2_cap2000.log

# Escenario B: Capital $1,000 | Clip $50 | Máx 4 clips
/usr/bin/time -v ./target/release/vortex-alfa scan-mm \
  --format par2 \
  --input /home/ubuntu/vortex-rs/data \
  --capital-usdt 1000 \
  --clip-usdt 50 \
  --max-inv-clips 4 \
  --out /tmp/mm_par2_cap1000.json \
  2>&1 | tee /tmp/mm_par2_cap1000.log
```

> **Nota sobre aislamiento de subdirectorios y telemetría**: Si bajo `/home/ubuntu/vortex-rs/data` coexistieran múltiples capturas independientes de `.par2z` o `.parquet` con rangos horarios distintos, apunte `--input` al subdirectorio específico de la captura correspondiente para evitar mezclar sesiones disjuntas. Verifique en cada log `Elapsed (wall clock) time`, `Maximum resident set size (kbytes)` (`Max RSS`), `Swaps: 0`, `Exit status: 0`, `Drift == 0` y los JSON generados en `/tmp/mm_*.json` antes de emitir el veredicto empírico de Fase 3B.

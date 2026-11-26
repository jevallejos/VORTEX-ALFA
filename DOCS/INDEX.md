# VORTEX-ALFA — Documentation Index

> **Status**: Phase 0 Forensically Closed | Phase 1A Track 2 Falsified (`0/36`) | Phase 1B Track 1 Falsified (`0/20`) | Phase 2 Track 3 Falsified (`0/27`) | Phase 3B Track 4 MM Falsified (`0/6`; `0/30` frontier) | Phase 4 Track 5 Macro `1d` Verified (`1/9 Pass`: `Carry_SpotPerp_TopCarry30d`; `41/41` tests `ok`) | **Architecture**: Hexagonal Clean DDD (`src/` <= 350 lines/file, 0 `unsafe`, 0 `unwrap`/`expect`/`panic!`)
> **Primary Audit Reports**: [`PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md) | [`PHASE_1A_TRACK2_FALSIFICATION_REPORT.md`](./PHASE_1A_TRACK2_FALSIFICATION_REPORT.md) | [`PHASE_1B_TRACK1_FALSIFICATION_REPORT.md`](./PHASE_1B_TRACK1_FALSIFICATION_REPORT.md) | [`PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md`](./PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md) | [`PHASE_3A_TRACK4_MM_DESIGN_REPORT.md`](./PHASE_3A_TRACK4_MM_DESIGN_REPORT.md) | [`PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md`](./PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md) | [`PHASE_4_TRACK5_MACRO_FALSIFICATION_REPORT.md`](./PHASE_4_TRACK5_MACRO_FALSIFICATION_REPORT.md)

---

## 1. Document Registry

| Document | Purpose | Scope |
|---|---|---|
| [`INDEX.md`](./INDEX.md) | Navigable architecture map, CLI quickstart, and phase verification summary | Workspace entrypoint |
| [`PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md) | Forensic closure audit: `VORTEX-L2` read-only layout inspection, file/layer/crate inventory, `LaTeX -> archivo:linea` math traceability table, 4-D compliance matrix, raw unedited 6-gate PS7 outputs, and 22-test `input -> expected -> actual` vector catalog | Complete Phase 0 sign-off |
| [`PHASE_1A_TRACK2_FALSIFICATION_REPORT.md`](./PHASE_1A_TRACK2_FALSIFICATION_REPORT.md) | Empirical Track 2 falsification over 24 months of real Bybit V5 linear perpetuals (`15m`, `1h`, `4h`, 12 pairs, real funding): data integrity manifest, 36-cell primary matrix (`VIP-0 + 1.5 bps` slip), Year 1 vs. Year 2 OOS stability, `$64 USDT` Ex-BTC & per-symbol concentration audit, slippage sensitivity (`0.5 / 1.5 / 3.0 bps`) vs. `DiagZero`, raw PS gates, and final falsification verdict (`0/36 Pass`) | Complete Phase 1A Track 2 sign-off |
| [`PHASE_1B_TRACK1_FALSIFICATION_REPORT.md`](./PHASE_1B_TRACK1_FALSIFICATION_REPORT.md) | Empirical Track 1 falsification on `sniper-tokio` (`rustc 1.99.1`, `--release`, read-only `/home/ubuntu/vortex-rs/data`) across `20,518,144` (`~20.52M`) real events: Run 1 `.par2z` (`VPZ1`, 12 hourly files, `19,469,568` events) + Run 2 `.parquet` (`PAR2`, `4,096 × 256 = 1,048,576` events), 20-cell `VIP-0` matrix vs. `DiagZero` range, structural `binance_mid = None` audit, and final falsification verdict (`0/20 Pass`) | Complete Phase 1B Track 1 sign-off |
| [`PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md`](./PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md) | Empirical Track 3 cross-sectional falsification over 24 months of 30 Bybit V5 linear perpetuals (`4h`, `0:BTCUSDT..29:ICPUSDT`): causal Option A funding aggregation (`INJUSDT` 128 sub-4h events), explicit survivorship bias disclosure, `LaTeX -> archivo:linea` traceability, 27-cell matrix (`K ∈ {1,2,3} × 3 × 3`), Year 1 vs. Year 2 OOS stability, slippage sensitivity (`0.5 / 1.5 / 3.0 bps`) vs. `VIP0-MakerDiag`, `$64 USDT` sizing caveats, raw 6-gate PS7 outputs (`29/29` tests), and final falsification verdict (`0/27 Pass`) | Complete Phase 2 Track 3 sign-off |
| [`PHASE_3A_TRACK4_MM_DESIGN_REPORT.md`](./PHASE_3A_TRACK4_MM_DESIGN_REPORT.md) | Phase 3A Track 4 Market Making design & verification closure: 4-D compliance matrix, SRP hexagonal split (`market_making.rs`, `mm_markout.rs`, `mm_fifo_sim.rs`, `mm_accounting.rs`, `falsifier_mm.rs`, `cli_mm.rs`), `LaTeX -> archivo:linea` traceability + homogeneous `[bps]`-space Avellaneda-Stoikov dimensional proof, 5 ms RTT FIFO L1 state machine, `M=6` AS grid & 5-fee frontier (`VIP0_Maker` solely binding + mandatory EOF `5.50 bps` Taker), 6-test `input -> expected -> actual` catalog, raw 6-gate PS7 outputs (`35/35` tests, `32/32` `src/` files `<= 350` lines), and copy-paste `sniper-tokio` sweep commands | Complete Phase 3A Track 4 sign-off |
| [`PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md`](./PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md) | Empirical Track 4 Market Making falsification on `sniper-tokio` (`rustc 1.99.1`, `35/35` tests ok, `--release`, read-only `/home/ubuntu/vortex-rs/data`) across 4 runs (`VPZ1` `19.47M` & `PAR2` `1.05M` events under `$2,000/$100` & `$1,000/$50`): `0/6 Pass` on binding `VIP0_Maker`, `0/30` positive across 5-fee frontier (even `-1.00 bps` rebate), `< 1s` toxicity (`93%` of damage, `MO1s = -1.12 bps`), `+157.9%` skew adverse in-flight fills (`1,134 -> 2,925`), `+3.20 bps/leg` (`+6.40 bps` RT) VIP-0 gap, `book_crossed`/`trade_through` scale invariance, `Drift == 0`, empirical closure of Camino B, and transition rationale to Track 5 Macro `1d` | Complete Phase 3B Track 4 sign-off |
| [`PHASE_4_TRACK5_MACRO_FALSIFICATION_REPORT.md`](./PHASE_4_TRACK5_MACRO_FALSIFICATION_REPORT.md) | Empirical Track 5 Macro (`1d`) falsification over 5 years (`2021-11-25` to `2026-11-25`, `1,826` daily bars) of 12 Bybit V5 dual `linear + spot` majors (`0:BTCUSDT..11:ATOMUSDT`): 4-D compliance matrix, `audit_report.json` (`72` CSVs, `87,438` rows, `PASS`), real `BNBUSDT` Spot `2022-03-10` listing & `SOLUSDT` `[2.0, 8.0]h` causal funding aggregation, survivorship bias disclosure, `LaTeX -> archivo:linea` traceability, 9-strategy + 2-benchmark primary matrix (`1/9 Pass`: `Carry_SpotPerp_TopCarry30d` `CAGR 3.00%`, `AnnSR 4.75`, `MaxDD 0.55%`, `t_nw = 8.594`, `DSR = 0.9967`), bi-regime OOS stability (`R1` `Pass` vs `R2` `InsufficientSample` under `-53.7%` funding compression), slippage sensitivity (`0.5 / 1.5 / 3.0 bps`), `$1,000–$2,000 USDT` operational feasibility, raw 6-gate PS outputs (`41/41` tests, `40/40` `src/` files `<= 350` lines), and final verdict | Complete Phase 4 Track 5 sign-off |

---

## 2. Hexagonal Module Map

Dependencies point strictly inward (`Interfaces -> Application -> Domain <- Infrastructure`):

```text
src/
├── lib.rs                              # Crate root: #![forbid(unsafe_code)], #![deny(clippy::unwrap_used, expect_used, panic)]
├── main.rs                             # Thin binary entrypoint delegating to interfaces::cli::run_cli
├── domain/                             # Pure domain math, invariants, and ports (zero I/O, zero infra deps)
│   ├── mod.rs
│   ├── error.rs                        # DomainError (thiserror::Error) across all hexagonal boundaries
│   ├── types.rs                        # PriceE8, QtyE8, NotionalE8, Bps, TimestampNs, BookTop, Candle, TapeEvent, FundingTimestampSemantics
│   ├── fees.rs                         # FeeSchedule (VIP-0 binding vs DiagZero non-binding), leg_fee_e8 ceil, Ask->Bid & Candle net bps
│   ├── ledger.rs                       # EquityLedgerE8 (i128 checked arithmetic, zero-drift invariant)
│   ├── normal_dist.rs                  # Cody (1969) erfc_exact, normal_cdf, Acklam + Halley normal_inv_cdf
│   ├── stats.rs                        # Welford, Moments4, Newey-West Bartlett LRV, Holm-Bonferroni, DSR (real sr_cross_std), n_min, GateVerdict
│   ├── ports.rs                        # Traits: MarketTapeReader, CandleSeriesReader, ReportSink
│   └── signals/
│       ├── mod.rs
│       ├── micro_detectors.rs          # HawkesProcess O(1) Ozaki, ShiryaevRobertsDetector log-space, CausalQuantileWindow
│       ├── micro_tail.rs               # Track 1 engine: HawkesTail, ShiryaevRobertsJump, LiquidationCascade, CrossExchangeShock
│       ├── meso_energy.rs              # Track 2 engine: Garman-Klass vol, r_comp, TSMOM-VW, Z-funding Welford
│       ├── cross_sectional.rs          # Track 3 engine: XsFundingCarry, XsResidualReversion, XsMomentumVolAdj, build_dollar_neutral_weights
│       ├── market_making.rs            # Track 4 engine: Stoikov microprice, CKS L1 OFI, TFI, EWMA vol, causal tick GCD, [bps]-space AS + PostOnly
│       ├── macro_vol.rs                # Track 5 vol/sizing: CausalEwmaVol (span=20 sample init, clamp [0.15, 3.00]), vol-target weight scaling & gross cap
│       ├── macro_trend.rs              # Track 5 trend: MacroTrendFeatureEngine (GK + EWMA + 20/60/120d TSMOM) & VolTargetTrendStrategy (band 0.02)
│       ├── macro_donchian.rs           # Track 5 breakout: DonchianBreakoutStrategy (strictly prior [t-n..=t-1] entry/exit channels, inv-vol sizing)
│       └── spot_perp_carry.rs          # Track 5 carry: SpotPerpCarryEngine (rolling APR hysteresis, dual linear+spot filter, 1d/30d cadence, 0.5*alloc)
├── application/                        # Causal multi-horizon falsification orchestrators (depend only on domain)
│   ├── mod.rs
│   ├── falsifier_micro.rs              # Track 1 orchestrator: RTT delay, cooldown, non-overlapping slots, VIP-0 + DiagZero dual eval
│   ├── falsifier_meso.rs               # Track 2 orchestrator: bar t+1 open entry, BarOpenSettlement vs BarCloseSettlement funding
│   ├── falsifier_xs.rs                 # Track 3 orchestrator: 1x gross 2L dollar-neutral, BarOpenSettlement, rolling/EOF turnover, VIP0-Taker vs VIP0-MakerDiag
│   ├── mm_markout.rs                   # Track 4 markout tracker: causal spread capture, 1s/5s/15s markouts, LVR = max(0, -markout)
│   ├── mm_fifo_sim.rs                  # Track 4 FIFO L1 simulator: 5ms RTT place/replace/cancel, adverse in-flight fills, PostOnly reject, anti-churn
│   ├── mm_accounting.rs                # Track 4 ledger (i128 Drift==0), 5-profile MM_FEE_FRONTIER, mandatory EOF taker liquidation, breakeven_maker_bps
│   ├── falsifier_mm.rs                 # Track 4 orchestrator: M=6 AS grid, 60s non-overlapping buckets, max gross leverage cap, Holm/NW/DSR eval
│   ├── macro_baselines.rs              # Track 5 baselines & equity metrics: MacroConfig, compute_equity_metrics (CAGR/AnnSR/MaxDD/Calmar), BuyHold_BTC & BuyHold_EW
│   ├── macro_accounting.rs             # Track 5 accounting: MacroFeeProfiles (Spot 10.00 bps + Perp 5.50 bps + slip vs MakerDiag), real Spot-Perp basis, BarOpenSettlement, EOF
│   └── falsifier_macro.rs              # Track 5 orchestrator: dual linear+spot slice alignment, Open(t+1) entry, 9-strategy Holm/NW/DSR + 2 passive benchmarks
├── infrastructure/                     # Adapters implementing domain ports & binary codecs
│   ├── mod.rs
│   ├── binary_codec.rs                 # Pure-Rust FNV-1a, PAR2 (256B), VPZ1 (36B hdr, Stride-256 XOR-Delta + Zero-RLE), VKL1 (96B)
│   ├── tape_reader.rs                  # StreamingTapeReader (Par2 | Vpz1 | Jsonl) implementing MarketTapeReader
│   ├── vkl1_reader.rs                  # StreamingVkl1Reader (.vkl1 directory/file) implementing CandleSeriesReader
│   └── csv_candle_reader.rs            # CsvCandleReader (ts_ns|ts_ms, funding_rate_bps|funding_rate) implementing CandleSeriesReader
└── interfaces/                         # CLI presentation & JSON ReportSink adapter
    ├── mod.rs
    ├── cli.rs                          # Subcommands scan-micro, scan-meso, scan-xs, scan-mm & scan-macro dispatch, ASCII tables, JSON --out writer
    ├── cli_mm.rs                       # Track 4 scan-mm argument parser, ASCII table + 5-profile fee frontier formatter
    └── cli_macro.rs                    # Track 5 scan-macro argument parser, ASCII table + passive Spot benchmarks formatter
```

---

## 3. CLI Usage (`vortex-alfa`)

### Track 1 — Microstructure Tail Falsifier (`scan-micro`)

Evaluates 4 micro hypotheses (`HawkesTail`, `ShiryaevRobertsJump`, `LiquidationCascade`, `CrossExchangeShock`) across 5 default horizons (`15s, 60s, 180s, 600s, 900s`, $M = 20$) under both `VIP-0` (binding) and `DiagZero` (non-binding):

```powershell
cargo run --release -- scan-micro --format <par2|vpz1|jsonl> --input <path> [--out <report.json>]
```

- Accepted `--format` values: `par2`, `vpz1` (or `par2z`), `jsonl`.
- `--input` accepts either a single file or a directory (files are sorted chronologically by first event timestamp).
- Note on `PAR2`/`VPZ1`: `binance_mid` is absent from the `PAR2` binary layout (`None`), so `CrossExchangeShock` cleanly evaluates as `N = 0` (`InsufficientSample`).

### Track 2 — Mesoscopic Energy & Crowding Falsifier (`scan-meso`)

Evaluates 3 meso hypotheses (`VolCompressionBreakout`, `TsmomVolumeWeighted`, `FundingCrowdingReversal`) across 4 default horizons (`1b, 3b, 6b, 12b`, $M = 12$):

```powershell
cargo run --release -- scan-meso --format <csv|vkl1> --input <path> [--slippage-bps <f64>] [--out <report.json>]
```

- Accepted `--format` values: `csv`, `vkl1`.
- `--slippage-bps` defaults to `1.5` bps per leg (`>= 0.0`).

### Track 3 — Cross-Sectional Portfolio Falsifier (`scan-xs`)

Evaluates 3 cross-sectional factors (`XsFundingCarry`, `XsResidualReversion`, `XsMomentumVolAdj`) across 3 default horizons (`6b, 18b, 42b`) and configurable `--top-k` dollar-neutral baskets (`1x` gross `2L = 1.0`, `VIP0-Taker` binding vs. `VIP0-MakerDiag` non-binding):

```powershell
cargo run --release -- scan-xs --format <csv|vkl1> --input <path> [--top-k <usize[,usize...]>] [--slippage-bps <f64>] [--include-btc] [--out <report.json>]
```

- Accepted `--format` values: `csv`, `vkl1`.
- `--top-k` defaults to `3`; comma-separated lists (e.g., `1,2,3`) evaluate a unified global batch across all $K \times 3 \times 3$ cells ($M = 27$).
- `--slippage-bps` defaults to `1.5` bps per unit of one-way turnover (`>= 0.0`).
- By default excludes benchmark `SymbolId(0)` (`BTCUSDT`) from traded legs while retaining it for OLS market-beta estimation; `--include-btc` disables this exclusion.

### Track 4 — Market Making Avellaneda-Stoikov Falsifier (`scan-mm`)

Evaluates $M = 6$ Avellaneda-Stoikov configurations in homogeneous `[bps]` space (`AS_Base_Tight`, `AS_Base_Wide`, `AS_Skew_Tight`, `AS_Skew_Mid`, `AS_Skew_Wide`, `AS_Skew_Deep`) over a conservative FIFO L1 queue simulator (`5 ms` RTT, `100 ms` min order life, `0.5 bps` requote threshold, strict PostOnly), non-overlapping `60s` MtM buckets, mandatory EOF liquidation at `VIP-0 Taker` (`5.50 bps`), and a 5-profile fee frontier (`VIP0_Maker` `+2.00 bps` binding vs. `Venue_1bp`, `Venue_0bp`, `Rebate_05`, `Rebate_10` non-binding):

```powershell
cargo run --release -- scan-mm --format <par2|vpz1|jsonl> --input <path> [--capital-usdt <f64>] [--clip-usdt <f64>] [--max-inv-clips <f64>] [--out <report.json>]
```

- Accepted `--format` values: `par2`, `vpz1` (or `par2z`), `jsonl` (single file or directory sorted chronologically).
- Defaults: `--capital-usdt 2000`, `--clip-usdt 100`, `--max-inv-clips 4` (`max_gross_leverage = 2.0`, `bucket = 60s`, `rtt = 5.0ms`).

### Track 5 — Macro Daily (`1d`) & Spot–Perp Carry Falsifier (`scan-macro`)

Evaluates $M = 9$ daily strategies across 3 families (`VolTargetTrend`: `MacroTrend_LS_Vol15`, `MacroTrend_LS_Vol30`, `MacroTrend_LO_Vol20`; `DonchianBreakout`: `Donchian_20_10_LS`, `Donchian_55_20_LS`, `Donchian_55_20_LO`; `SpotPerpCarry`: `Carry_SpotPerp_10Apr`, `Carry_SpotPerp_15Apr`, `Carry_SpotPerp_TopCarry30d`) alongside 2 passive Spot benchmarks (`Baseline_BuyHold_BTC`, `Baseline_BuyHold_EW`) over aligned `linear` and `spot` `1d` CSV directories:

```powershell
cargo run --release -- scan-macro --linear-dir <dir> --spot-dir <dir> [--slippage-bps <f64>] [--out <report.json>]
```

- Requires both `--linear-dir` and `--spot-dir` (CSV directories read via `CsvCandleReader`).
- `--slippage-bps` defaults to `1.5` bps per leg (`>= 0.0`): Perp-only strategies pay `5.50 + slip` (`7.00 bps`/turn, vs. `2.00 bps` `VIP0-MakerDiag`); dual `SpotPerpCarry` pays `(10.00 + slip) + (5.50 + slip)` (`18.50 bps`/`turn_L`, vs. `12.00 bps` `VIP0-MakerDiag`); Spot passive benchmarks pay `10.00 + slip` (`11.50 bps`/turn, vs. `10.00 bps` `VIP0-MakerDiag`).

---

## 4. Phase 0 Verification Status

All 6 mandatory PowerShell 7 quality gates passed on the Phase 0 baseline codebase (`22/22` integration tests across `adversarial_math_tests`, `zero_lookahead_tests`, and `edge_cases_and_drift_tests`):

1. `cargo fmt --all -- --check` — **PASS**
2. `cargo clippy --all-targets --all-features -- -D warnings` — **PASS**
3. `cargo test --all-targets -- --nocapture` — **PASS** (`9 + 8 + 5 = 22` tests passed, `0` failed)
4. File size gate (`src/**/*.rs <= 350` lines) — **PASS** (max `348` lines in `src/domain/stats.rs`)
5. Banned patterns in `src/**/*.rs` (`.unwrap(`, `.expect(`, `panic!(`, `unsafe`, `todo!(`, `unimplemented!(`) — **PASS** (0 matches)
6. Encoding hygiene (`UTF-8 without BOM` + `LF-only` across `src/` and `tests/`) — **PASS**

Full unedited tool outputs and line-by-line forensic proofs are recorded in [`PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md).

---

## 5. Phase 1A Verification Status (Track 2 — 24m Bybit Empirical Falsification)

- **Window & Universe**: 24 exact UTC months (`2024-11-25T00:00:00Z` to `2026-11-25T00:00:00Z`), 12 Bybit V5 linear perpetuals (`BTCUSDT..LTCUSDT`), `15m` / `1h` / `4h` + real `funding/history` (`0` gaps, `0` reconciliation diff).
- **Binding Sweep (`VIP-0 + 1.5 bps` slip = `14.0 bps` RT)**: **`0 / 36` `Pass`** (`29 Fail`, `7 InsufficientSample`, across `M=12` and global `M=36`). Track 2 with the 3 baseline signals is empirically falsified; details in [`PHASE_1A_TRACK2_FALSIFICATION_REPORT.md`](./PHASE_1A_TRACK2_FALSIFICATION_REPORT.md).

---

## 6. Phase 1B Verification Status (Track 1 — `20.52M` Real Events on `sniper-tokio`)

- **Host & Telemetry**: `sniper-tokio` (`rustc 1.99.1`, `22/22` tests ok, `--release`, `CARGO_BUILD_JOBS=1`, read-only on `/home/ubuntu/vortex-rs/data`).
  - **Run 1 (`.par2z` / `VPZ1`)**: 12 hourly files (`2026-11-23 13h` to `2026-11-24 00h`, `~2.07 GB` compressed), `19,469,568` events, `Elapsed = 1:35.68` (`~203,486 evt/s`), `Max RSS = 7,048 KB (6.88 MiB)`, `Swaps = 0`, `Exit = 0`.
  - **Run 2 (`.parquet` / `PAR2`)**: `4,096` segments × `256` rows = `1,048,576` events, `Elapsed = 11.39s`, `Max RSS = 4,252 KB (4.15 MiB)`, `Swaps = 0`, `Exit = 0`.
- **Binding Sweep (`VIP-0` `Ask -> Bid`, `11.00 bps` RT, `M = 20`)**: **`0 / 20` `Pass`** across both datasets (`0.00` to `-3.84 bps` gross before fees on `HawkesTail` and `ShiryaevRobertsJump` with all `DiagZero` cells also negative; `LiquidationCascade` `N <= 5`; `CrossExchangeShock` intentionally `N = 0` due to `binance_mid = None` in `PAR2`). Details in [`PHASE_1B_TRACK1_FALSIFICATION_REPORT.md`](./PHASE_1B_TRACK1_FALSIFICATION_REPORT.md).

---

## 7. Phase 2 Verification Status (Track 3 — 30-Pair `4h` 24m Cross-Sectional Falsification)

- **Window & Universe**: 24 exact UTC months (`2024-11-25T00:00:00Z` to `2026-11-25T00:00:00Z`), 30 Bybit V5 linear perpetuals (`0:BTCUSDT..29:ICPUSDT`), `4h` (`4,380` closed bars/sym, `100.0%` coverage, `0` gaps, `0` reconciliation diff via causal Option A half-open bucket aggregation of `128` sub-4h `INJUSDT` settlements; residual survivorship bias disclosed).
- **6 Mandatory PowerShell Gates (Track 3 Baseline)**:
  1. `cargo fmt --all -- --check` — **PASS** (`EXIT_CODE=0`)
  2. `cargo clippy --all-targets --all-features -- -D warnings` — **PASS** (`EXIT_CODE=0`)
  3. `cargo test --all-targets -- --nocapture` — **PASS** (`9 + 7 + 8 + 5 = 29` integration tests passed, `0` failed, `EXIT_CODE=0`)
  4. File size gate (`src/**/*.rs <= 350` lines) — **PASS** (`26/26` files `Pass`, max `348` lines in `src/domain/stats.rs`)
  5. Banned patterns in `src/**/*.rs` — **PASS** (`0` matches)
  6. Encoding hygiene (`UTF-8 without BOM` + `LF-only` across `src/` and `tests/`, `30/30` files) — **PASS**
- **Binding Sweep (`VIP0-Taker + 1.5 bps` slip = `7.0 bps`/turnover)**: **`0 / 27` `Pass`** (`10 Fail`, `17 InsufficientSample`, across `M = 9` and global `M = 27`; `0 / 27` in Y1/Y2 OOS and `0.5 / 3.0 bps` sensitivity; `0 / 27` reaching `t_NW >= 3.5` or `DSR >= 0.95` even under non-binding `VIP0-MakerDiag`). Details in [`PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md`](./PHASE_2_TRACK3_XS_FALSIFICATION_REPORT.md).

---

## 8. Phase 3A Verification Status (Track 4 — Market Making Design & Code Closure)

- **Design Scope**: Purely additive Hexagonal SRP modules (`market_making.rs`, `mm_markout.rs`, `mm_fifo_sim.rs`, `mm_accounting.rs`, `falsifier_mm.rs`, `cli_mm.rs`) implementing Stoikov L1 microprice, CKS L1 OFI ($\tau = 2\text{s}$), Trade Flow Imbalance ($\tau = 5\text{s}$), EWMA volatility ($\tau = 30\text{s}$), causal `tick_e8` GCD inference, homogeneous `[bps]`-space Avellaneda-Stoikov quoting (`M = 6`), conservative `5 ms` RTT FIFO L1 queue simulator, exact `i128` accounting (`Drift == 0`), mandatory EOF `5.50 bps` Taker liquidation, and a 5-fee frontier (`VIP0_Maker` `+2.00 bps` solely binding).
- **6 Mandatory PowerShell Gates (Track 4 Closure)**:
  1. `cargo fmt --all -- --check` — **PASS** (`EXIT_CODE=0`)
  2. `cargo clippy --all-targets --all-features -- -D warnings` — **PASS** (`EXIT_CODE=0`)
  3. `cargo test --all-targets -- --nocapture` — **PASS** (`9 + 7 + 8 + 6 + 5 = 35` integration tests passed, `0` failed, `EXIT_CODE=0`)
  4. File size gate (`src/**/*.rs <= 350` lines) — **PASS** (`32/32` files `Pass`, max `348` lines in `src/domain/stats.rs`)
  5. Banned patterns in `src/**/*.rs` — **PASS** (`0` matches)
  6. Encoding hygiene (`UTF-8 without BOM` + `LF-only` across `src/` and `tests/`, `37/37` files) — **PASS**
- **Specification**: Full `LaTeX -> archivo:linea` proofs and `sniper-tokio` sweep commands in [`PHASE_3A_TRACK4_MM_DESIGN_REPORT.md`](./PHASE_3A_TRACK4_MM_DESIGN_REPORT.md).

---

## 9. Phase 3B Verification Status (Track 4 — `20.52M` Real Events MM Empirical Falsification)

- **Host & Telemetry (`sniper-tokio`, `rustc 1.99.1`, `35/35` tests ok, `--release`, read-only `/home/ubuntu/vortex-rs/data`)**:
  - **Run 1 (`VPZ1`, `$2,000 / $100`)**: `19,469,568` events, `693` buckets (`60s`), `sr_std(VIP0) = 0.6635`, `Elapsed = 1:14.42`, `Max RSS = 3,420 KB (3.34 MiB)`, `Swaps = 0`, `Exit = 0`.
  - **Run 2 (`VPZ1`, `$1,000 / $50`)**: `Elapsed = 1:15.02`, `Max RSS = 3,436 KB`, `Swaps = 0`, `Exit = 0` — exact scale invariance (identical fills `31,508 .. 180`, exact 50% PnL `-$504.28 .. -$3.81`, identical `BrkMkr`, `t_nw`, `Drift = 0`).
  - **Run 3 (`PAR2`, `$2,000 / $100`)**: `1,048,576` events, `52` buckets (`60s`), `sr_std(VIP0) = 0.9294`, `Elapsed = 0:11.39`, `Max RSS = 3,772 KB`, `Swaps = 0`, `Exit = 0`.
  - **Run 4 (`PAR2`, `$1,000 / $50`)**: `Elapsed = 0:03.23`, `Max RSS = 4,072 KB`, `Swaps = 0`, `Exit = 0` — identical fills/indicators, exact 50% PnL (`-$33.02 .. -$0.26`), `Drift = 0`.
- **Binding Sweep (`M = 6`, `VIP0_Maker` `+2.00 bps` / EOF `5.50 bps`)**: **`0 / 6` `Pass`** and **`0 / 30` positive cells** across the 5-fee frontier (even under `-1.00 bps` rebate). Root causes: `93%` of damage occurs in `< 1s` (`MO1s = -1.12 bps`), OFI+TFI skew increased adverse in-flight fills by `+157.9%` (`1,134 -> 2,925`), VIP-0 breakeven gap is `+3.20 bps/leg` (`+6.40 bps` RT), and fills are dominated by `book_crossed`/`trade_through`. Camino B (passive L1 MM on Bybit VIP-0) is empirically falsified and closed; details in [`PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md`](./PHASE_3B_TRACK4_MM_FALSIFICATION_REPORT.md).

---

## 10. Phase 4 Verification Status (Track 5 — 5-Year `1d` Dual Linear+Spot Macro Falsification)

- **Window & Universe**: `5` exact UTC years (`2021-11-25T00:00:00Z` to `2026-11-25T00:00:00Z`, `1,826` daily bars; `R1 = 912d`, `R2 = 914d`), `12` Bybit V5 majors (`0:BTCUSDT..11:ATOMUSDT`) in dual `linear + spot` (`21,912` linear + `21,807` spot = `43,719` unique candles; `audit_report.json` `PASS` across `72` CSVs / `87,438` rows; real `BNBUSDT` Spot `2022-03-10` listing `94.2497%` with `105` leading missing bars and `0` synthetic prices; `44,184` sub-24h funding events including `SOLUSDT` `[2.0, 8.0]h` aggregated with `0` unmatched events and `0` diff; residual survivorship bias disclosed).
- **6 Mandatory PowerShell Gates (Current Codebase)**:
  1. `cargo fmt --all -- --check` — **PASS** (`FMT_EXIT=0`)
  2. `cargo clippy --all-targets --all-features -- -D warnings` — **PASS** (`CLIPPY_EXIT=0`)
  3. `cargo test --all-targets -- --nocapture` — **PASS** (`9 + 7 + 8 + 6 + 6 + 5 = 41` integration tests passed, `0` failed, `TEST_EXIT=0`)
  4. File size gate (`src/**/*.rs <= 350` lines) — **PASS** (`40/40` files `Pass`, max `348` lines in `src/domain/stats.rs`)
  5. Banned patterns in `src/**/*.rs` — **PASS** (`BANNED_MATCHES=0`)
  6. Encoding hygiene (`UTF-8 without BOM` + `LF-only` across `src/` and `tests/`, `46/46` files) — **PASS**
- **Binding Sweep (`M = 9`, `t_crit = 3.5000`, `VIP-0 + 1.5 bps` slip)**: **`1 / 9` `Pass`** — **`Carry_SpotPerp_TopCarry30d`** passes 5y (`N = 1,796 >= n_min = 199`, `mean = +0.811 bps/d`, `CAGR = 3.00%`, `AnnSR = 4.75`, `MaxDD = 0.55%`, `Calmar = 5.47`, `t_nw = 8.594`, `DSR = 0.9967`), `0.5 bps` (`DSR = 0.9990`), `3.0 bps` (`DSR = 0.9773`), and `R1` (`t_nw = 7.363`, `DSR = 0.9999`), while falling to `InsufficientSample` in `R2` (`0 / 9 Pass`: `mean = +0.3665 bps/d`, `t_nw = 3.695`, `N = 884 < n_min = 1,106`, `DSR = 0.3216` due to `-53.7%` funding compression); all 6 `VolTargetTrend` and `DonchianBreakout` variants score `0 / 6 Pass` even under non-binding `VIP0-MakerDiag`. Details in [`PHASE_4_TRACK5_MACRO_FALSIFICATION_REPORT.md`](./PHASE_4_TRACK5_MACRO_FALSIFICATION_REPORT.md).

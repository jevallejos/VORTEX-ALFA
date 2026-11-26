# VORTEX-ALFA — Documentation Index

> **Status**: Phase 0 Forensically Closed | **Architecture**: Hexagonal Clean DDD (`src/` <= 350 lines/file, 0 `unsafe`, 0 `unwrap`/`expect`/`panic!`)
> **Primary Audit Report**: [`PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md)

---

## 1. Document Registry

| Document | Purpose | Scope |
|---|---|---|
| [`INDEX.md`](./INDEX.md) | Navigable architecture map, CLI quickstart, and Phase 0 verification summary | Workspace entrypoint |
| [`PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md) | Forensic closure audit: `VORTEX-L2` read-only layout inspection, file/layer/crate inventory, `LaTeX -> archivo:linea` math traceability table, 4-D compliance matrix, raw unedited 6-gate PS7 outputs, and 22-test `input -> expected -> actual` vector catalog | Complete Phase 0 sign-off |

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
│       └── meso_energy.rs              # Track 2 engine: Garman-Klass vol, r_comp, TSMOM-VW, Z-funding Welford
├── application/                        # Causal multi-horizon falsification orchestrators (depend only on domain)
│   ├── mod.rs
│   ├── falsifier_micro.rs              # Track 1 orchestrator: RTT delay, cooldown, non-overlapping slots, VIP-0 + DiagZero dual eval
│   └── falsifier_meso.rs               # Track 2 orchestrator: bar t+1 open entry, BarOpenSettlement vs BarCloseSettlement funding
├── infrastructure/                     # Adapters implementing domain ports & binary codecs
│   ├── mod.rs
│   ├── binary_codec.rs                 # Pure-Rust FNV-1a, PAR2 (256B), VPZ1 (36B hdr, Stride-256 XOR-Delta + Zero-RLE), VKL1 (96B)
│   ├── tape_reader.rs                  # StreamingTapeReader (Par2 | Vpz1 | Jsonl) implementing MarketTapeReader
│   ├── vkl1_reader.rs                  # StreamingVkl1Reader (.vkl1 directory/file) implementing CandleSeriesReader
│   └── csv_candle_reader.rs            # CsvCandleReader (ts_ns|ts_ms, funding_rate_bps|funding_rate) implementing CandleSeriesReader
└── interfaces/                         # CLI presentation & JSON ReportSink adapter
    ├── mod.rs
    └── cli.rs                          # Subcommands scan-micro & scan-meso, ASCII summary table, JSON --out writer
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

---

## 4. Phase 0 Verification Status

All 6 mandatory PowerShell 7 quality gates pass on the clean Phase 0 codebase (`22/22` integration tests across `adversarial_math_tests`, `zero_lookahead_tests`, and `edge_cases_and_drift_tests`):

1. `cargo fmt --all -- --check` — **PASS**
2. `cargo clippy --all-targets --all-features -- -D warnings` — **PASS**
3. `cargo test --all-targets -- --nocapture` — **PASS** (`9 + 8 + 5 = 22` tests passed, `0` failed)
4. File size gate (`src/**/*.rs <= 350` lines) — **PASS** (max `348` lines in `src/domain/stats.rs`)
5. Banned patterns in `src/**/*.rs` (`.unwrap(`, `.expect(`, `panic!(`, `unsafe`, `todo!(`, `unimplemented!(`) — **PASS** (0 matches)
6. Encoding hygiene (`UTF-8 without BOM` + `LF-only` across `src/` and `tests/`) — **PASS**

Full unedited tool outputs and line-by-line forensic proofs are recorded in [`PHASE_0_CLOSURE_REPORT.md`](./PHASE_0_CLOSURE_REPORT.md).

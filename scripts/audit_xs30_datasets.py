#!/usr/bin/env python3
"""Audit Track 3 (30-pair 4h) Bybit CSV datasets and Decimal funding reconciliation.

Strict Law compliance: validates file integrity, OHLCV invariants, window bounds,
timestamp monotonicity, gaps/coverage, Y1/Y2 split partitioning, raw funding JSON
integrity, and exact Decimal funding reconciliation. Performs zero signal, fee,
Newey-West, or DSR computations.
"""

from __future__ import annotations

import argparse
import csv
from decimal import Decimal, InvalidOperation
import json
import math
from pathlib import Path
import sys

SYMBOLS: list[tuple[int, str]] = [
    (0, "BTCUSDT"),
    (1, "ETHUSDT"),
    (2, "SOLUSDT"),
    (3, "AVAXUSDT"),
    (4, "TAOUSDT"),
    (5, "SUIUSDT"),
    (6, "ORDIUSDT"),
    (7, "PENDLEUSDT"),
    (8, "WIFUSDT"),
    (9, "LINKUSDT"),
    (10, "NEARUSDT"),
    (11, "LTCUSDT"),
    (12, "BNBUSDT"),
    (13, "XRPUSDT"),
    (14, "DOGEUSDT"),
    (15, "ADAUSDT"),
    (16, "DOTUSDT"),
    (17, "ATOMUSDT"),
    (18, "UNIUSDT"),
    (19, "AAVEUSDT"),
    (20, "ARBUSDT"),
    (21, "OPUSDT"),
    (22, "APTUSDT"),
    (23, "INJUSDT"),
    (24, "TIAUSDT"),
    (25, "SEIUSDT"),
    (26, "FILUSDT"),
    (27, "BCHUSDT"),
    (28, "ETCUSDT"),
    (29, "ICPUSDT"),
]

TF_NAME = "4h"
INTERVAL_MS = 14_400_000
START_MS = 1732492800000  # 2024-11-25T00:00:00Z
SPLIT_MS = 1764028800000  # 2025-11-25T00:00:00Z
END_MS = 1795564800000    # 2026-11-25T00:00:00Z
EXPECTED_FULL_BARS = (END_MS - START_MS) // INTERVAL_MS  # 4380
EXPECTED_SPLIT_BARS = (SPLIT_MS - START_MS) // INTERVAL_MS  # 2190

EXPECTED_HEADER = [
    "symbol_id",
    "ts_ms",
    "open",
    "high",
    "low",
    "close",
    "volume",
    "funding_rate_bps",
]


def format_decimal(val: Decimal) -> str:
    if val == 0:
        return "0"
    s = format(val, "f")
    if "." in s:
        s = s.rstrip("0").rstrip(".")
    return "0" if s == "-0" else s


def audit_raw_funding(
    path: Path, expected_sym_id: int, expected_sym: str
) -> tuple[dict[int, Decimal], dict]:
    raw_bytes = path.read_bytes()
    has_bom = raw_bytes.startswith(b"\xef\xbb\xbf")
    has_crlf = b"\r\n" in raw_bytes

    payload = json.loads(raw_bytes.decode("utf-8"))
    events = payload.get("events", [])
    schema_ok = (
        payload.get("symbol_id") == expected_sym_id
        and payload.get("symbol") == expected_sym
        and payload.get("window_start_ms") == START_MS
        and payload.get("window_end_ms") == END_MS
        and isinstance(events, list)
        and payload.get("count") == len(events)
    )

    out_of_window_count = 0
    duplicate_ts_count = 0
    backward_ts_count = 0
    conversion_mismatch_count = 0
    prev_ts: int | None = None
    funding_by_ts: dict[int, Decimal] = {}

    for ev in events:
        ts_ms = int(ev["ts_ms"])
        if ts_ms < START_MS or ts_ms >= END_MS:
            out_of_window_count += 1
        if prev_ts is not None:
            if ts_ms == prev_ts:
                duplicate_ts_count += 1
            elif ts_ms < prev_ts:
                backward_ts_count += 1
        prev_ts = ts_ms
        try:
            rate_dec = Decimal(str(ev["funding_rate_decimal"]))
            rate_bps = Decimal(str(ev["funding_rate_bps"]))
            if rate_dec * Decimal("10000") != rate_bps:
                conversion_mismatch_count += 1
        except InvalidOperation:
            conversion_mismatch_count += 1
            rate_bps = Decimal("0")
        funding_by_ts[ts_ms] = rate_bps

    return funding_by_ts, {
        "file": str(path),
        "events": len(events),
        "schema_ok": schema_ok,
        "utf8_no_bom": not has_bom,
        "lf_only": not has_crlf,
        "out_of_window_count": out_of_window_count,
        "duplicate_ts_count": duplicate_ts_count,
        "backward_ts_count": backward_ts_count,
        "conversion_mismatch_count": conversion_mismatch_count,
    }


def build_expected_bucket_map(
    raw_map: dict[int, Decimal]
) -> tuple[dict[int, Decimal], int, list[tuple[int, int]]]:
    """Map raw funding events onto causal (bar_open_ts - 4h, bar_open_ts] buckets."""
    bucket_map: dict[int, Decimal] = {}
    subbar_aggregated_events = 0
    event_targets: list[tuple[int, int]] = []
    for f_ts, f_bps in raw_map.items():
        rem = (f_ts - START_MS) % INTERVAL_MS
        target_open_ts = f_ts if rem == 0 else f_ts + (INTERVAL_MS - rem)
        if rem != 0:
            subbar_aggregated_events += 1
        bucket_map[target_open_ts] = bucket_map.get(target_open_ts, Decimal("0")) + f_bps
        event_targets.append((f_ts, target_open_ts))
    return bucket_map, subbar_aggregated_events, event_targets


def audit_csv(
    path: Path,
    expected_sym_id: int,
    win_start: int,
    win_end: int,
) -> dict:
    raw_bytes = path.read_bytes()
    has_bom = raw_bytes.startswith(b"\xef\xbb\xbf")
    has_crlf = b"\r\n" in raw_bytes

    nan_inf_count = 0
    non_pos_price_count = 0
    invalid_ohlc_count = 0
    neg_vol_count = 0
    duplicate_ts_count = 0
    backward_ts_count = 0
    wrong_sym_id_count = 0
    out_of_window_count = 0
    misaligned_ts_count = 0
    gap_count = 0
    missing_bars_total = 0
    row_count = 0
    prev_ts: int | None = None
    first_ts: int | None = None
    last_ts: int | None = None
    funding_sum = Decimal("0")
    ts_set: set[int] = set()
    funding_by_ts: dict[int, Decimal] = {}

    with path.open("r", encoding="utf-8", newline="") as fh:
        reader = csv.reader(fh)
        header = next(reader, None)
        header_ok = header == EXPECTED_HEADER
        for row in reader:
            row_count += 1
            if len(row) != len(EXPECTED_HEADER):
                invalid_ohlc_count += 1
                continue
            sym_id = int(row[0])
            ts_ms = int(row[1])
            o, h, l, c, vol = (
                float(row[2]),
                float(row[3]),
                float(row[4]),
                float(row[5]),
                float(row[6]),
            )
            f_dec = Decimal(row[7])
            f_flt = float(f_dec)
            if sym_id != expected_sym_id:
                wrong_sym_id_count += 1
            if not all(math.isfinite(x) for x in (o, h, l, c, vol, f_flt)):
                nan_inf_count += 1
            if o <= 0.0 or h <= 0.0 or l <= 0.0 or c <= 0.0:
                non_pos_price_count += 1
            if h < max(o, c) or l > min(o, c) or h < l:
                invalid_ohlc_count += 1
            if vol < 0.0:
                neg_vol_count += 1
            if ts_ms < win_start or ts_ms + INTERVAL_MS > win_end:
                out_of_window_count += 1
            if (ts_ms - START_MS) % INTERVAL_MS != 0:
                misaligned_ts_count += 1
            if prev_ts is not None:
                if ts_ms == prev_ts:
                    duplicate_ts_count += 1
                elif ts_ms < prev_ts:
                    backward_ts_count += 1
                elif ts_ms - prev_ts > INTERVAL_MS:
                    gap_count += 1
                    missing_bars_total += ((ts_ms - prev_ts) // INTERVAL_MS) - 1
            if first_ts is None:
                first_ts = ts_ms
            last_ts = ts_ms
            prev_ts = ts_ms
            ts_set.add(ts_ms)
            funding_by_ts[ts_ms] = f_dec
            funding_sum += f_dec

    expected_bars = (win_end - win_start) // INTERVAL_MS
    leading_missing = (
        (first_ts - win_start) // INTERVAL_MS if first_ts is not None else expected_bars
    )
    trailing_missing = (
        ((win_end - INTERVAL_MS) - last_ts) // INTERVAL_MS if last_ts is not None else 0
    )

    return {
        "file": str(path),
        "rows": row_count,
        "expected_bars": expected_bars,
        "coverage_pct": round((row_count / expected_bars) * 100.0, 4) if expected_bars else 0.0,
        "header_ok": header_ok,
        "utf8_no_bom": not has_bom,
        "lf_only": not has_crlf,
        "nan_inf_count": nan_inf_count,
        "non_pos_price_count": non_pos_price_count,
        "invalid_ohlc_count": invalid_ohlc_count,
        "neg_vol_count": neg_vol_count,
        "duplicate_ts_count": duplicate_ts_count,
        "backward_ts_count": backward_ts_count,
        "wrong_sym_id_count": wrong_sym_id_count,
        "out_of_window_count": out_of_window_count,
        "misaligned_ts_count": misaligned_ts_count,
        "gap_count": gap_count,
        "missing_bars_total": missing_bars_total,
        "leading_missing_bars": leading_missing,
        "trailing_missing_bars": trailing_missing,
        "funding_sum_bps": format_decimal(funding_sum),
        "ts_set": ts_set,
        "funding_by_ts": funding_by_ts,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Audit Track 3 (30-pair 4h) CSV datasets.")
    parser.add_argument("--data-root", default="data_local", help="Root data directory")
    args = parser.parse_args()
    root = Path(args.data_root)

    datasets = [
        ("bybit_xs30_4h", START_MS, END_MS),
        ("bybit_xs30_4h_y1", START_MS, SPLIT_MS),
        ("bybit_xs30_4h_y2", SPLIT_MS, END_MS),
    ]

    totals = {
        "raw_funding_files_checked": 0,
        "raw_funding_failures": 0,
        "csv_files_checked": 0,
        "rows_checked": 0,
        "nan_inf_total": 0,
        "non_pos_price_total": 0,
        "invalid_ohlc_total": 0,
        "neg_vol_total": 0,
        "duplicate_ts_total": 0,
        "backward_ts_total": 0,
        "wrong_sym_id_total": 0,
        "out_of_window_total": 0,
        "misaligned_ts_total": 0,
        "header_failures": 0,
        "encoding_failures": 0,
        "gap_files_total": 0,
        "gap_count_total": 0,
        "missing_bars_total": 0,
        "incomplete_coverage_files": 0,
        "split_mismatch_total": 0,
        "per_bar_funding_mismatch_total": 0,
        "subbar_aggregated_events_total": 0,
        "unmatched_funding_events_total": 0,
        "funding_reconciliation_failures": 0,
    }

    raw_funding_by_sym: dict[str, dict[int, Decimal]] = {}
    expected_buckets_by_sym: dict[str, tuple[dict[int, Decimal], int, list[tuple[int, int]]]] = {}
    raw_funding_audit: list[dict] = []
    for sym_id, sym in SYMBOLS:
        raw_path = root / "bybit_xs30_4h" / "raw_funding" / f"{sym}.json"
        if not raw_path.exists():
            print(f"ERROR: missing raw funding file {raw_path}", file=sys.stderr)
            return 2
        f_map, f_audit = audit_raw_funding(raw_path, sym_id, sym)
        raw_funding_by_sym[sym] = f_map
        expected_buckets_by_sym[sym] = build_expected_bucket_map(f_map)
        raw_funding_audit.append({"symbol_id": sym_id, "symbol": sym, **f_audit})
        totals["raw_funding_files_checked"] += 1
        if (
            not f_audit["schema_ok"]
            or not (f_audit["utf8_no_bom"] and f_audit["lf_only"])
            or f_audit["out_of_window_count"] > 0
            or f_audit["duplicate_ts_count"] > 0
            or f_audit["backward_ts_count"] > 0
            or f_audit["conversion_mismatch_count"] > 0
        ):
            totals["raw_funding_failures"] += 1

    per_ds_results: dict[tuple[str, str], dict] = {}
    reconciliation_rows: list[dict] = []
    coverage_rows: list[dict] = []

    for ds_name, w_start, w_end in datasets:
        for sym_id, sym in SYMBOLS:
            csv_path = root / ds_name / f"{sym}.csv"
            if not csv_path.exists():
                print(f"ERROR: missing dataset file {csv_path}", file=sys.stderr)
                return 2
            res = audit_csv(csv_path, sym_id, w_start, w_end)
            per_ds_results[(ds_name, sym)] = res

            totals["csv_files_checked"] += 1
            totals["rows_checked"] += res["rows"]
            totals["nan_inf_total"] += res["nan_inf_count"]
            totals["non_pos_price_total"] += res["non_pos_price_count"]
            totals["invalid_ohlc_total"] += res["invalid_ohlc_count"]
            totals["neg_vol_total"] += res["neg_vol_count"]
            totals["duplicate_ts_total"] += res["duplicate_ts_count"]
            totals["backward_ts_total"] += res["backward_ts_count"]
            totals["wrong_sym_id_total"] += res["wrong_sym_id_count"]
            totals["out_of_window_total"] += res["out_of_window_count"]
            totals["misaligned_ts_total"] += res["misaligned_ts_count"]
            if not res["header_ok"]:
                totals["header_failures"] += 1
            if not (res["utf8_no_bom"] and res["lf_only"]):
                totals["encoding_failures"] += 1
            if res["gap_count"] > 0:
                totals["gap_files_total"] += 1
                totals["gap_count_total"] += res["gap_count"]
                totals["missing_bars_total"] += res["missing_bars_total"]
            if res["rows"] != res["expected_bars"]:
                totals["incomplete_coverage_files"] += 1

            raw_map = raw_funding_by_sym[sym]
            bucket_map, subbar_aggregated_events, event_targets = expected_buckets_by_sym[sym]
            window_expected = {
                ts: bps for ts, bps in bucket_map.items() if w_start <= ts < w_end
            }
            per_bar_mismatch = sum(
                1
                for ts, csv_bps in res["funding_by_ts"].items()
                if csv_bps != window_expected.get(ts, Decimal("0"))
            )
            missing_bucket_bars = sum(
                1 for ts in window_expected if ts not in res["ts_set"]
            )
            totals["per_bar_funding_mismatch_total"] += per_bar_mismatch + missing_bucket_bars

            if ds_name == "bybit_xs30_4h":
                raw_sum = sum(raw_map.values(), Decimal("0"))
                csv_sum = Decimal(res["funding_sum_bps"])
                diff_bps = raw_sum - csv_sum
                unmatched_events = [
                    f_ts for f_ts, target_ts in event_targets if target_ts not in res["ts_set"]
                ]
                aligned_funding_buckets = sum(
                    1 for ts in res["ts_set"] if ts in bucket_map
                )
                totals["subbar_aggregated_events_total"] += subbar_aggregated_events
                totals["unmatched_funding_events_total"] += len(unmatched_events)

                if (
                    diff_bps != 0
                    or len(unmatched_events) != 0
                    or per_bar_mismatch != 0
                    or missing_bucket_bars != 0
                ):
                    totals["funding_reconciliation_failures"] += 1

                reconciliation_rows.append(
                    {
                        "symbol_id": sym_id,
                        "symbol": sym,
                        "rows": res["rows"],
                        "raw_funding_events": len(raw_map),
                        "subbar_aggregated_events": subbar_aggregated_events,
                        "aligned_funding_buckets": aligned_funding_buckets,
                        "unmatched_funding_events": len(unmatched_events),
                        "per_bar_mismatch_count": per_bar_mismatch + missing_bucket_bars,
                        "raw_sum_bps": format_decimal(raw_sum),
                        "csv_sum_bps": format_decimal(csv_sum),
                        "diff_bps": format_decimal(diff_bps),
                    }
                )
                coverage_rows.append(
                    {
                        "symbol_id": sym_id,
                        "symbol": sym,
                        "full_rows": res["rows"],
                        "expected_bars": EXPECTED_FULL_BARS,
                        "coverage_pct": res["coverage_pct"],
                        "gap_count": res["gap_count"],
                        "missing_bars_total": res["missing_bars_total"],
                        "leading_missing_bars": res["leading_missing_bars"],
                        "trailing_missing_bars": res["trailing_missing_bars"],
                    }
                )

    for sym_id, sym in SYMBOLS:
        full_r = per_ds_results[("bybit_xs30_4h", sym)]
        y1_r = per_ds_results[("bybit_xs30_4h_y1", sym)]
        y2_r = per_ds_results[("bybit_xs30_4h_y2", sym)]
        counts_ok = (y1_r["rows"] + y2_r["rows"]) == full_r["rows"]
        sets_ok = (
            y1_r["ts_set"].isdisjoint(y2_r["ts_set"])
            and (y1_r["ts_set"] | y2_r["ts_set"]) == full_r["ts_set"]
        )
        if not (counts_ok and sets_ok):
            totals["split_mismatch_total"] += 1

    hard_errors = (
        totals["raw_funding_failures"]
        + totals["nan_inf_total"]
        + totals["non_pos_price_total"]
        + totals["invalid_ohlc_total"]
        + totals["neg_vol_total"]
        + totals["duplicate_ts_total"]
        + totals["backward_ts_total"]
        + totals["wrong_sym_id_total"]
        + totals["out_of_window_total"]
        + totals["misaligned_ts_total"]
        + totals["header_failures"]
        + totals["encoding_failures"]
        + totals["split_mismatch_total"]
        + totals["per_bar_funding_mismatch_total"]
        + totals["unmatched_funding_events_total"]
        + totals["funding_reconciliation_failures"]
    )

    status = "PASS" if hard_errors == 0 else "FAIL"
    report = {
        "status": status,
        "funding_alignment": "BarOpenSettlement_CausalHalfOpen_(bar_open_ts-4h,bar_open_ts]",
        "totals": totals,
        "coverage": coverage_rows,
        "funding_reconciliation": reconciliation_rows,
        "raw_funding_audit": raw_funding_audit,
    }
    out_path = root / "bybit_xs30_4h" / "audit_report.json"
    with out_path.open("w", encoding="utf-8", newline="\n") as fh:
        json.dump(report, fh, indent=2)
        fh.write("\n")

    print(json.dumps({"status": status, "totals": totals}, indent=2))
    gap_entries = [c for c in coverage_rows if c["gap_count"] > 0 or c["full_rows"] != EXPECTED_FULL_BARS]
    print(f"Coverage / gap anomalies: {len(gap_entries)}")
    for item in gap_entries:
        print(
            f"  [{item['symbol_id']:02d}] {item['symbol']:10s}: rows={item['full_rows']}/{item['expected_bars']} "
            f"({item['coverage_pct']}%), gaps={item['gap_count']}, missing={item['missing_bars_total']}"
        )

    nonzero_diffs = [
        r
        for r in reconciliation_rows
        if Decimal(r["diff_bps"]) != 0
        or r["unmatched_funding_events"] != 0
        or r["per_bar_mismatch_count"] != 0
    ]
    print(f"Non-zero funding reconciliation entries: {len(nonzero_diffs)}")
    for item in nonzero_diffs:
        print(
            f"  [{item['symbol_id']:02d}] {item['symbol']:10s}: "
            f"unmatched={item['unmatched_funding_events']}, "
            f"per_bar_mismatch={item['per_bar_mismatch_count']}, "
            f"raw_sum={item['raw_sum_bps']}, csv_sum={item['csv_sum_bps']}, diff={item['diff_bps']} bps"
        )
    subbar_entries = [r for r in reconciliation_rows if r["subbar_aggregated_events"] > 0]
    print(f"Symbols with causal sub-4h bucket aggregation: {len(subbar_entries)}")
    for item in subbar_entries:
        print(
            f"  [{item['symbol_id']:02d}] {item['symbol']:10s}: "
            f"subbar_aggregated_events={item['subbar_aggregated_events']}, "
            f"raw_sum={item['raw_sum_bps']}, csv_sum={item['csv_sum_bps']}, diff={item['diff_bps']} bps"
        )

    return 0 if hard_errors == 0 else 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Audit Track 5 (5-year 1d dual linear+spot) Bybit CSV datasets and Decimal funding reconciliation.

Strict Law compliance:
- Pure Python stdlib only.
- Validates encoding (UTF-8 no BOM, LF-only), headers, OHLCV invariants, window bounds,
  timestamp monotonicity, interior gaps, R1/R2 split partitioning, spot zero-funding,
  raw funding JSON integrity, and exact Decimal funding reconciliation (Law 1 & Law 4).
- Leading missing bars from actual listing dates (e.g. BNBUSDT spot 2022-03-10) are logged
  in coverage without failing the audit, whereas interior gaps or trailing missing bars fail.
"""

from __future__ import annotations

import argparse
import csv
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
import json
import math
from pathlib import Path
import sys

SYMBOLS: list[tuple[int, str]] = [
    (0, "BTCUSDT"),
    (1, "ETHUSDT"),
    (2, "SOLUSDT"),
    (3, "BNBUSDT"),
    (4, "XRPUSDT"),
    (5, "ADAUSDT"),
    (6, "DOGEUSDT"),
    (7, "LINKUSDT"),
    (8, "AVAXUSDT"),
    (9, "LTCUSDT"),
    (10, "DOTUSDT"),
    (11, "ATOMUSDT"),
]

TF_NAME = "1d"
INTERVAL_MS = 86_400_000
START_MS = 1637798400000  # 2021-11-25T00:00:00Z
SPLIT_MS = 1716595200000  # 2024-05-25T00:00:00Z
END_MS = 1795564800000    # 2026-11-25T00:00:00Z
EXPECTED_FULL_BARS = (END_MS - START_MS) // INTERVAL_MS  # 1826
EXPECTED_R1_BARS = (SPLIT_MS - START_MS) // INTERVAL_MS  # 912
EXPECTED_R2_BARS = (END_MS - SPLIT_MS) // INTERVAL_MS    # 914

FUND_START_MS = START_MS - INTERVAL_MS + 1  # 1637712000001
FUND_END_MS = END_MS - INTERVAL_MS          # 1795478400000

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


def ms_to_iso(ts_ms: int | None) -> str | None:
    if ts_ms is None:
        return None
    return datetime.fromtimestamp(ts_ms / 1000.0, tz=timezone.utc).strftime(
        "%Y-%m-%dT%H:%M:%SZ"
    )


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
    has_cr = b"\r" in raw_bytes

    decode_ok = True
    try:
        payload = json.loads(raw_bytes.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        decode_ok = False
        payload = {}

    events = payload.get("events", []) if isinstance(payload, dict) else []
    schema_ok = (
        decode_ok
        and isinstance(payload, dict)
        and payload.get("symbol_id") == expected_sym_id
        and payload.get("symbol") == expected_sym
        and payload.get("window_start_ms") == START_MS
        and payload.get("window_end_ms") == END_MS
        and payload.get("funding_start_ms") == FUND_START_MS
        and payload.get("funding_end_ms") == FUND_END_MS
        and isinstance(events, list)
        and len(events) > 0
        and payload.get("count") == len(events)
    )

    out_of_window_count = 0
    duplicate_ts_count = 0
    backward_ts_count = 0
    conversion_mismatch_count = 0
    nan_inf_count = 0
    prev_ts: int | None = None
    funding_by_ts: dict[int, Decimal] = {}

    if isinstance(events, list):
        for ev in events:
            try:
                ts_ms = int(ev["ts_ms"])
            except (KeyError, TypeError, ValueError):
                conversion_mismatch_count += 1
                continue
            if ts_ms < FUND_START_MS or ts_ms > FUND_END_MS:
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
                if not (math.isfinite(float(rate_dec)) and math.isfinite(float(rate_bps))):
                    nan_inf_count += 1
                if rate_dec * Decimal("10000") != rate_bps:
                    conversion_mismatch_count += 1
            except (KeyError, TypeError, ValueError, InvalidOperation):
                conversion_mismatch_count += 1
                rate_bps = Decimal("0")
            funding_by_ts[ts_ms] = rate_bps

    return funding_by_ts, {
        "file": str(path),
        "events": len(events) if isinstance(events, list) else 0,
        "schema_ok": schema_ok,
        "utf8_no_bom": decode_ok and not has_bom,
        "lf_only": not has_cr,
        "nan_inf_count": nan_inf_count,
        "out_of_window_count": out_of_window_count,
        "duplicate_ts_count": duplicate_ts_count,
        "backward_ts_count": backward_ts_count,
        "conversion_mismatch_count": conversion_mismatch_count,
    }


def build_expected_bucket_map(
    raw_map: dict[int, Decimal]
) -> tuple[dict[int, Decimal], int, list[tuple[int, int]]]:
    """Map raw funding events onto causal half-open (bar_open_ts - 24h, bar_open_ts] buckets."""
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
    category: str,
    win_start: int,
    win_end: int,
) -> dict:
    raw_bytes = path.read_bytes()
    has_bom = raw_bytes.startswith(b"\xef\xbb\xbf")
    has_cr = b"\r" in raw_bytes

    nan_inf_count = 0
    non_pos_price_count = 0
    invalid_ohlc_count = 0
    neg_vol_count = 0
    duplicate_ts_count = 0
    backward_ts_count = 0
    wrong_sym_id_count = 0
    out_of_window_count = 0
    misaligned_ts_count = 0
    nonzero_spot_funding_count = 0
    gap_count = 0
    missing_bars_total = 0
    row_count = 0
    prev_ts: int | None = None
    first_ts: int | None = None
    last_ts: int | None = None
    funding_sum = Decimal("0")
    ts_set: set[int] = set()
    funding_by_ts: dict[int, Decimal] = {}
    rows_by_ts: dict[int, tuple[str, ...]] = {}
    decode_ok = True
    header_ok = False

    try:
        text = raw_bytes.decode("utf-8")
    except UnicodeDecodeError:
        decode_ok = False
        text = ""

    if decode_ok:
        lines = text.splitlines()
        reader = csv.reader(lines)
        header = next(reader, None)
        header_ok = header == EXPECTED_HEADER
        for row in reader:
            row_count += 1
            if len(row) != len(EXPECTED_HEADER):
                invalid_ohlc_count += 1
                continue
            try:
                sym_id = int(row[0])
                ts_ms = int(row[1])
                o_dec, h_dec, l_dec, c_dec, v_dec = (
                    Decimal(row[2]),
                    Decimal(row[3]),
                    Decimal(row[4]),
                    Decimal(row[5]),
                    Decimal(row[6]),
                )
                f_dec = Decimal(row[7])
                o, h, l, c, vol, f_flt = (
                    float(o_dec),
                    float(h_dec),
                    float(l_dec),
                    float(c_dec),
                    float(v_dec),
                    float(f_dec),
                )
            except (ValueError, InvalidOperation):
                invalid_ohlc_count += 1
                continue

            if sym_id != expected_sym_id:
                wrong_sym_id_count += 1
            if not all(math.isfinite(x) for x in (o, h, l, c, vol, f_flt)):
                nan_inf_count += 1
            if (
                o <= 0.0
                or h <= 0.0
                or l <= 0.0
                or c <= 0.0
                or round(o * 1e8) < 1
                or round(h * 1e8) < 1
                or round(l * 1e8) < 1
                or round(c * 1e8) < 1
            ):
                non_pos_price_count += 1
            if h_dec < max(o_dec, c_dec) or l_dec > min(o_dec, c_dec) or h_dec < l_dec:
                invalid_ohlc_count += 1
            if vol < 0.0:
                neg_vol_count += 1
            if category == "spot" and f_dec != Decimal("0"):
                nonzero_spot_funding_count += 1
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
            rows_by_ts[ts_ms] = tuple(row)
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
        "min_ts_ms": first_ts,
        "min_utc": ms_to_iso(first_ts),
        "max_ts_ms": last_ts,
        "max_utc": ms_to_iso(last_ts),
        "header_ok": header_ok,
        "utf8_no_bom": decode_ok and not has_bom,
        "lf_only": not has_cr,
        "nan_inf_count": nan_inf_count,
        "non_pos_price_count": non_pos_price_count,
        "invalid_ohlc_count": invalid_ohlc_count,
        "neg_vol_count": neg_vol_count,
        "nonzero_spot_funding_count": nonzero_spot_funding_count,
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
        "rows_by_ts": rows_by_ts,
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Audit Track 5 (5y 1d linear+spot) CSV datasets and Decimal funding reconciliation."
    )
    parser.add_argument("--data-root", default="data_local", help="Root data directory")
    args = parser.parse_args()

    assert int(datetime(2021, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == START_MS
    assert int(datetime(2024, 5, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == SPLIT_MS
    assert int(datetime(2026, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == END_MS
    assert EXPECTED_FULL_BARS == 1826
    assert EXPECTED_R1_BARS == 912
    assert EXPECTED_R2_BARS == 914

    root = Path(args.data_root)
    datasets = [
        ("bybit_macro5y_1d", START_MS, END_MS),
        ("bybit_macro5y_r1", START_MS, SPLIT_MS),
        ("bybit_macro5y_r2", SPLIT_MS, END_MS),
    ]
    categories = ("linear", "spot")

    totals = {
        "raw_funding_files_checked": 0,
        "raw_funding_failures": 0,
        "csv_files_checked": 0,
        "missing_files_total": 0,
        "rows_checked": 0,
        "nan_inf_total": 0,
        "non_pos_price_total": 0,
        "invalid_ohlc_total": 0,
        "neg_vol_total": 0,
        "nonzero_spot_funding_total": 0,
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
        "trailing_missing_bars_total": 0,
        "leading_incomplete_files": 0,
        "split_mismatch_total": 0,
        "per_bar_funding_mismatch_total": 0,
        "subbar_aggregated_events_total": 0,
        "unmatched_funding_events_total": 0,
        "funding_reconciliation_failures": 0,
    }

    manifest_path = root / "bybit_macro5y_1d" / "download_manifest.json"
    if manifest_path.exists():
        m_bytes = manifest_path.read_bytes()
        try:
            m_text = m_bytes.decode("utf-8")
            json.loads(m_text)
            m_ok = not m_bytes.startswith(b"\xef\xbb\xbf") and b"\r" not in m_bytes
        except (UnicodeDecodeError, json.JSONDecodeError):
            m_ok = False
        if not m_ok:
            totals["encoding_failures"] += 1
    else:
        totals["missing_files_total"] += 1

    raw_funding_by_sym: dict[str, dict[int, Decimal]] = {}
    expected_buckets_by_sym: dict[str, tuple[dict[int, Decimal], int, list[tuple[int, int]]]] = {}
    raw_funding_audit: list[dict] = []

    for sym_id, sym in SYMBOLS:
        raw_path = root / "bybit_macro5y_1d" / "raw_funding" / f"{sym}.json"
        if not raw_path.exists():
            totals["missing_files_total"] += 1
            totals["raw_funding_failures"] += 1
            raw_funding_by_sym[sym] = {}
            expected_buckets_by_sym[sym] = ({}, 0, [])
            raw_funding_audit.append(
                {
                    "symbol_id": sym_id,
                    "symbol": sym,
                    "file": str(raw_path),
                    "missing": True,
                }
            )
            continue
        f_map, f_audit = audit_raw_funding(raw_path, sym_id, sym)
        raw_funding_by_sym[sym] = f_map
        expected_buckets_by_sym[sym] = build_expected_bucket_map(f_map)
        raw_funding_audit.append({"symbol_id": sym_id, "symbol": sym, **f_audit})
        totals["raw_funding_files_checked"] += 1
        if (
            not f_audit["schema_ok"]
            or not (f_audit["utf8_no_bom"] and f_audit["lf_only"])
            or f_audit["nan_inf_count"] > 0
            or f_audit["out_of_window_count"] > 0
            or f_audit["duplicate_ts_count"] > 0
            or f_audit["backward_ts_count"] > 0
            or f_audit["conversion_mismatch_count"] > 0
        ):
            totals["raw_funding_failures"] += 1

    per_ds_results: dict[tuple[str, str, str], dict] = {}
    reconciliation_rows: list[dict] = []

    for ds_name, w_start, w_end in datasets:
        for cat in categories:
            for sym_id, sym in SYMBOLS:
                csv_path = root / ds_name / cat / f"{sym}.csv"
                if not csv_path.exists():
                    totals["missing_files_total"] += 1
                    per_ds_results[(ds_name, cat, sym)] = {
                        "file": str(csv_path),
                        "missing": True,
                        "rows": 0,
                        "expected_bars": (w_end - w_start) // INTERVAL_MS,
                        "coverage_pct": 0.0,
                        "min_ts_ms": None,
                        "min_utc": None,
                        "max_ts_ms": None,
                        "max_utc": None,
                        "gap_count": 0,
                        "missing_bars_total": 0,
                        "leading_missing_bars": (w_end - w_start) // INTERVAL_MS,
                        "trailing_missing_bars": 0,
                        "funding_sum_bps": "0",
                        "ts_set": set(),
                        "funding_by_ts": {},
                        "rows_by_ts": {},
                    }
                    continue

                res = audit_csv(csv_path, sym_id, cat, w_start, w_end)
                per_ds_results[(ds_name, cat, sym)] = res

                totals["csv_files_checked"] += 1
                totals["rows_checked"] += res["rows"]
                totals["nan_inf_total"] += res["nan_inf_count"]
                totals["non_pos_price_total"] += res["non_pos_price_count"]
                totals["invalid_ohlc_total"] += res["invalid_ohlc_count"]
                totals["neg_vol_total"] += res["neg_vol_count"]
                totals["nonzero_spot_funding_total"] += res["nonzero_spot_funding_count"]
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
                if res["trailing_missing_bars"] > 0:
                    totals["trailing_missing_bars_total"] += res["trailing_missing_bars"]
                if res["leading_missing_bars"] > 0:
                    totals["leading_incomplete_files"] += 1

                if cat == "linear":
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

                    if ds_name == "bybit_macro5y_1d":
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

                        fund_ts_list = sorted(raw_map.keys())
                        observed_fund_intervals_h = sorted(
                            {
                                round((fund_ts_list[i] - fund_ts_list[i - 1]) / 3_600_000.0, 6)
                                for i in range(1, len(fund_ts_list))
                            }
                        )
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
                                "observed_funding_intervals_h": observed_fund_intervals_h,
                            }
                        )

    split_audit: list[dict] = []
    for cat in categories:
        for sym_id, sym in SYMBOLS:
            full_r = per_ds_results[("bybit_macro5y_1d", cat, sym)]
            r1_r = per_ds_results[("bybit_macro5y_r1", cat, sym)]
            r2_r = per_ds_results[("bybit_macro5y_r2", cat, sym)]

            counts_ok = (r1_r["rows"] + r2_r["rows"]) == full_r["rows"]
            disjoint_ok = r1_r["ts_set"].isdisjoint(r2_r["ts_set"])
            union_ok = (r1_r["ts_set"] | r2_r["ts_set"]) == full_r["ts_set"]
            rows_ok = (
                all(full_r["rows_by_ts"].get(ts) == row for ts, row in r1_r["rows_by_ts"].items())
                and all(full_r["rows_by_ts"].get(ts) == row for ts, row in r2_r["rows_by_ts"].items())
            )
            split_ok = (
                not full_r.get("missing")
                and not r1_r.get("missing")
                and not r2_r.get("missing")
                and counts_ok
                and disjoint_ok
                and union_ok
                and rows_ok
            )
            if not split_ok:
                totals["split_mismatch_total"] += 1
            split_audit.append(
                {
                    "symbol_id": sym_id,
                    "symbol": sym,
                    "category": cat,
                    "full_rows": full_r["rows"],
                    "r1_rows": r1_r["rows"],
                    "r2_rows": r2_r["rows"],
                    "split_ok": split_ok,
                }
            )

    coverage_rows: list[dict] = []
    for sym_id, sym in SYMBOLS:
        lin_full = per_ds_results[("bybit_macro5y_1d", "linear", sym)]
        lin_r1 = per_ds_results[("bybit_macro5y_r1", "linear", sym)]
        lin_r2 = per_ds_results[("bybit_macro5y_r2", "linear", sym)]
        spot_full = per_ds_results[("bybit_macro5y_1d", "spot", sym)]
        spot_r1 = per_ds_results[("bybit_macro5y_r1", "spot", sym)]
        spot_r2 = per_ds_results[("bybit_macro5y_r2", "spot", sym)]

        overlap_ts = sorted(lin_full["ts_set"] & spot_full["ts_set"])
        overlap_r1 = sum(1 for ts in overlap_ts if ts < SPLIT_MS)
        overlap_r2 = sum(1 for ts in overlap_ts if ts >= SPLIT_MS)

        coverage_rows.append(
            {
                "symbol_id": sym_id,
                "symbol": sym,
                "dual_overlap_days": len(overlap_ts),
                "dual_overlap_r1_days": overlap_r1,
                "dual_overlap_r2_days": overlap_r2,
                "dual_overlap_min_utc": ms_to_iso(overlap_ts[0]) if overlap_ts else None,
                "dual_overlap_max_utc": ms_to_iso(overlap_ts[-1]) if overlap_ts else None,
                "linear": {
                    "rows": lin_full["rows"],
                    "expected_bars": EXPECTED_FULL_BARS,
                    "coverage_pct": lin_full["coverage_pct"],
                    "r1_rows": lin_r1["rows"],
                    "r2_rows": lin_r2["rows"],
                    "min_utc": lin_full["min_utc"],
                    "max_utc": lin_full["max_utc"],
                    "leading_missing_bars": lin_full["leading_missing_bars"],
                    "trailing_missing_bars": lin_full["trailing_missing_bars"],
                    "gap_count": lin_full["gap_count"],
                    "missing_bars_total": lin_full["missing_bars_total"],
                },
                "spot": {
                    "rows": spot_full["rows"],
                    "expected_bars": EXPECTED_FULL_BARS,
                    "coverage_pct": spot_full["coverage_pct"],
                    "r1_rows": spot_r1["rows"],
                    "r2_rows": spot_r2["rows"],
                    "min_utc": spot_full["min_utc"],
                    "max_utc": spot_full["max_utc"],
                    "leading_missing_bars": spot_full["leading_missing_bars"],
                    "trailing_missing_bars": spot_full["trailing_missing_bars"],
                    "gap_count": spot_full["gap_count"],
                    "missing_bars_total": spot_full["missing_bars_total"],
                },
            }
        )

    hard_errors = (
        totals["missing_files_total"]
        + totals["raw_funding_failures"]
        + totals["nan_inf_total"]
        + totals["non_pos_price_total"]
        + totals["invalid_ohlc_total"]
        + totals["neg_vol_total"]
        + totals["nonzero_spot_funding_total"]
        + totals["duplicate_ts_total"]
        + totals["backward_ts_total"]
        + totals["wrong_sym_id_total"]
        + totals["out_of_window_total"]
        + totals["misaligned_ts_total"]
        + totals["header_failures"]
        + totals["encoding_failures"]
        + totals["gap_count_total"]
        + totals["trailing_missing_bars_total"]
        + totals["split_mismatch_total"]
        + totals["per_bar_funding_mismatch_total"]
        + totals["unmatched_funding_events_total"]
        + totals["funding_reconciliation_failures"]
    )

    status = "PASS" if hard_errors == 0 else "FAIL"
    report = {
        "status": status,
        "timeframe": TF_NAME,
        "interval_ms": INTERVAL_MS,
        "window_start_ms": START_MS,
        "window_start_utc": ms_to_iso(START_MS),
        "split_ms": SPLIT_MS,
        "split_utc": ms_to_iso(SPLIT_MS),
        "window_end_ms": END_MS,
        "window_end_utc": ms_to_iso(END_MS),
        "funding_alignment": "BarOpenSettlement_CausalHalfOpen_(bar_open_ts-24h,bar_open_ts]",
        "totals": totals,
        "coverage": coverage_rows,
        "funding_reconciliation": reconciliation_rows,
        "raw_funding_audit": raw_funding_audit,
        "split_audit": split_audit,
    }

    out_path = root / "bybit_macro5y_1d" / "audit_report.json"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = out_path.with_suffix(".json.tmp")
    with tmp_path.open("w", encoding="utf-8", newline="\n") as fh:
        json.dump(report, fh, indent=2)
        fh.write("\n")
    tmp_path.replace(out_path)

    print(json.dumps({"status": status, "totals": totals}, indent=2))

    coverage_anomalies = [
        c
        for c in coverage_rows
        if c["linear"]["rows"] != EXPECTED_FULL_BARS
        or c["spot"]["rows"] != EXPECTED_FULL_BARS
        or c["linear"]["gap_count"] > 0
        or c["spot"]["gap_count"] > 0
        or c["linear"]["trailing_missing_bars"] > 0
        or c["spot"]["trailing_missing_bars"] > 0
    ]
    print(f"Coverage / listing / gap notes: {len(coverage_anomalies)}")
    for item in coverage_anomalies:
        lin = item["linear"]
        spot = item["spot"]
        print(
            f"  [{item['symbol_id']:02d}] {item['symbol']:10s}: "
            f"lin={lin['rows']}/{lin['expected_bars']} ({lin['coverage_pct']}%, lead_miss={lin['leading_missing_bars']}, trail_miss={lin['trailing_missing_bars']}, gaps={lin['gap_count']}, {lin['min_utc']}..{lin['max_utc']}) | "
            f"spot={spot['rows']}/{spot['expected_bars']} ({spot['coverage_pct']}%, lead_miss={spot['leading_missing_bars']}, trail_miss={spot['trailing_missing_bars']}, gaps={spot['gap_count']}, {spot['min_utc']}..{spot['max_utc']}) | "
            f"overlap={item['dual_overlap_days']}d"
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

    return 0 if hard_errors == 0 else 1


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""Audit Track 6 (5-year 1d dual linear+spot, 30 pairs) Bybit CSV datasets and Decimal funding reconciliation.

Strict Law compliance:
- Pure Python stdlib only.
- Validates encoding (UTF-8 no BOM, LF-only), headers, OHLCV invariants, window bounds,
  timestamp monotonicity, interior gaps, R1/R2 and 24m split partitioning, spot zero-funding,
  manifest-confirmed spot absences (e.g. TAOUSDT header-only), raw funding JSON integrity
  with contract-life causal bounds, and exact Decimal funding reconciliation (Law 1 & Law 4).
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

TF_NAME = "1d"
INTERVAL_MS = 86_400_000
START_MS = 1637798400000     # 2021-11-25T00:00:00Z
SPLIT_MS = 1716595200000     # 2024-05-25T00:00:00Z
COHORT24_MS = 1732492800000  # 2024-11-25T00:00:00Z
END_MS = 1795564800000       # 2026-11-25T00:00:00Z

EXPECTED_FULL_BARS = (END_MS - START_MS) // INTERVAL_MS       # 1826
EXPECTED_R1_BARS = (SPLIT_MS - START_MS) // INTERVAL_MS       # 912
EXPECTED_R2_BARS = (END_MS - SPLIT_MS) // INTERVAL_MS         # 914
EXPECTED_24M_BARS = (END_MS - COHORT24_MS) // INTERVAL_MS     # 730

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
    pre_events = payload.get("pre_listing_filtered_events", []) if isinstance(payload, dict) else []
    first_lin_ms = payload.get("first_linear_open_ms") if isinstance(payload, dict) else None
    eff_fund_start_ms = payload.get("effective_funding_start_ms") if isinstance(payload, dict) else None

    expected_eff_start: int | None = None
    if isinstance(first_lin_ms, int):
        expected_eff_start = first_lin_ms if first_lin_ms > START_MS else FUND_START_MS

    schema_ok = (
        decode_ok
        and isinstance(payload, dict)
        and payload.get("symbol_id") == expected_sym_id
        and payload.get("symbol") == expected_sym
        and payload.get("window_start_ms") == START_MS
        and payload.get("window_end_ms") == END_MS
        and payload.get("funding_start_ms") == FUND_START_MS
        and payload.get("funding_end_ms") == FUND_END_MS
        and isinstance(first_lin_ms, int)
        and expected_eff_start is not None
        and eff_fund_start_ms == expected_eff_start
        and isinstance(events, list)
        and len(events) > 0
        and payload.get("count") == len(events)
        and isinstance(pre_events, list)
        and payload.get("pre_listing_filtered_count") == len(pre_events)
        and "pre_listing_filtered_sum_bps" in payload
    )

    out_of_window_count = 0
    duplicate_ts_count = 0
    backward_ts_count = 0
    conversion_mismatch_count = 0
    nan_inf_count = 0
    prev_ts: int | None = None
    funding_by_ts: dict[int, Decimal] = {}

    eff_lb = eff_fund_start_ms if isinstance(eff_fund_start_ms, int) else FUND_START_MS

    if isinstance(events, list):
        for ev in events:
            try:
                ts_ms = int(ev["ts_ms"])
            except (KeyError, TypeError, ValueError):
                conversion_mismatch_count += 1
                continue
            if ts_ms < eff_lb or ts_ms > FUND_END_MS:
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

    pre_sum_calc = Decimal("0")
    pre_prev_ts: int | None = None
    if isinstance(pre_events, list):
        for pev in pre_events:
            try:
                p_ts = int(pev["ts_ms"])
            except (KeyError, TypeError, ValueError):
                conversion_mismatch_count += 1
                continue
            if p_ts < FUND_START_MS or p_ts >= eff_lb:
                out_of_window_count += 1
            if pre_prev_ts is not None:
                if p_ts == pre_prev_ts:
                    duplicate_ts_count += 1
                elif p_ts < pre_prev_ts:
                    backward_ts_count += 1
            pre_prev_ts = p_ts
            try:
                p_dec = Decimal(str(pev["funding_rate_decimal"]))
                p_bps = Decimal(str(pev["funding_rate_bps"]))
                if not (math.isfinite(float(p_dec)) and math.isfinite(float(p_bps))):
                    nan_inf_count += 1
                if p_dec * Decimal("10000") != p_bps:
                    conversion_mismatch_count += 1
                pre_sum_calc += p_bps
            except (KeyError, TypeError, ValueError, InvalidOperation):
                conversion_mismatch_count += 1

    pre_sum_str = "0"
    if isinstance(payload, dict) and "pre_listing_filtered_sum_bps" in payload:
        try:
            pre_sum_decl = Decimal(str(payload["pre_listing_filtered_sum_bps"]))
            if pre_sum_decl != pre_sum_calc:
                conversion_mismatch_count += 1
            pre_sum_str = format_decimal(pre_sum_decl)
        except (ValueError, InvalidOperation):
            conversion_mismatch_count += 1

    return funding_by_ts, {
        "file": str(path),
        "events": len(events) if isinstance(events, list) else 0,
        "first_linear_open_ms": first_lin_ms,
        "first_linear_open_utc": ms_to_iso(first_lin_ms) if isinstance(first_lin_ms, int) else None,
        "effective_funding_start_ms": eff_fund_start_ms,
        "effective_funding_start_utc": ms_to_iso(eff_fund_start_ms) if isinstance(eff_fund_start_ms, int) else None,
        "pre_listing_filtered_count": len(pre_events) if isinstance(pre_events, list) else 0,
        "pre_listing_filtered_sum_bps": pre_sum_str,
        "pre_listing_filtered_events": pre_events if isinstance(pre_events, list) else [],
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
        description="Audit Track 6 (5y 1d linear+spot, 30 pairs) CSV datasets and Decimal funding reconciliation."
    )
    parser.add_argument("--data-root", default="data_local", help="Root data directory")
    args = parser.parse_args()

    assert int(datetime(2021, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == START_MS
    assert int(datetime(2024, 5, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == SPLIT_MS
    assert int(datetime(2024, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == COHORT24_MS
    assert int(datetime(2026, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == END_MS
    assert EXPECTED_FULL_BARS == 1826
    assert EXPECTED_R1_BARS == 912
    assert EXPECTED_R2_BARS == 914
    assert EXPECTED_24M_BARS == 730

    root = Path(args.data_root)
    datasets = [
        ("bybit_carry30_5y", START_MS, END_MS),
        ("bybit_carry30_r1", START_MS, SPLIT_MS),
        ("bybit_carry30_r2", SPLIT_MS, END_MS),
        ("bybit_carry30_24m", COHORT24_MS, END_MS),
    ]
    categories = ("linear", "spot")

    totals = {
        "raw_funding_files_checked": 0,
        "raw_funding_failures": 0,
        "manifest_failures": 0,
        "csv_files_checked": 0,
        "missing_files_total": 0,
        "empty_non_r1_files_total": 0,
        "spot_absent_files_total": 0,
        "spot_absence_mismatch_total": 0,
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
        "header_only_r1_files": 0,
        "split_mismatch_total": 0,
        "cohort24_mismatch_total": 0,
        "per_bar_funding_mismatch_total": 0,
        "subbar_aggregated_events_total": 0,
        "pre_listing_filtered_events_total": 0,
        "pre_listing_filtered_sum_bps_total": "0",
        "unmatched_funding_events_total": 0,
        "funding_contract_life_mismatch_total": 0,
        "funding_reconciliation_failures": 0,
    }

    spot_listed_map: dict[str, bool] = {sym: True for _, sym in SYMBOLS}
    manifest_path = root / "bybit_carry30_5y" / "download_manifest.json"
    if manifest_path.exists():
        m_bytes = manifest_path.read_bytes()
        try:
            m_text = m_bytes.decode("utf-8")
            manifest = json.loads(m_text)
            m_ok = not m_bytes.startswith(b"\xef\xbb\xbf") and b"\r" not in m_bytes
        except (UnicodeDecodeError, json.JSONDecodeError):
            manifest = {}
            m_ok = False
        if not m_ok:
            totals["encoding_failures"] += 1
            totals["manifest_failures"] += 1
        else:
            m_syms = manifest.get("symbols")
            if not isinstance(m_syms, list) or len(m_syms) != len(SYMBOLS):
                totals["manifest_failures"] += 1
            else:
                parsed_map: dict[str, bool] = {}
                for expected_tup, entry in zip(SYMBOLS, m_syms):
                    if (
                        not isinstance(entry, dict)
                        or entry.get("symbol_id") != expected_tup[0]
                        or entry.get("symbol") != expected_tup[1]
                        or not isinstance(entry.get("spot_listed_on_bybit"), bool)
                    ):
                        totals["manifest_failures"] += 1
                        break
                    parsed_map[expected_tup[1]] = entry["spot_listed_on_bybit"]
                if len(parsed_map) == len(SYMBOLS):
                    spot_listed_map = parsed_map
                    expected_dual_cnt = sum(1 for v in spot_listed_map.values() if v)
                    expected_perp_only = [s for _, s in SYMBOLS if not spot_listed_map[s]]
                    if (
                        manifest.get("dual_listed_symbols_count") != expected_dual_cnt
                        or manifest.get("perp_only_symbols") != expected_perp_only
                    ):
                        totals["manifest_failures"] += 1
    else:
        totals["missing_files_total"] += 1
        totals["manifest_failures"] += 1

    raw_funding_by_sym: dict[str, dict[int, Decimal]] = {}
    expected_buckets_by_sym: dict[str, tuple[dict[int, Decimal], int, list[tuple[int, int]]]] = {}
    raw_funding_audit: list[dict] = []
    raw_funding_meta: dict[str, dict] = {}
    total_pre_filtered_bps = Decimal("0")

    for sym_id, sym in SYMBOLS:
        raw_path = root / "bybit_carry30_5y" / "raw_funding" / f"{sym}.json"
        if not raw_path.exists():
            totals["missing_files_total"] += 1
            totals["raw_funding_failures"] += 1
            raw_funding_by_sym[sym] = {}
            expected_buckets_by_sym[sym] = ({}, 0, [])
            raw_funding_meta[sym] = {
                "first_linear_open_ms": None,
                "pre_listing_filtered_count": 0,
                "pre_listing_filtered_sum_bps": "0",
                "pre_listing_filtered_events": [],
            }
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
        totals["pre_listing_filtered_events_total"] += f_audit["pre_listing_filtered_count"]
        total_pre_filtered_bps += Decimal(f_audit["pre_listing_filtered_sum_bps"])
        raw_funding_meta[sym] = f_audit
        raw_funding_audit.append(
            {
                "symbol_id": sym_id,
                "symbol": sym,
                "file": f_audit["file"],
                "events": f_audit["events"],
                "first_linear_open_ms": f_audit["first_linear_open_ms"],
                "first_linear_open_utc": f_audit["first_linear_open_utc"],
                "effective_funding_start_ms": f_audit["effective_funding_start_ms"],
                "effective_funding_start_utc": f_audit["effective_funding_start_utc"],
                "pre_listing_filtered_count": f_audit["pre_listing_filtered_count"],
                "pre_listing_filtered_sum_bps": f_audit["pre_listing_filtered_sum_bps"],
                "pre_listing_filtered_events_sample": f_audit["pre_listing_filtered_events"][:25],
                "schema_ok": f_audit["schema_ok"],
                "utf8_no_bom": f_audit["utf8_no_bom"],
                "lf_only": f_audit["lf_only"],
                "nan_inf_count": f_audit["nan_inf_count"],
                "out_of_window_count": f_audit["out_of_window_count"],
                "duplicate_ts_count": f_audit["duplicate_ts_count"],
                "backward_ts_count": f_audit["backward_ts_count"],
                "conversion_mismatch_count": f_audit["conversion_mismatch_count"],
            }
        )
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

    totals["pre_listing_filtered_sum_bps_total"] = format_decimal(total_pre_filtered_bps)

    per_ds_results: dict[tuple[str, str, str], dict] = {}
    reconciliation_rows: list[dict] = []

    for ds_name, w_start, w_end in datasets:
        for cat in categories:
            for sym_id, sym in SYMBOLS:
                csv_path = root / ds_name / cat / f"{sym}.csv"
                is_spot_absent = cat == "spot" and not spot_listed_map.get(sym, True)
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

                if is_spot_absent:
                    totals["spot_absent_files_total"] += 1
                    if res["rows"] != 0:
                        totals["spot_absence_mismatch_total"] += 1
                elif res["rows"] == 0:
                    if ds_name == "bybit_carry30_r1":
                        totals["header_only_r1_files"] += 1
                    else:
                        totals["empty_non_r1_files_total"] += 1

                if res["gap_count"] > 0:
                    totals["gap_files_total"] += 1
                    totals["gap_count_total"] += res["gap_count"]
                    totals["missing_bars_total"] += res["missing_bars_total"]
                if res["trailing_missing_bars"] > 0:
                    totals["trailing_missing_bars_total"] += res["trailing_missing_bars"]
                if res["leading_missing_bars"] > 0 and not is_spot_absent:
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

                    if ds_name == "bybit_carry30_5y":
                        meta = raw_funding_meta[sym]
                        if meta["first_linear_open_ms"] != res["min_ts_ms"]:
                            totals["funding_contract_life_mismatch_total"] += 1

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
                                "first_linear_open_utc": res["min_utc"],
                                "effective_funding_start_utc": meta.get("effective_funding_start_utc"),
                                "pre_listing_filtered_count": meta.get("pre_listing_filtered_count", 0),
                                "pre_listing_filtered_sum_bps": meta.get("pre_listing_filtered_sum_bps", "0"),
                                "pre_listing_filtered_events_sample": meta.get("pre_listing_filtered_events", [])[:25],
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
    cohort24_audit: list[dict] = []
    for cat in categories:
        for sym_id, sym in SYMBOLS:
            full_r = per_ds_results[("bybit_carry30_5y", cat, sym)]
            r1_r = per_ds_results[("bybit_carry30_r1", cat, sym)]
            r2_r = per_ds_results[("bybit_carry30_r2", cat, sym)]
            c24_r = per_ds_results[("bybit_carry30_24m", cat, sym)]
            is_spot_absent = cat == "spot" and not spot_listed_map.get(sym, True)

            counts_ok = (r1_r["rows"] + r2_r["rows"]) == full_r["rows"]
            disjoint_ok = r1_r["ts_set"].isdisjoint(r2_r["ts_set"])
            union_ok = (r1_r["ts_set"] | r2_r["ts_set"]) == full_r["ts_set"]
            r1_empty_consistent = (
                is_spot_absent
                or r1_r["rows"] > 0
                or (full_r["min_ts_ms"] is not None and full_r["min_ts_ms"] >= SPLIT_MS)
            )
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
                and r1_empty_consistent
                and rows_ok
            )
            if not split_ok:
                totals["split_mismatch_total"] += 1
            split_audit.append(
                {
                    "symbol_id": sym_id,
                    "symbol": sym,
                    "category": cat,
                    "spot_listed_on_bybit": spot_listed_map.get(sym, True) if cat == "spot" else True,
                    "full_rows": full_r["rows"],
                    "r1_rows": r1_r["rows"],
                    "r2_rows": r2_r["rows"],
                    "r1_empty_consistent": r1_empty_consistent,
                    "split_ok": split_ok,
                }
            )

            expected_24m_ts = {ts for ts in full_r["ts_set"] if ts >= COHORT24_MS}
            c24_ts_ok = c24_r["ts_set"] == expected_24m_ts
            c24_rows_ok = all(
                full_r["rows_by_ts"].get(ts) == row for ts, row in c24_r["rows_by_ts"].items()
            )
            c24_ok = (
                not full_r.get("missing")
                and not c24_r.get("missing")
                and c24_r["rows"] == len(expected_24m_ts)
                and c24_ts_ok
                and c24_rows_ok
            )
            if not c24_ok:
                totals["cohort24_mismatch_total"] += 1
            cohort24_audit.append(
                {
                    "symbol_id": sym_id,
                    "symbol": sym,
                    "category": cat,
                    "c24_rows": c24_r["rows"],
                    "expected_from_full": len(expected_24m_ts),
                    "cohort24_ok": c24_ok,
                }
            )

    coverage_rows: list[dict] = []
    lin_24m_full_cov_cnt = 0
    spot_24m_full_cov_cnt = 0
    dual_24m_full_cov_cnt = 0

    for sym_id, sym in SYMBOLS:
        spot_listed = spot_listed_map.get(sym, True)
        lin_full = per_ds_results[("bybit_carry30_5y", "linear", sym)]
        lin_r1 = per_ds_results[("bybit_carry30_r1", "linear", sym)]
        lin_r2 = per_ds_results[("bybit_carry30_r2", "linear", sym)]
        lin_24m = per_ds_results[("bybit_carry30_24m", "linear", sym)]

        spot_full = per_ds_results[("bybit_carry30_5y", "spot", sym)]
        spot_r1 = per_ds_results[("bybit_carry30_r1", "spot", sym)]
        spot_r2 = per_ds_results[("bybit_carry30_r2", "spot", sym)]
        spot_24m = per_ds_results[("bybit_carry30_24m", "spot", sym)]

        overlap_ts = sorted(lin_full["ts_set"] & spot_full["ts_set"])
        overlap_r1 = sum(1 for ts in overlap_ts if ts < SPLIT_MS)
        overlap_r2 = sum(1 for ts in overlap_ts if ts >= SPLIT_MS)
        overlap_24m = sum(1 for ts in overlap_ts if ts >= COHORT24_MS)

        if lin_24m["rows"] == EXPECTED_24M_BARS:
            lin_24m_full_cov_cnt += 1
        if spot_listed and spot_24m["rows"] == EXPECTED_24M_BARS:
            spot_24m_full_cov_cnt += 1
        if spot_listed and overlap_24m == EXPECTED_24M_BARS:
            dual_24m_full_cov_cnt += 1

        coverage_rows.append(
            {
                "symbol_id": sym_id,
                "symbol": sym,
                "spot_listed_on_bybit": spot_listed,
                "dual_overlap_days": len(overlap_ts),
                "dual_overlap_r1_days": overlap_r1,
                "dual_overlap_r2_days": overlap_r2,
                "dual_overlap_24m_days": overlap_24m,
                "dual_overlap_min_utc": ms_to_iso(overlap_ts[0]) if overlap_ts else None,
                "dual_overlap_max_utc": ms_to_iso(overlap_ts[-1]) if overlap_ts else None,
                "linear": {
                    "rows": lin_full["rows"],
                    "expected_bars": EXPECTED_FULL_BARS,
                    "coverage_pct": lin_full["coverage_pct"],
                    "r1_rows": lin_r1["rows"],
                    "r2_rows": lin_r2["rows"],
                    "c24_rows": lin_24m["rows"],
                    "c24_expected_bars": EXPECTED_24M_BARS,
                    "c24_coverage_pct": lin_24m["coverage_pct"],
                    "min_utc": lin_full["min_utc"],
                    "max_utc": lin_full["max_utc"],
                    "leading_missing_bars": lin_full["leading_missing_bars"],
                    "trailing_missing_bars": lin_full["trailing_missing_bars"],
                    "gap_count": lin_full["gap_count"],
                    "missing_bars_total": lin_full["missing_bars_total"],
                },
                "spot": {
                    "spot_listed_on_bybit": spot_listed,
                    "rows": spot_full["rows"],
                    "expected_bars": EXPECTED_FULL_BARS,
                    "coverage_pct": spot_full["coverage_pct"],
                    "r1_rows": spot_r1["rows"],
                    "r2_rows": spot_r2["rows"],
                    "c24_rows": spot_24m["rows"],
                    "c24_expected_bars": EXPECTED_24M_BARS,
                    "c24_coverage_pct": spot_24m["coverage_pct"],
                    "min_utc": spot_full["min_utc"],
                    "max_utc": spot_full["max_utc"],
                    "leading_missing_bars": spot_full["leading_missing_bars"],
                    "trailing_missing_bars": spot_full["trailing_missing_bars"],
                    "gap_count": spot_full["gap_count"],
                    "missing_bars_total": spot_full["missing_bars_total"],
                },
            }
        )

    dual_listed_count = sum(1 for _, sym in SYMBOLS if spot_listed_map.get(sym, True))
    perp_only_symbols = [sym for _, sym in SYMBOLS if not spot_listed_map.get(sym, True)]

    cohort24_summary = {
        "symbols_total": len(SYMBOLS),
        "dual_listed_symbols_count": dual_listed_count,
        "perp_only_symbols": perp_only_symbols,
        "expected_24m_bars": EXPECTED_24M_BARS,
        "linear_full_730_count": lin_24m_full_cov_cnt,
        "spot_dual_full_730_count": spot_24m_full_cov_cnt,
        "dual_overlap_full_730_count": dual_24m_full_cov_cnt,
        "active_dual_pairs_24m_100pct": (
            dual_listed_count > 0 and dual_24m_full_cov_cnt == dual_listed_count
        ),
    }

    hard_errors = (
        totals["missing_files_total"]
        + totals["manifest_failures"]
        + totals["empty_non_r1_files_total"]
        + totals["spot_absence_mismatch_total"]
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
        + totals["cohort24_mismatch_total"]
        + totals["per_bar_funding_mismatch_total"]
        + totals["unmatched_funding_events_total"]
        + totals["funding_contract_life_mismatch_total"]
        + totals["funding_reconciliation_failures"]
    )

    status = "PASS" if hard_errors == 0 else "FAIL"
    report = {
        "status": status,
        "timeframe": TF_NAME,
        "interval_ms": INTERVAL_MS,
        "symbols_count": len(SYMBOLS),
        "dual_listed_symbols_count": dual_listed_count,
        "perp_only_symbols": perp_only_symbols,
        "window_start_ms": START_MS,
        "window_start_utc": ms_to_iso(START_MS),
        "split_ms": SPLIT_MS,
        "split_utc": ms_to_iso(SPLIT_MS),
        "cohort24_ms": COHORT24_MS,
        "cohort24_utc": ms_to_iso(COHORT24_MS),
        "window_end_ms": END_MS,
        "window_end_utc": ms_to_iso(END_MS),
        "funding_alignment": "BarOpenSettlement_CausalHalfOpen_(bar_open_ts-24h,bar_open_ts]_ContractLifeBounded",
        "totals": totals,
        "cohort24_summary": cohort24_summary,
        "coverage": coverage_rows,
        "funding_reconciliation": reconciliation_rows,
        "raw_funding_audit": raw_funding_audit,
        "split_audit": split_audit,
        "cohort24_audit": cohort24_audit,
    }

    out_path = root / "bybit_carry30_5y" / "audit_report.json"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = out_path.with_suffix(".json.tmp")
    with tmp_path.open("w", encoding="utf-8", newline="\n") as fh:
        json.dump(report, fh, indent=2)
        fh.write("\n")
    tmp_path.replace(out_path)

    print(json.dumps({"status": status, "totals": totals, "cohort24_summary": cohort24_summary}, indent=2))

    coverage_anomalies = [
        c
        for c in coverage_rows
        if not c["spot_listed_on_bybit"]
        or c["linear"]["rows"] != EXPECTED_FULL_BARS
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
        spot_desc = (
            f"spot={spot['rows']}/{spot['expected_bars']} ({spot['coverage_pct']}%, r1={spot['r1_rows']}, r2={spot['r2_rows']}, 24m={spot['c24_rows']}/730, gaps={spot['gap_count']}, {spot['min_utc']}..{spot['max_utc']})"
            if item["spot_listed_on_bybit"]
            else "spot=0/1826 (spot_listed_on_bybit=False, header-only)"
        )
        print(
            f"  [{item['symbol_id']:02d}] {item['symbol']:10s}: "
            f"lin={lin['rows']}/{lin['expected_bars']} ({lin['coverage_pct']}%, r1={lin['r1_rows']}, r2={lin['r2_rows']}, 24m={lin['c24_rows']}/730, gaps={lin['gap_count']}, {lin['min_utc']}..{lin['max_utc']}) | "
            f"{spot_desc} | "
            f"overlap={item['dual_overlap_days']}d (r1={item['dual_overlap_r1_days']}, r2={item['dual_overlap_r2_days']}, 24m={item['dual_overlap_24m_days']})"
        )

    pre_filtered = [r for r in reconciliation_rows if r["pre_listing_filtered_count"] > 0]
    print(f"Pre-listing contract-life filtered funding entries: {len(pre_filtered)}")
    for item in pre_filtered:
        print(
            f"  [{item['symbol_id']:02d}] {item['symbol']:10s}: "
            f"first_lin={item['first_linear_open_utc']}, eff_fund_start={item['effective_funding_start_utc']}, "
            f"filtered={item['pre_listing_filtered_count']} events ({item['pre_listing_filtered_sum_bps']} bps), "
            f"retained={item['raw_funding_events']} events"
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

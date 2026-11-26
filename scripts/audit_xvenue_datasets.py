#!/usr/bin/env python3
"""Audit Track 7 (16-pair 4h Bybit + Hyperliquid) CSV/JSON datasets and descriptive funding spread stats.

Strict Law compliance:
- Pure Python stdlib only (0 pip dependencies).
- Law 1: Validates encoding, schemas, OHLCV invariants, bounds, monotonicity,
  exact symbol_id (0..15) mapping, Y1+Y2 == 24m split partitioning, cross-venue
  bar alignment, exact Decimal funding reconciliation on BOTH venues, and computes
  only descriptive raw funding spread statistics over matched 4h bars.
  Zero signals, turnover, fees, recentering, Newey-West, or DSR computations.
- Law 4: Never synthesizes or pads missing bars.
"""

from __future__ import annotations

import argparse
import csv
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
import json
import math
from pathlib import Path
import statistics
import sys

UNIVERSE: list[tuple[int, str, str]] = [
    (0, "BTCUSDT", "BTC"),
    (1, "ETHUSDT", "ETH"),
    (2, "SOLUSDT", "SOL"),
    (3, "AVAXUSDT", "AVAX"),
    (4, "TAOUSDT", "TAO"),
    (5, "SUIUSDT", "SUI"),
    (6, "ORDIUSDT", "ORDI"),
    (7, "PENDLEUSDT", "PENDLE"),
    (8, "WIFUSDT", "WIF"),
    (9, "LINKUSDT", "LINK"),
    (10, "BNBUSDT", "BNB"),
    (11, "XRPUSDT", "XRP"),
    (12, "DOGEUSDT", "DOGE"),
    (13, "AAVEUSDT", "AAVE"),
    (14, "ARBUSDT", "ARB"),
    (15, "INJUSDT", "INJ"),
]

INTERVAL_MS = 14_400_000
HOUR_MS = 3_600_000
START_MS = 1732492800000           # 2024-11-25T00:00:00Z
SPLIT_MS = 1764028800000           # 2025-11-25T00:00:00Z
END_MS = 1795564800000             # 2026-11-25T00:00:00Z
LAST_BAR_OPEN_MS = 1795550400000   # 2026-11-24T20:00:00Z
EXPECTED_FULL_BARS = (END_MS - START_MS) // INTERVAL_MS       # 4380
EXPECTED_SPLIT_BARS = (SPLIT_MS - START_MS) // INTERVAL_MS    # 2190

HL_FUNDING_KEEP_MIN_MS = START_MS - 3 * HOUR_MS
MAX_JITTER_MS = 1_799_999
WARN_JITTER_MS = 60_000

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


def percentile_linear(sorted_vals: list[float], q: float) -> float:
    if not sorted_vals:
        return 0.0
    if len(sorted_vals) == 1:
        return sorted_vals[0]
    idx = (len(sorted_vals) - 1) * q
    lo = math.floor(idx)
    hi = math.ceil(idx)
    if lo == hi:
        return sorted_vals[lo]
    frac = idx - lo
    return sorted_vals[lo] * (1.0 - frac) + sorted_vals[hi] * frac


def build_expected_bucket_map(
    raw_map: dict[int, Decimal]
) -> tuple[dict[int, Decimal], int, list[tuple[int, int]]]:
    """Map raw funding events onto causal half-open (bar_open_ts - 4h, bar_open_ts] buckets."""
    bucket_map: dict[int, Decimal] = {}
    subbar_aggregated_events = 0
    event_targets: list[tuple[int, int]] = []
    for f_ts, f_bps in sorted(raw_map.items()):
        rem = (f_ts - START_MS) % INTERVAL_MS
        target_open_ts = f_ts if rem == 0 else f_ts + (INTERVAL_MS - rem)
        if rem != 0:
            subbar_aggregated_events += 1
        bucket_map[target_open_ts] = bucket_map.get(target_open_ts, Decimal("0")) + f_bps
        event_targets.append((f_ts, target_open_ts))
    return bucket_map, subbar_aggregated_events, event_targets


def audit_raw_bybit_funding(
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
        and isinstance(events, list)
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


def audit_raw_hl_funding(
    path: Path, expected_sym_id: int, expected_bybit_sym: str, expected_coin: str
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
    first_candle_ts_ms = payload.get("first_candle_ts_ms") if isinstance(payload, dict) else None
    schema_ok = (
        decode_ok
        and isinstance(payload, dict)
        and payload.get("symbol_id") == expected_sym_id
        and payload.get("coin") == expected_coin
        and payload.get("bybit_symbol") == expected_bybit_sym
        and payload.get("window_start_ms") == START_MS
        and payload.get("last_bar_open_ms") == LAST_BAR_OPEN_MS
        and payload.get("window_end_ms") == END_MS
        and isinstance(first_candle_ts_ms, int)
        and isinstance(events, list)
        and payload.get("count") == len(events)
    )

    out_of_window_count = 0
    duplicate_snapped_count = 0
    backward_snapped_count = 0
    conversion_mismatch_count = 0
    jitter_violation_count = 0
    snap_formula_mismatch_count = 0
    nan_inf_count = 0
    max_jitter_ms = 0
    jitter_gt_60s_events: list[dict] = []
    prev_snapped: int | None = None
    funding_by_snapped: dict[int, Decimal] = {}

    eff_min_snapped = (
        max(HL_FUNDING_KEEP_MIN_MS, first_candle_ts_ms - INTERVAL_MS + 1)
        if isinstance(first_candle_ts_ms, int)
        else HL_FUNDING_KEEP_MIN_MS
    )

    if isinstance(events, list):
        for ev in events:
            try:
                raw_time = int(ev["raw_time_ms"])
                snapped_ms = int(ev["snapped_ms"])
                jitter_decl = int(ev["jitter_ms"])
            except (KeyError, TypeError, ValueError):
                conversion_mismatch_count += 1
                continue
            expected_snap = ((raw_time + 1_800_000) // HOUR_MS) * HOUR_MS
            if snapped_ms != expected_snap:
                snap_formula_mismatch_count += 1
            jitter = abs(raw_time - snapped_ms)
            if jitter > MAX_JITTER_MS or jitter != jitter_decl:
                jitter_violation_count += 1
            max_jitter_ms = max(max_jitter_ms, jitter)
            if jitter > WARN_JITTER_MS:
                jitter_gt_60s_events.append(
                    {
                        "raw_time_ms": raw_time,
                        "snapped_ms": snapped_ms,
                        "utc": ev.get("utc"),
                        "jitter_ms": jitter,
                    }
                )
            if snapped_ms < eff_min_snapped or snapped_ms > LAST_BAR_OPEN_MS:
                out_of_window_count += 1
            if prev_snapped is not None:
                if snapped_ms == prev_snapped:
                    duplicate_snapped_count += 1
                elif snapped_ms < prev_snapped:
                    backward_snapped_count += 1
            prev_snapped = snapped_ms
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
            funding_by_snapped[snapped_ms] = rate_bps

    if isinstance(payload, dict) and payload.get("max_jitter_ms") != max_jitter_ms:
        schema_ok = False

    return funding_by_snapped, {
        "file": str(path),
        "events": len(events) if isinstance(events, list) else 0,
        "first_candle_ts_ms": first_candle_ts_ms,
        "max_jitter_ms": max_jitter_ms,
        "jitter_gt_60s_count": len(jitter_gt_60s_events),
        "jitter_gt_60s_events": jitter_gt_60s_events,
        "schema_ok": schema_ok,
        "utf8_no_bom": decode_ok and not has_bom,
        "lf_only": not has_cr,
        "nan_inf_count": nan_inf_count,
        "out_of_window_count": out_of_window_count,
        "duplicate_snapped_count": duplicate_snapped_count,
        "backward_snapped_count": backward_snapped_count,
        "jitter_violation_count": jitter_violation_count,
        "snap_formula_mismatch_count": snap_formula_mismatch_count,
        "conversion_mismatch_count": conversion_mismatch_count,
    }


def audit_csv(
    path: Path,
    expected_sym_id: int,
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
        "max_ts_ms": last_ts,
        "header_ok": header_ok,
        "utf8_no_bom": decode_ok and not has_bom,
        "lf_only": not has_cr,
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
        "rows_by_ts": rows_by_ts,
    }


def compute_descriptive_spread_stats(
    bybit_funding: dict[int, Decimal],
    hl_funding: dict[int, Decimal],
    matched_ts: list[int],
) -> dict:
    diffs_dec: list[Decimal] = []
    diffs_flt: list[float] = []
    sum_hl = Decimal("0")
    sum_bybit = Decimal("0")
    gt_count = 0

    for ts in matched_ts:
        hl_v = hl_funding[ts]
        bb_v = bybit_funding[ts]
        diff = hl_v - bb_v
        sum_hl += hl_v
        sum_bybit += bb_v
        diffs_dec.append(diff)
        diffs_flt.append(float(diff))
        if hl_v > bb_v:
            gt_count += 1

    n = len(diffs_flt)
    sorted_flt = sorted(diffs_flt)
    mean_v = statistics.fmean(diffs_flt) if n > 0 else 0.0
    std_v = statistics.stdev(diffs_flt) if n >= 2 else 0.0
    median_v = statistics.median(sorted_flt) if n > 0 else 0.0
    p10_v = percentile_linear(sorted_flt, 0.10)
    p90_v = percentile_linear(sorted_flt, 0.90)
    sum_diff = sum_hl - sum_bybit
    pct_gt = (gt_count / n) * 100.0 if n > 0 else 0.0

    return {
        "matched_bars": n,
        "mean": round(mean_v, 6),
        "std": round(std_v, 6),
        "median": round(median_v, 6),
        "p10": round(p10_v, 6),
        "p90": round(p90_v, 6),
        "sum_hl_bps": format_decimal(sum_hl),
        "sum_bybit_bps": format_decimal(sum_bybit),
        "sum_diff_bps": format_decimal(sum_diff),
        "pct_bars_hl_gt_bybit": round(pct_gt, 4),
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Audit Track 7 (16-pair 4h Bybit + Hyperliquid) CSV/JSON datasets."
    )
    parser.add_argument("--data-root", default="data_local", help="Root data directory")
    args = parser.parse_args()
    root = Path(args.data_root)

    datasets = [
        ("bybit_hl_xvenue_24m", START_MS, END_MS),
        ("bybit_hl_xvenue_y1", START_MS, SPLIT_MS),
        ("bybit_hl_xvenue_y2", SPLIT_MS, END_MS),
    ]
    venues = ("bybit", "hyperliquid")

    totals = {
        "manifest_failures": 0,
        "raw_bybit_funding_checked": 0,
        "raw_bybit_funding_failures": 0,
        "raw_hl_funding_checked": 0,
        "raw_hl_funding_failures": 0,
        "csv_files_checked": 0,
        "missing_files_total": 0,
        "empty_files_total": 0,
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
        "cross_venue_alignment_failures": 0,
        "per_bar_funding_mismatch_total": 0,
        "unmatched_funding_events_total": 0,
        "funding_reconciliation_failures": 0,
    }

    # 1. Audit download_manifest.json
    manifest_path = root / "bybit_hl_xvenue_24m" / "download_manifest.json"
    manifest_audit: dict = {"file": str(manifest_path), "exists": manifest_path.exists()}
    if not manifest_path.exists():
        totals["missing_files_total"] += 1
        totals["manifest_failures"] += 1
        manifest_audit["ok"] = False
    else:
        m_bytes = manifest_path.read_bytes()
        m_no_bom = not m_bytes.startswith(b"\xef\xbb\xbf")
        m_lf_only = b"\r" not in m_bytes
        if not (m_no_bom and m_lf_only):
            totals["encoding_failures"] += 1
        try:
            manifest = json.loads(m_bytes.decode("utf-8"))
        except (UnicodeDecodeError, json.JSONDecodeError):
            manifest = {}
        m_syms = manifest.get("symbols") if isinstance(manifest, dict) else None
        manifest_ok = (
            isinstance(manifest, dict)
            and m_no_bom
            and m_lf_only
            and manifest.get("timeframe") == "4h"
            and manifest.get("interval_ms") == INTERVAL_MS
            and manifest.get("expected_candles") == EXPECTED_FULL_BARS
            and manifest.get("universe_count") == len(UNIVERSE)
            and manifest.get("completed_symbols_count") == len(UNIVERSE)
            and manifest.get("pending_coins") == []
            and manifest.get("window_start_ms") == START_MS
            and manifest.get("split_ms") == SPLIT_MS
            and manifest.get("window_end_ms") == END_MS
            and isinstance(m_syms, list)
            and len(m_syms) == len(UNIVERSE)
            and all(
                isinstance(e, dict)
                and e.get("symbol_id") == sid
                and e.get("bybit_symbol") == bsym
                and e.get("hl_coin") == hcoin
                for (sid, bsym, hcoin), e in zip(UNIVERSE, m_syms)
            )
        )
        if not manifest_ok:
            totals["manifest_failures"] += 1
        manifest_audit.update(
            {
                "ok": manifest_ok,
                "utf8_no_bom": m_no_bom,
                "lf_only": m_lf_only,
                "completed_symbols_count": manifest.get("completed_symbols_count") if isinstance(manifest, dict) else None,
            }
        )

    # 2. Audit 16 Bybit raw funding JSONs + 16 HL raw funding JSONs
    raw_funding_by_venue: dict[tuple[str, int], dict[int, Decimal]] = {}
    expected_buckets_by_venue: dict[tuple[str, int], tuple[dict[int, Decimal], int, list[tuple[int, int]]]] = {}
    raw_bybit_audit: list[dict] = []
    raw_hl_audit: list[dict] = []

    for sym_id, bybit_sym, coin in UNIVERSE:
        bb_raw_path = root / "bybit_hl_xvenue_24m" / "raw_bybit_funding" / f"{bybit_sym}.json"
        if not bb_raw_path.exists():
            totals["missing_files_total"] += 1
            totals["raw_bybit_funding_failures"] += 1
            raw_funding_by_venue[("bybit", sym_id)] = {}
            expected_buckets_by_venue[("bybit", sym_id)] = ({}, 0, [])
            raw_bybit_audit.append({"symbol_id": sym_id, "symbol": bybit_sym, "missing": True})
        else:
            f_map, f_audit = audit_raw_bybit_funding(bb_raw_path, sym_id, bybit_sym)
            raw_funding_by_venue[("bybit", sym_id)] = f_map
            expected_buckets_by_venue[("bybit", sym_id)] = build_expected_bucket_map(f_map)
            raw_bybit_audit.append({"symbol_id": sym_id, "symbol": bybit_sym, **f_audit})
            totals["raw_bybit_funding_checked"] += 1
            if not (f_audit["utf8_no_bom"] and f_audit["lf_only"]):
                totals["encoding_failures"] += 1
            if (
                not f_audit["schema_ok"]
                or f_audit["nan_inf_count"] > 0
                or f_audit["out_of_window_count"] > 0
                or f_audit["duplicate_ts_count"] > 0
                or f_audit["backward_ts_count"] > 0
                or f_audit["conversion_mismatch_count"] > 0
            ):
                totals["raw_bybit_funding_failures"] += 1

        hl_raw_path = root / "bybit_hl_xvenue_24m" / "raw_hl" / f"{coin}_funding.json"
        if not hl_raw_path.exists():
            totals["missing_files_total"] += 1
            totals["raw_hl_funding_failures"] += 1
            raw_funding_by_venue[("hyperliquid", sym_id)] = {}
            expected_buckets_by_venue[("hyperliquid", sym_id)] = ({}, 0, [])
            raw_hl_audit.append({"symbol_id": sym_id, "coin": coin, "missing": True})
        else:
            h_map, h_audit = audit_raw_hl_funding(hl_raw_path, sym_id, bybit_sym, coin)
            raw_funding_by_venue[("hyperliquid", sym_id)] = h_map
            expected_buckets_by_venue[("hyperliquid", sym_id)] = build_expected_bucket_map(h_map)
            raw_hl_audit.append({"symbol_id": sym_id, "coin": coin, **h_audit})
            totals["raw_hl_funding_checked"] += 1
            if not (h_audit["utf8_no_bom"] and h_audit["lf_only"]):
                totals["encoding_failures"] += 1
            if (
                not h_audit["schema_ok"]
                or h_audit["nan_inf_count"] > 0
                or h_audit["out_of_window_count"] > 0
                or h_audit["duplicate_snapped_count"] > 0
                or h_audit["backward_snapped_count"] > 0
                or h_audit["jitter_violation_count"] > 0
                or h_audit["snap_formula_mismatch_count"] > 0
                or h_audit["conversion_mismatch_count"] > 0
            ):
                totals["raw_hl_funding_failures"] += 1

    # 3. Audit all 96 CSVs (3 splits x 2 venues x 16 coins)
    per_ds_results: dict[tuple[str, str, int], dict] = {}
    reconciliation_rows: list[dict] = []

    for ds_name, w_start, w_end in datasets:
        for venue in venues:
            for sym_id, bybit_sym, coin in UNIVERSE:
                fname = f"{bybit_sym}.csv" if venue == "bybit" else f"{coin}.csv"
                csv_path = root / ds_name / venue / fname
                if not csv_path.exists():
                    totals["missing_files_total"] += 1
                    per_ds_results[(ds_name, venue, sym_id)] = {
                        "missing": True,
                        "rows": 0,
                        "expected_bars": (w_end - w_start) // INTERVAL_MS,
                        "ts_set": set(),
                        "funding_by_ts": {},
                        "rows_by_ts": {},
                    }
                    continue

                res = audit_csv(csv_path, sym_id, w_start, w_end)
                per_ds_results[(ds_name, venue, sym_id)] = res
                totals["csv_files_checked"] += 1
                totals["rows_checked"] += res["rows"]
                if res["rows"] == 0:
                    totals["empty_files_total"] += 1
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

                raw_map = raw_funding_by_venue[(venue, sym_id)]
                bucket_map, subbar_aggregated_events, event_targets = expected_buckets_by_venue[(venue, sym_id)]
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

                if ds_name == "bybit_hl_xvenue_24m":
                    raw_sum = sum(raw_map.values(), Decimal("0"))
                    csv_sum = Decimal(res["funding_sum_bps"])
                    diff_bps = raw_sum - csv_sum
                    unmatched_events = [
                        f_ts for f_ts, target_ts in event_targets if target_ts not in res["ts_set"]
                    ]
                    aligned_buckets = sum(1 for ts in res["ts_set"] if ts in bucket_map)
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
                            "venue": venue,
                            "name": bybit_sym if venue == "bybit" else coin,
                            "rows": res["rows"],
                            "raw_funding_events": len(raw_map),
                            "subbar_aggregated_events": subbar_aggregated_events,
                            "aligned_funding_buckets": aligned_buckets,
                            "unmatched_funding_events": len(unmatched_events),
                            "per_bar_mismatch_count": per_bar_mismatch + missing_bucket_bars,
                            "raw_sum_bps": format_decimal(raw_sum),
                            "csv_sum_bps": format_decimal(csv_sum),
                            "diff_bps": format_decimal(diff_bps),
                        }
                    )

    # 4. Exact Y1 + Y2 == 24m partition check for both venues
    split_audit: list[dict] = []
    for venue in venues:
        for sym_id, bybit_sym, coin in UNIVERSE:
            full_r = per_ds_results[("bybit_hl_xvenue_24m", venue, sym_id)]
            y1_r = per_ds_results[("bybit_hl_xvenue_y1", venue, sym_id)]
            y2_r = per_ds_results[("bybit_hl_xvenue_y2", venue, sym_id)]
            counts_ok = (y1_r["rows"] + y2_r["rows"]) == full_r["rows"]
            disjoint_ok = y1_r["ts_set"].isdisjoint(y2_r["ts_set"])
            union_ok = (y1_r["ts_set"] | y2_r["ts_set"]) == full_r["ts_set"]
            rows_ok = (
                all(full_r["rows_by_ts"].get(ts) == row for ts, row in y1_r["rows_by_ts"].items())
                and all(full_r["rows_by_ts"].get(ts) == row for ts, row in y2_r["rows_by_ts"].items())
            )
            split_ok = (
                not full_r.get("missing")
                and not y1_r.get("missing")
                and not y2_r.get("missing")
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
                    "venue": venue,
                    "name": bybit_sym if venue == "bybit" else coin,
                    "full_rows": full_r["rows"],
                    "y1_rows": y1_r["rows"],
                    "y2_rows": y2_r["rows"],
                    "split_ok": split_ok,
                }
            )

    # 5. Cross-venue alignment audit & descriptive raw funding spread stats
    cross_venue_alignment: list[dict] = []
    descriptive_spread_stats: list[dict] = []
    for sym_id, bybit_sym, coin in UNIVERSE:
        bb_full = per_ds_results[("bybit_hl_xvenue_24m", "bybit", sym_id)]
        hl_full = per_ds_results[("bybit_hl_xvenue_24m", "hyperliquid", sym_id)]
        bb_y1 = per_ds_results[("bybit_hl_xvenue_y1", "bybit", sym_id)]
        hl_y1 = per_ds_results[("bybit_hl_xvenue_y1", "hyperliquid", sym_id)]
        bb_y2 = per_ds_results[("bybit_hl_xvenue_y2", "bybit", sym_id)]
        hl_y2 = per_ds_results[("bybit_hl_xvenue_y2", "hyperliquid", sym_id)]

        matched_24m_ts = sorted(bb_full["ts_set"] & hl_full["ts_set"])
        matched_y1_ts = sorted(bb_y1["ts_set"] & hl_y1["ts_set"])
        matched_y2_ts = sorted(bb_y2["ts_set"] & hl_y2["ts_set"])

        bb_only_24m = len(bb_full["ts_set"] - hl_full["ts_set"])
        hl_only_24m = len(hl_full["ts_set"] - bb_full["ts_set"])
        exact_equal_24m = (
            len(matched_24m_ts) == bb_full["rows"] == hl_full["rows"]
            and bb_only_24m == 0
            and hl_only_24m == 0
            and len(matched_24m_ts) > 0
        )
        split_matched_sum_ok = (len(matched_y1_ts) + len(matched_y2_ts)) == len(matched_24m_ts)
        # Require exact alignment count consistency (and check if any HL bars lack Bybit bars)
        if not exact_equal_24m or not split_matched_sum_ok:
            totals["cross_venue_alignment_failures"] += 1

        cross_venue_alignment.append(
            {
                "symbol_id": sym_id,
                "bybit_symbol": bybit_sym,
                "hl_coin": coin,
                "bybit_rows_24m": bb_full["rows"],
                "hl_rows_24m": hl_full["rows"],
                "matched_24m": len(matched_24m_ts),
                "matched_y1": len(matched_y1_ts),
                "matched_y2": len(matched_y2_ts),
                "bybit_only_24m": bb_only_24m,
                "hl_only_24m": hl_only_24m,
                "exact_equal_24m": exact_equal_24m,
                "split_matched_sum_ok": split_matched_sum_ok,
            }
        )

        stats_24m = compute_descriptive_spread_stats(
            bb_full["funding_by_ts"], hl_full["funding_by_ts"], matched_24m_ts
        )
        stats_y1 = compute_descriptive_spread_stats(
            bb_y1["funding_by_ts"], hl_y1["funding_by_ts"], matched_y1_ts
        )
        stats_y2 = compute_descriptive_spread_stats(
            bb_y2["funding_by_ts"], hl_y2["funding_by_ts"], matched_y2_ts
        )
        descriptive_spread_stats.append(
            {
                "symbol_id": sym_id,
                "bybit_symbol": bybit_sym,
                "hl_coin": coin,
                **stats_24m,
                "y1": stats_y1,
                "y2": stats_y2,
            }
        )

    hard_errors = (
        totals["manifest_failures"]
        + totals["raw_bybit_funding_failures"]
        + totals["raw_hl_funding_failures"]
        + totals["missing_files_total"]
        + totals["empty_files_total"]
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
        + totals["cross_venue_alignment_failures"]
        + totals["per_bar_funding_mismatch_total"]
        + totals["unmatched_funding_events_total"]
        + totals["funding_reconciliation_failures"]
    )

    status = "PASS" if hard_errors == 0 else "FAIL"
    report = {
        "status": status,
        "generated_utc": datetime.now(tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "funding_alignment": "BarOpenSettlement_CausalHalfOpen_(bar_open_ts-4h,bar_open_ts]",
        "spread_definition_bps": "HL_funding_bps - Bybit_funding_bps (descriptive raw 4h spread over matched bars)",
        "totals": totals,
        "manifest_audit": manifest_audit,
        "cross_venue_alignment": cross_venue_alignment,
        "raw_funding_spread_stats": descriptive_spread_stats,
        "funding_reconciliation": reconciliation_rows,
        "split_audit": split_audit,
        "raw_bybit_funding_audit": raw_bybit_audit,
        "raw_hl_funding_audit": raw_hl_audit,
    }

    out_path = root / "bybit_hl_xvenue_24m" / "audit_report.json"
    out_path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = out_path.with_suffix(".json.tmp")
    with tmp_path.open("w", encoding="utf-8", newline="\n") as fh:
        json.dump(report, fh, indent=2)
        fh.write("\n")
    tmp_path.replace(out_path)

    print(json.dumps({"status": status, "totals": totals}, indent=2))
    print("\nDescriptive raw funding spread stats (HL_bps - Bybit_bps, 24m matched 4h bars):")
    print(
        f"{'ID':>2} {'Coin':<7} {'Bars':>5} {'Mean':>8} {'Std':>8} {'Med':>8} "
        f"{'P10':>8} {'P90':>8} {'SumHL':>11} {'SumBybit':>11} {'SumDiff':>11} {'HL>BB%':>7}"
    )
    for row in descriptive_spread_stats:
        print(
            f"{row['symbol_id']:02d} {row['hl_coin']:<7} {row['matched_bars']:5d} "
            f"{row['mean']:8.4f} {row['std']:8.4f} {row['median']:8.4f} "
            f"{row['p10']:8.4f} {row['p90']:8.4f} "
            f"{row['sum_hl_bps']:>11} {row['sum_bybit_bps']:>11} {row['sum_diff_bps']:>11} "
            f"{row['pct_bars_hl_gt_bybit']:6.2f}%"
        )

    return 0 if hard_errors == 0 else 1


if __name__ == "__main__":
    sys.exit(main())

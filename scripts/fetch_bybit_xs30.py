#!/usr/bin/env python3
"""Fetch 24 months of Bybit V5 public 4h linear perpetual klines and funding history for 30 pairs.

Strict Law compliance: only downloads, validates, aligns BarOpenSettlement funding,
and writes normalized CSVs for the Rust `vortex-alfa` binary. Performs zero signal,
turnover, fee, Newey-West, or DSR computations. Never synthesizes missing bars.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import csv
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
import json
import math
from pathlib import Path
import random
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

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
INTERVAL = "240"
INTERVAL_MS = 14_400_000

START_MS = 1732492800000  # 2024-11-25T00:00:00Z
SPLIT_MS = 1764028800000  # 2025-11-25T00:00:00Z
END_MS = 1795564800000    # 2026-11-25T00:00:00Z
EXPECTED_CANDLES = (END_MS - START_MS) // INTERVAL_MS  # 4380

BASE_URL = "https://api.bybit.com"
CSV_HEADER = [
    "symbol_id",
    "ts_ms",
    "open",
    "high",
    "low",
    "close",
    "volume",
    "funding_rate_bps",
]


def ms_to_iso(ts_ms: int) -> str:
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


def fetch_json(endpoint: str, params: dict[str, str | int]) -> dict:
    qs = urllib.parse.urlencode(params)
    url = f"{BASE_URL}{endpoint}?{qs}"
    headers = {"User-Agent": "vortex-alfa-track3/1.0"}
    max_attempts = 10
    for attempt in range(max_attempts):
        req = urllib.request.Request(url, headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=20) as resp:
                payload = json.loads(resp.read().decode("utf-8"))
            ret_code = payload.get("retCode", -1)
            if ret_code == 0:
                return payload
            ret_msg = payload.get("retMsg", "unknown")
            if ret_code in (10006, 10016) or attempt < max_attempts - 1:
                delay = min(30.0, 0.4 * (2 ** min(attempt, 6)) + random.uniform(0.05, 0.35))
                time.sleep(delay)
                continue
            raise RuntimeError(f"Bybit API error retCode={ret_code} retMsg={ret_msg} url={url}")
        except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError, json.JSONDecodeError) as exc:
            if attempt == max_attempts - 1:
                raise RuntimeError(f"HTTP fetch failed after {max_attempts} attempts for {url}: {exc}") from exc
            extra = 1.5 if isinstance(exc, urllib.error.HTTPError) and exc.code == 429 else 0.0
            delay = min(30.0, extra + 0.4 * (2 ** min(attempt, 6)) + random.uniform(0.05, 0.35))
            time.sleep(delay)
    raise RuntimeError(f"Unreachable retry exhaustion for {url}")


def fetch_klines(symbol: str) -> list[tuple[int, str, str, str, str, str]]:
    by_ts: dict[int, tuple[int, str, str, str, str, str]] = {}
    cur_end = END_MS - INTERVAL_MS
    while cur_end >= START_MS:
        payload = fetch_json(
            "/v5/market/kline",
            {
                "category": "linear",
                "symbol": symbol,
                "interval": INTERVAL,
                "start": START_MS,
                "end": cur_end,
                "limit": 1000,
            },
        )
        rows = payload.get("result", {}).get("list", [])
        if not rows:
            break
        oldest_ts: int | None = None
        for row in rows:
            open_ts = int(row[0])
            if oldest_ts is None or open_ts < oldest_ts:
                oldest_ts = open_ts
            if START_MS <= open_ts and open_ts + INTERVAL_MS <= END_MS:
                by_ts[open_ts] = (
                    open_ts,
                    str(row[1]).strip(),
                    str(row[2]).strip(),
                    str(row[3]).strip(),
                    str(row[4]).strip(),
                    str(row[5]).strip(),
                )
        if oldest_ts is None or oldest_ts <= START_MS or oldest_ts - 1 >= cur_end:
            break
        cur_end = oldest_ts - 1
        time.sleep(0.03)
    return [by_ts[k] for k in sorted(by_ts.keys())]


def fetch_funding(symbol: str) -> dict[int, Decimal]:
    by_ts: dict[int, Decimal] = {}
    cur_end = END_MS - 1
    while cur_end >= START_MS:
        payload = fetch_json(
            "/v5/market/funding/history",
            {
                "category": "linear",
                "symbol": symbol,
                "startTime": START_MS,
                "endTime": cur_end,
                "limit": 200,
            },
        )
        rows = payload.get("result", {}).get("list", [])
        if not rows:
            break
        oldest_ts: int | None = None
        for item in rows:
            ts_ms = int(item["fundingRateTimestamp"])
            if oldest_ts is None or ts_ms < oldest_ts:
                oldest_ts = ts_ms
            if START_MS <= ts_ms < END_MS:
                by_ts[ts_ms] = Decimal(str(item["fundingRate"]).strip())
        if oldest_ts is None or oldest_ts <= START_MS or oldest_ts - 1 >= cur_end:
            break
        cur_end = oldest_ts - 1
        time.sleep(0.03)
    return dict(sorted(by_ts.items()))


def write_csv_atomic(path: Path, rows: list[list[str | int]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(".csv.tmp")
    with tmp_path.open("w", encoding="utf-8", newline="\n") as fh:
        writer = csv.writer(fh, lineterminator="\n")
        writer.writerow(CSV_HEADER)
        writer.writerows(rows)
    tmp_path.replace(path)


def write_json_atomic(path: Path, data: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp_path = path.with_suffix(".json.tmp")
    with tmp_path.open("w", encoding="utf-8", newline="\n") as fh:
        json.dump(data, fh, indent=2)
        fh.write("\n")
    tmp_path.replace(path)


def load_validated_raw_funding(
    path: Path, symbol_id: int, symbol: str
) -> dict[int, Decimal] | None:
    if not path.exists():
        return None
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
        if payload.get("symbol_id") != symbol_id or payload.get("symbol") != symbol:
            return None
        if payload.get("window_start_ms") != START_MS or payload.get("window_end_ms") != END_MS:
            return None
        events = payload.get("events")
        if not isinstance(events, list) or payload.get("count") != len(events):
            return None
        fund_by_ts: dict[int, Decimal] = {}
        prev_ts: int | None = None
        for ev in events:
            ts_ms = int(ev["ts_ms"])
            if ts_ms < START_MS or ts_ms >= END_MS:
                return None
            if prev_ts is not None and ts_ms <= prev_ts:
                return None
            prev_ts = ts_ms
            rate_dec = Decimal(str(ev["funding_rate_decimal"]))
            rate_bps = Decimal(str(ev["funding_rate_bps"]))
            if rate_dec * Decimal("10000") != rate_bps:
                return None
            fund_by_ts[ts_ms] = rate_dec
        return fund_by_ts
    except (OSError, ValueError, KeyError, TypeError, InvalidOperation):
        return None


def load_validated_cached_4h_csv(
    path: Path, symbol_id: int, expected_bucket_bps_by_ts: dict[int, Decimal]
) -> list[tuple[int, str, str, str, str, str]] | None:
    if not path.exists():
        return None
    try:
        raw_bytes = path.read_bytes()
        if raw_bytes.startswith(b"\xef\xbb\xbf") or b"\r\n" in raw_bytes:
            return None
        candles: list[tuple[int, str, str, str, str, str]] = []
        prev_ts: int | None = None
        csv_funding_sum = Decimal("0")
        with path.open("r", encoding="utf-8", newline="") as fh:
            reader = csv.reader(fh)
            header = next(reader, None)
            if header != CSV_HEADER:
                return None
            for row in reader:
                if len(row) != len(CSV_HEADER):
                    return None
                if int(row[0]) != symbol_id:
                    return None
                ts_ms = int(row[1])
                if ts_ms < START_MS or ts_ms + INTERVAL_MS > END_MS:
                    return None
                if (ts_ms - START_MS) % INTERVAL_MS != 0:
                    return None
                if prev_ts is not None and ts_ms <= prev_ts:
                    return None
                prev_ts = ts_ms
                o, h, l, c, vol = (
                    float(row[2]),
                    float(row[3]),
                    float(row[4]),
                    float(row[5]),
                    float(row[6]),
                )
                if not all(math.isfinite(x) for x in (o, h, l, c, vol)):
                    return None
                if o <= 0.0 or h <= 0.0 or l <= 0.0 or c <= 0.0 or vol < 0.0:
                    return None
                if h < max(o, c) or l > min(o, c) or h < l:
                    return None
                f_bps = Decimal(row[7])
                expected_bps = expected_bucket_bps_by_ts.get(ts_ms, Decimal("0"))
                if f_bps != expected_bps:
                    return None
                csv_funding_sum += f_bps
                candles.append((ts_ms, row[2], row[3], row[4], row[5], row[6]))
        if not candles:
            return None
        # Only reuse cached 4h CSV if complete 4380-bar coverage and exact funding sum match.
        if len(candles) != EXPECTED_CANDLES:
            return None
        expected_sum = sum(expected_bucket_bps_by_ts.values(), Decimal("0"))
        if csv_funding_sum != expected_sum:
            return None
        return candles
    except (OSError, ValueError, InvalidOperation):
        return None


def aggregate_funding_causal_buckets(
    fund_by_ts: dict[int, Decimal]
) -> tuple[dict[int, Decimal], int, list[tuple[int, int, Decimal]]]:
    """Map each raw funding event onto causal half-open (bar_open_ts - 4h, bar_open_ts] buckets."""
    bucket_bps_by_ts: dict[int, Decimal] = {}
    subbar_aggregated_events = 0
    mapped_events: list[tuple[int, int, Decimal]] = []
    for f_ts, rate in fund_by_ts.items():
        rem = (f_ts - START_MS) % INTERVAL_MS
        target_open_ts = f_ts if rem == 0 else f_ts + (INTERVAL_MS - rem)
        if rem != 0:
            subbar_aggregated_events += 1
        f_bps = rate * Decimal("10000")
        bucket_bps_by_ts[target_open_ts] = (
            bucket_bps_by_ts.get(target_open_ts, Decimal("0")) + f_bps
        )
        mapped_events.append((f_ts, target_open_ts, f_bps))
    return bucket_bps_by_ts, subbar_aggregated_events, mapped_events


def build_raw_funding_payload(
    symbol_id: int, symbol: str, fund_by_ts: dict[int, Decimal]
) -> dict:
    fund_events = [
        {
            "ts_ms": ts_ms,
            "utc": ms_to_iso(ts_ms),
            "funding_rate_decimal": format_decimal(rate),
            "funding_rate_bps": format_decimal(rate * Decimal("10000")),
        }
        for ts_ms, rate in fund_by_ts.items()
    ]
    return {
        "symbol_id": symbol_id,
        "symbol": symbol,
        "window_start_ms": START_MS,
        "window_end_ms": END_MS,
        "count": len(fund_events),
        "events": fund_events,
    }


def process_symbol(
    symbol_id: int,
    symbol: str,
    data_root: Path,
    force: bool,
) -> dict:
    xs30_dir = data_root / "bybit_xs30_4h"
    xs30_y1_dir = data_root / "bybit_xs30_4h_y1"
    xs30_y2_dir = data_root / "bybit_xs30_4h_y2"

    raw_fund_path = xs30_dir / "raw_funding" / f"{symbol}.json"
    cache_fund_path = data_root / "bybit_24m" / "raw_funding" / f"{symbol}.json"

    p_full = xs30_dir / f"{symbol}.csv"
    p_y1 = xs30_y1_dir / f"{symbol}.csv"
    p_y2 = xs30_y2_dir / f"{symbol}.csv"
    cache_csv_path = data_root / "bybit_24m" / "4h" / f"{symbol}.csv"

    funding_source = "bybit_v5_api"
    fund_by_ts: dict[int, Decimal] | None = None
    if not force:
        fund_by_ts = load_validated_raw_funding(raw_fund_path, symbol_id, symbol)
        if fund_by_ts is not None:
            funding_source = "local_existing"
        else:
            fund_by_ts = load_validated_raw_funding(cache_fund_path, symbol_id, symbol)
            if fund_by_ts is not None:
                funding_source = "cache_bybit_24m"
                write_json_atomic(
                    raw_fund_path,
                    build_raw_funding_payload(symbol_id, symbol, fund_by_ts),
                )

    if fund_by_ts is None:
        fund_by_ts = fetch_funding(symbol)
        funding_source = "bybit_v5_api"
        write_json_atomic(
            raw_fund_path,
            build_raw_funding_payload(symbol_id, symbol, fund_by_ts),
        )

    fund_ts_list = sorted(fund_by_ts.keys())
    observed_fund_intervals_h = sorted(
        {
            round((fund_ts_list[i] - fund_ts_list[i - 1]) / 3_600_000.0, 6)
            for i in range(1, len(fund_ts_list))
        }
    )
    bucket_bps_by_ts, subbar_aggregated_events, mapped_events = (
        aggregate_funding_causal_buckets(fund_by_ts)
    )
    raw_funding_sum_bps = sum((f_bps for _, _, f_bps in mapped_events), Decimal("0"))

    kline_source = "bybit_v5_api"
    candles: list[tuple[int, str, str, str, str, str]] | None = None
    if not force:
        if p_full.exists() and p_y1.exists() and p_y2.exists():
            candles = load_validated_cached_4h_csv(p_full, symbol_id, bucket_bps_by_ts)
            if candles is not None:
                kline_source = "local_existing"
        if candles is None:
            candles = load_validated_cached_4h_csv(cache_csv_path, symbol_id, bucket_bps_by_ts)
            if candles is not None:
                kline_source = "cache_bybit_24m"

    if candles is None:
        candles = fetch_klines(symbol)
        kline_source = "bybit_v5_api"

    full_rows: list[list[str | int]] = []
    y1_rows: list[list[str | int]] = []
    y2_rows: list[list[str | int]] = []
    candle_ts_set: set[int] = set()
    csv_funding_sum_bps = Decimal("0")
    aligned_funding_buckets = 0

    for open_ts, o, h, l, c, vol in candles:
        candle_ts_set.add(open_ts)
        f_bps = bucket_bps_by_ts.get(open_ts, Decimal("0"))
        if open_ts in bucket_bps_by_ts:
            aligned_funding_buckets += 1
            csv_funding_sum_bps += f_bps
        row: list[str | int] = [
            symbol_id,
            open_ts,
            o,
            h,
            l,
            c,
            vol,
            format_decimal(f_bps),
        ]
        full_rows.append(row)
        if open_ts < SPLIT_MS:
            y1_rows.append(row)
        else:
            y2_rows.append(row)

    if force or kline_source != "local_existing" or not (p_full.exists() and p_y1.exists() and p_y2.exists()):
        write_csv_atomic(p_full, full_rows)
        write_csv_atomic(p_y1, y1_rows)
        write_csv_atomic(p_y2, y2_rows)

    gap_count = 0
    missing_bars_total = 0
    gap_details: list[dict[str, object]] = []
    for idx in range(1, len(candles)):
        diff = candles[idx][0] - candles[idx - 1][0]
        if diff > INTERVAL_MS:
            gap_count += 1
            missing = (diff // INTERVAL_MS) - 1
            missing_bars_total += missing
            gap_details.append(
                {
                    "after_ts_ms": candles[idx - 1][0],
                    "after_utc": ms_to_iso(candles[idx - 1][0]),
                    "before_ts_ms": candles[idx][0],
                    "before_utc": ms_to_iso(candles[idx][0]),
                    "missing_bars": missing,
                }
            )

    leading_missing_bars = (
        (candles[0][0] - START_MS) // INTERVAL_MS if candles else EXPECTED_CANDLES
    )
    trailing_missing_bars = (
        ((END_MS - INTERVAL_MS) - candles[-1][0]) // INTERVAL_MS if candles else 0
    )

    unmatched_events = []
    unmatched_events_count = 0
    for f_ts, target_open_ts, f_bps in mapped_events:
        if target_open_ts in candle_ts_set:
            continue
        unmatched_events_count += 1
        unmatched_events.append(
            {
                "ts_ms": f_ts,
                "utc": ms_to_iso(f_ts),
                "target_open_ts": target_open_ts,
                "target_open_utc": ms_to_iso(target_open_ts),
                "funding_rate_bps": format_decimal(f_bps),
                "classification": "unmatched_gap_or_post_end",
            }
        )
    aligned_funding_count = len(mapped_events) - unmatched_events_count

    min_ts = candles[0][0] if candles else None
    max_ts = candles[-1][0] if candles else None
    return {
        "symbol_id": symbol_id,
        "symbol": symbol,
        "funding_source": funding_source,
        "kline_source": kline_source,
        "candle_count": len(candles),
        "expected_candles": EXPECTED_CANDLES,
        "y1_count": len(y1_rows),
        "y2_count": len(y2_rows),
        "min_ts_ms": min_ts,
        "min_utc": ms_to_iso(min_ts) if min_ts is not None else None,
        "max_ts_ms": max_ts,
        "max_utc": ms_to_iso(max_ts) if max_ts is not None else None,
        "leading_missing_bars": leading_missing_bars,
        "trailing_missing_bars": trailing_missing_bars,
        "gap_count": gap_count,
        "missing_bars_total": missing_bars_total,
        "gaps_sample": gap_details[:10],
        "observed_funding_intervals_h": observed_fund_intervals_h,
        "raw_funding_count": len(fund_by_ts),
        "subbar_aggregated_events": subbar_aggregated_events,
        "aligned_funding_count": aligned_funding_count,
        "aligned_funding_buckets": aligned_funding_buckets,
        "unmatched_events_count": unmatched_events_count,
        "unmatched_events_sample": unmatched_events[:10],
        "raw_funding_sum_bps": format_decimal(raw_funding_sum_bps),
        "csv_funding_sum_bps": format_decimal(csv_funding_sum_bps),
        "reconciliation_diff_bps": format_decimal(raw_funding_sum_bps - csv_funding_sum_bps),
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Fetch 24m Bybit 4h klines & funding for the 30-pair Track 3 universe."
    )
    parser.add_argument("--data-root", default="data_local", help="Root output directory")
    parser.add_argument("--workers", type=int, default=5, help="Concurrent symbol workers")
    parser.add_argument("--force", action="store_true", help="Re-download existing CSV/JSON files")
    args = parser.parse_args()

    assert int(datetime(2024, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == START_MS
    assert int(datetime(2025, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == SPLIT_MS
    assert int(datetime(2026, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == END_MS

    t0 = time.monotonic()
    data_root = Path(args.data_root)
    results: dict[int, dict] = {}

    with ThreadPoolExecutor(max_workers=max(1, min(args.workers, 8))) as pool:
        future_map = {
            pool.submit(process_symbol, sym_id, sym, data_root, args.force): (sym_id, sym)
            for sym_id, sym in SYMBOLS
        }
        for fut in as_completed(future_map):
            sym_id, sym = future_map[fut]
            res = fut.result()
            results[sym_id] = res
            print(
                f"[{sym_id:02d}] {sym:10s} | "
                f"4h={res['candle_count']}/{EXPECTED_CANDLES} (y1={res['y1_count']}, y2={res['y2_count']}) | "
                f"gaps={res['gap_count']} | "
                f"fund_raw={res['raw_funding_count']} subbar={res['subbar_aggregated_events']} ints_h={res['observed_funding_intervals_h']} | "
                f"diff_bps={res['reconciliation_diff_bps']} | src={res['kline_source']}",
                flush=True,
            )

    ordered = [results[sym_id] for sym_id, _ in SYMBOLS]
    manifest = {
        "generated_utc": datetime.now(tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "timeframe": TF_NAME,
        "interval": INTERVAL,
        "interval_ms": INTERVAL_MS,
        "expected_candles": EXPECTED_CANDLES,
        "symbols_count": len(SYMBOLS),
        "funding_alignment": "BarOpenSettlement_CausalHalfOpen_(bar_open_ts-4h,bar_open_ts]",
        "window_start_ms": START_MS,
        "window_start_utc": ms_to_iso(START_MS),
        "split_ms": SPLIT_MS,
        "split_utc": ms_to_iso(SPLIT_MS),
        "window_end_ms": END_MS,
        "window_end_utc": ms_to_iso(END_MS),
        "elapsed_sec": round(time.monotonic() - t0, 2),
        "symbols": ordered,
    }
    manifest_path = data_root / "bybit_xs30_4h" / "download_manifest.json"
    write_json_atomic(manifest_path, manifest)
    print(f"Manifest written to {manifest_path} in {manifest['elapsed_sec']}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())

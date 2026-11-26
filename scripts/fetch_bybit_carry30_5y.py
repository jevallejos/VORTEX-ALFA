#!/usr/bin/env python3
"""Fetch 5 years of Bybit V5 public 1d linear + spot klines and linear funding for 30 pairs.

Strict Law compliance:
- Pure Python stdlib only.
- Performs zero signal, turnover, fee, borrow interest, Newey-West, or DSR computations (Law 1).
- Never synthesizes missing bars; preserves exact real Bybit listing dates and spot absences (Law 4).
- Contract-life causal funding bounding: when first_linear_open_ts > START_MS, retain in-life
  settlements ts_ms >= first_linear_open_ts and transparently log pre-listing API rows.
- Causal BarOpenSettlement funding buckets: (bar_open_ts - 24h, bar_open_ts].
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import csv
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation
import http.client
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

TF_NAME = "1d"
INTERVAL = "D"
INTERVAL_MS = 86_400_000

START_MS = 1637798400000     # 2021-11-25T00:00:00Z
SPLIT_MS = 1716595200000     # 2024-05-25T00:00:00Z -> R1=912d, R2=914d
COHORT24_MS = 1732492800000  # 2024-11-25T00:00:00Z -> 24m=730d
END_MS = 1795564800000       # 2026-11-25T00:00:00Z -> full=1826d (last open 2026-11-24T00:00:00Z)

EXPECTED_CANDLES = (END_MS - START_MS) // INTERVAL_MS          # 1826
EXPECTED_R1_CANDLES = (SPLIT_MS - START_MS) // INTERVAL_MS     # 912
EXPECTED_R2_CANDLES = (END_MS - SPLIT_MS) // INTERVAL_MS       # 914
EXPECTED_24M_CANDLES = (END_MS - COHORT24_MS) // INTERVAL_MS   # 730

# Causal BarOpenSettlement half-open bounds for contracts live at START_MS:
# First bar (START_MS) covers (START_MS - INTERVAL_MS, START_MS] -> > 2021-11-24T00:00:00Z.
# Last bar (END_MS - INTERVAL_MS) covers (END_MS - 2*INTERVAL_MS, END_MS - INTERVAL_MS] -> <= 2026-11-24T00:00:00Z.
FUND_START_MS = START_MS - INTERVAL_MS + 1  # 1637712000001
FUND_END_MS = END_MS - INTERVAL_MS          # 1795478400000

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
    headers = {"User-Agent": "vortex-alfa-track6/1.0"}
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
        except (
            urllib.error.HTTPError,
            urllib.error.URLError,
            http.client.HTTPException,
            OSError,
            TimeoutError,
            json.JSONDecodeError,
        ) as exc:
            if attempt == max_attempts - 1:
                raise RuntimeError(f"HTTP fetch failed after {max_attempts} attempts for {url}: {exc}") from exc
            extra = 1.5 if isinstance(exc, urllib.error.HTTPError) and exc.code == 429 else 0.0
            delay = min(30.0, extra + 0.4 * (2 ** min(attempt, 6)) + random.uniform(0.05, 0.35))
            time.sleep(delay)
    raise RuntimeError(f"Unreachable retry exhaustion for {url}")


def check_spot_listed_on_bybit(symbol: str) -> bool:
    """Confirm spot instrument listing via Bybit V5 instruments-info (never swallows network errors)."""
    payload = fetch_json(
        "/v5/market/instruments-info",
        {"category": "spot", "symbol": symbol},
    )
    items = payload.get("result", {}).get("list", [])
    return any(item.get("symbol") == symbol for item in items)


def fetch_klines(category: str, symbol: str) -> list[tuple[int, str, str, str, str, str]]:
    by_ts: dict[int, tuple[int, str, str, str, str, str]] = {}
    cur_end = END_MS - INTERVAL_MS
    while cur_end >= START_MS:
        payload = fetch_json(
            "/v5/market/kline",
            {
                "category": category,
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
        time.sleep(0.04)
    if not by_ts:
        raise RuntimeError(f"No klines returned for category={category} symbol={symbol}")
    return [by_ts[k] for k in sorted(by_ts.keys())]


def fetch_funding(symbol: str) -> dict[int, Decimal]:
    by_ts: dict[int, Decimal] = {}
    cur_end = FUND_END_MS
    while cur_end >= FUND_START_MS:
        payload = fetch_json(
            "/v5/market/funding/history",
            {
                "category": "linear",
                "symbol": symbol,
                "startTime": FUND_START_MS,
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
            if FUND_START_MS <= ts_ms <= FUND_END_MS:
                by_ts[ts_ms] = Decimal(str(item["fundingRate"]).strip())
        if oldest_ts is None or oldest_ts <= FUND_START_MS or oldest_ts - 1 >= cur_end:
            break
        cur_end = oldest_ts - 1
        time.sleep(0.04)
    if not by_ts:
        raise RuntimeError(f"No funding history returned for linear symbol={symbol}")
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


def serialize_funding_events(events_map: dict[int, Decimal]) -> list[dict]:
    return [
        {
            "ts_ms": ts_ms,
            "utc": ms_to_iso(ts_ms),
            "funding_rate_decimal": format_decimal(rate),
            "funding_rate_bps": format_decimal(rate * Decimal("10000")),
        }
        for ts_ms, rate in sorted(events_map.items())
    ]


def build_raw_funding_payload(
    symbol_id: int,
    symbol: str,
    first_linear_open_ts: int,
    effective_fund_start_ms: int,
    in_life_by_ts: dict[int, Decimal],
    pre_listing_by_ts: dict[int, Decimal],
) -> dict:
    fund_events = serialize_funding_events(in_life_by_ts)
    pre_events = serialize_funding_events(pre_listing_by_ts)
    pre_sum_bps = sum(
        (rate * Decimal("10000") for rate in pre_listing_by_ts.values()),
        Decimal("0"),
    )
    return {
        "symbol_id": symbol_id,
        "symbol": symbol,
        "window_start_ms": START_MS,
        "window_end_ms": END_MS,
        "funding_start_ms": FUND_START_MS,
        "funding_end_ms": FUND_END_MS,
        "first_linear_open_ms": first_linear_open_ts,
        "first_linear_open_utc": ms_to_iso(first_linear_open_ts),
        "effective_funding_start_ms": effective_fund_start_ms,
        "effective_funding_start_utc": ms_to_iso(effective_fund_start_ms),
        "pre_listing_filtered_count": len(pre_events),
        "pre_listing_filtered_sum_bps": format_decimal(pre_sum_bps),
        "pre_listing_filtered_events": pre_events,
        "count": len(fund_events),
        "events": fund_events,
    }


def load_validated_raw_funding(
    path: Path,
    symbol_id: int,
    symbol: str,
    expected_first_linear_open_ts: int,
    expected_effective_fund_start_ms: int,
) -> tuple[dict[int, Decimal], list[dict], Decimal] | None:
    if not path.exists():
        return None
    try:
        raw_bytes = path.read_bytes()
        if raw_bytes.startswith(b"\xef\xbb\xbf") or b"\r" in raw_bytes:
            return None
        payload = json.loads(raw_bytes.decode("utf-8"))
        if payload.get("symbol_id") != symbol_id or payload.get("symbol") != symbol:
            return None
        if payload.get("window_start_ms") != START_MS or payload.get("window_end_ms") != END_MS:
            return None
        if payload.get("funding_start_ms") != FUND_START_MS or payload.get("funding_end_ms") != FUND_END_MS:
            return None
        if payload.get("first_linear_open_ms") != expected_first_linear_open_ts:
            return None
        if payload.get("effective_funding_start_ms") != expected_effective_fund_start_ms:
            return None
        events = payload.get("events")
        if not isinstance(events, list) or not events or payload.get("count") != len(events):
            return None
        pre_events = payload.get("pre_listing_filtered_events")
        if not isinstance(pre_events, list) or payload.get("pre_listing_filtered_count") != len(pre_events):
            return None
        pre_sum_expected = Decimal(str(payload["pre_listing_filtered_sum_bps"]))
        pre_sum_calc = Decimal("0")
        prev_pre_ts: int | None = None
        for pev in pre_events:
            p_ts = int(pev["ts_ms"])
            if p_ts < FUND_START_MS or p_ts >= expected_effective_fund_start_ms:
                return None
            if prev_pre_ts is not None and p_ts <= prev_pre_ts:
                return None
            prev_pre_ts = p_ts
            p_dec = Decimal(str(pev["funding_rate_decimal"]))
            p_bps = Decimal(str(pev["funding_rate_bps"]))
            if not (math.isfinite(float(p_dec)) and math.isfinite(float(p_bps))):
                return None
            if p_dec * Decimal("10000") != p_bps:
                return None
            pre_sum_calc += p_bps
        if pre_sum_calc != pre_sum_expected:
            return None

        fund_by_ts: dict[int, Decimal] = {}
        prev_ts: int | None = None
        for ev in events:
            ts_ms = int(ev["ts_ms"])
            if ts_ms < expected_effective_fund_start_ms or ts_ms > FUND_END_MS:
                return None
            if prev_ts is not None and ts_ms <= prev_ts:
                return None
            prev_ts = ts_ms
            rate_dec = Decimal(str(ev["funding_rate_decimal"]))
            rate_bps = Decimal(str(ev["funding_rate_bps"]))
            if not (math.isfinite(float(rate_dec)) and math.isfinite(float(rate_bps))):
                return None
            if rate_dec * Decimal("10000") != rate_bps:
                return None
            fund_by_ts[ts_ms] = rate_dec
        return fund_by_ts, pre_events, pre_sum_expected
    except (OSError, ValueError, KeyError, TypeError, InvalidOperation):
        return None


def is_valid_header_only_csv(path: Path) -> bool:
    if not path.exists():
        return False
    try:
        raw_bytes = path.read_bytes()
        if raw_bytes.startswith(b"\xef\xbb\xbf") or b"\r" in raw_bytes:
            return False
        with path.open("r", encoding="utf-8", newline="") as fh:
            reader = csv.reader(fh)
            header = next(reader, None)
            if header != CSV_HEADER:
                return False
            if next(reader, None) is not None:
                return False
        return True
    except OSError:
        return False


def load_validated_cached_csv(
    path: Path,
    symbol_id: int,
    expected_bucket_bps_by_ts: dict[int, Decimal] | None,
    check_funding: bool = True,
) -> list[tuple[int, str, str, str, str, str]] | None:
    if not path.exists():
        return None
    try:
        raw_bytes = path.read_bytes()
        if raw_bytes.startswith(b"\xef\xbb\xbf") or b"\r" in raw_bytes:
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
                if prev_ts is not None:
                    if ts_ms <= prev_ts or (ts_ms - prev_ts) != INTERVAL_MS:
                        return None
                prev_ts = ts_ms
                o_dec, h_dec, l_dec, c_dec, v_dec = (
                    Decimal(row[2]),
                    Decimal(row[3]),
                    Decimal(row[4]),
                    Decimal(row[5]),
                    Decimal(row[6]),
                )
                o, h, l, c, vol = (
                    float(o_dec),
                    float(h_dec),
                    float(l_dec),
                    float(c_dec),
                    float(v_dec),
                )
                if not all(math.isfinite(x) for x in (o, h, l, c, vol)):
                    return None
                if o <= 0.0 or h <= 0.0 or l <= 0.0 or c <= 0.0 or vol < 0.0:
                    return None
                if h_dec < max(o_dec, c_dec) or l_dec > min(o_dec, c_dec) or h_dec < l_dec:
                    return None
                f_bps = Decimal(row[7])
                if check_funding:
                    if expected_bucket_bps_by_ts is None:
                        if f_bps != Decimal("0"):
                            return None
                    else:
                        expected_bps = expected_bucket_bps_by_ts.get(ts_ms, Decimal("0"))
                        if f_bps != expected_bps:
                            return None
                csv_funding_sum += f_bps
                candles.append((ts_ms, row[2], row[3], row[4], row[5], row[6]))
        if not candles:
            return None
        # Allow leading missing bars from real listing dates; require zero trailing missing bars.
        if candles[-1][0] != END_MS - INTERVAL_MS:
            return None
        if check_funding and expected_bucket_bps_by_ts is not None:
            expected_sum = sum(expected_bucket_bps_by_ts.values(), Decimal("0"))
            if csv_funding_sum != expected_sum:
                return None
        return candles
    except (OSError, ValueError, InvalidOperation):
        return None


def aggregate_funding_causal_buckets(
    fund_by_ts: dict[int, Decimal]
) -> tuple[dict[int, Decimal], int, list[tuple[int, int, Decimal]]]:
    """Map raw funding events onto causal half-open (bar_open_ts - 24h, bar_open_ts] buckets."""
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


def compute_series_stats(
    candles: list[tuple[int, str, str, str, str, str]],
) -> tuple[int, int, int, int, list[dict[str, object]]]:
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
    return leading_missing_bars, trailing_missing_bars, gap_count, missing_bars_total, gap_details


def write_split_csvs(
    symbol_id: int,
    candles: list[tuple[int, str, str, str, str, str]],
    bucket_bps_by_ts: dict[int, Decimal] | None,
    p_full: Path,
    p_r1: Path,
    p_r2: Path,
    p_24m: Path,
    force_write: bool,
) -> tuple[set[int], int, int, int, Decimal, int]:
    full_rows: list[list[str | int]] = []
    r1_rows: list[list[str | int]] = []
    r2_rows: list[list[str | int]] = []
    c24_rows: list[list[str | int]] = []
    ts_set: set[int] = set()
    csv_funding_sum_bps = Decimal("0")
    aligned_funding_buckets = 0

    for open_ts, o, h, l, c, vol in candles:
        ts_set.add(open_ts)
        if bucket_bps_by_ts is not None:
            f_bps = bucket_bps_by_ts.get(open_ts, Decimal("0"))
            if open_ts in bucket_bps_by_ts:
                aligned_funding_buckets += 1
                csv_funding_sum_bps += f_bps
            f_str = format_decimal(f_bps)
        else:
            f_str = "0"
        row: list[str | int] = [symbol_id, open_ts, o, h, l, c, vol, f_str]
        full_rows.append(row)
        if open_ts < SPLIT_MS:
            r1_rows.append(row)
        else:
            r2_rows.append(row)
        if open_ts >= COHORT24_MS:
            c24_rows.append(row)

    if force_write or not (p_full.exists() and p_r1.exists() and p_r2.exists() and p_24m.exists()):
        write_csv_atomic(p_full, full_rows)
        write_csv_atomic(p_r1, r1_rows)
        write_csv_atomic(p_r2, r2_rows)
        write_csv_atomic(p_24m, c24_rows)

    return ts_set, len(r1_rows), len(r2_rows), len(c24_rows), csv_funding_sum_bps, aligned_funding_buckets


def process_symbol(
    symbol_id: int,
    symbol: str,
    data_root: Path,
    force: bool,
) -> dict:
    dir_full = data_root / "bybit_carry30_5y"
    dir_r1 = data_root / "bybit_carry30_r1"
    dir_r2 = data_root / "bybit_carry30_r2"
    dir_24m = data_root / "bybit_carry30_24m"

    raw_fund_path = dir_full / "raw_funding" / f"{symbol}.json"

    lin_full = dir_full / "linear" / f"{symbol}.csv"
    lin_r1 = dir_r1 / "linear" / f"{symbol}.csv"
    lin_r2 = dir_r2 / "linear" / f"{symbol}.csv"
    lin_24m = dir_24m / "linear" / f"{symbol}.csv"

    spot_full = dir_full / "spot" / f"{symbol}.csv"
    spot_r1 = dir_r1 / "spot" / f"{symbol}.csv"
    spot_r2 = dir_r2 / "spot" / f"{symbol}.csv"
    spot_24m = dir_24m / "spot" / f"{symbol}.csv"

    # Step 1: Load/fetch linear klines first so contract-life funding bound is known.
    lin_source = "bybit_v5_api"
    lin_candles: list[tuple[int, str, str, str, str, str]] | None = None
    if not force and lin_full.exists() and lin_r1.exists() and lin_r2.exists() and lin_24m.exists():
        lin_candles = load_validated_cached_csv(lin_full, symbol_id, None, check_funding=False)
        if lin_candles is not None:
            lin_source = "local_existing"
    if lin_candles is None:
        lin_candles = fetch_klines("linear", symbol)
        lin_source = "bybit_v5_api"

    first_linear_open_ts = lin_candles[0][0]
    effective_fund_start_ms = (
        first_linear_open_ts if first_linear_open_ts > START_MS else FUND_START_MS
    )

    # Step 2: Load/fetch funding and apply contract-life causal bound while logging pre-listing rows.
    funding_source = "bybit_v5_api"
    fund_by_ts: dict[int, Decimal] | None = None
    pre_listing_events: list[dict] = []
    pre_listing_sum_bps = Decimal("0")

    if not force:
        loaded_fund = load_validated_raw_funding(
            raw_fund_path, symbol_id, symbol, first_linear_open_ts, effective_fund_start_ms
        )
        if loaded_fund is not None:
            fund_by_ts, pre_listing_events, pre_listing_sum_bps = loaded_fund
            funding_source = "local_existing"

    if fund_by_ts is None:
        all_fund_by_ts = fetch_funding(symbol)
        funding_source = "bybit_v5_api"
        in_life_by_ts = {
            ts: rate for ts, rate in all_fund_by_ts.items() if ts >= effective_fund_start_ms
        }
        pre_listing_by_ts = {
            ts: rate for ts, rate in all_fund_by_ts.items() if ts < effective_fund_start_ms
        }
        if not in_life_by_ts:
            raise RuntimeError(
                f"No in-life funding events remain for symbol={symbol} after effective_fund_start_ms={effective_fund_start_ms}"
            )
        payload = build_raw_funding_payload(
            symbol_id,
            symbol,
            first_linear_open_ts,
            effective_fund_start_ms,
            in_life_by_ts,
            pre_listing_by_ts,
        )
        write_json_atomic(raw_fund_path, payload)
        fund_by_ts = in_life_by_ts
        pre_listing_events = payload["pre_listing_filtered_events"]
        pre_listing_sum_bps = Decimal(payload["pre_listing_filtered_sum_bps"])

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

    # Confirm cached linear CSV funding column matches current buckets; rewrite splits if needed.
    lin_force_write = force or lin_source != "local_existing"
    if not lin_force_write:
        if load_validated_cached_csv(lin_full, symbol_id, bucket_bps_by_ts, check_funding=True) is None:
            lin_force_write = True

    # Step 3: Verify Bybit spot listing via instruments-info before spot kline download.
    spot_listed = check_spot_listed_on_bybit(symbol)
    spot_candles: list[tuple[int, str, str, str, str, str]] | None = None
    if spot_listed:
        spot_source = "bybit_v5_api"
        if not force and spot_full.exists() and spot_r1.exists() and spot_r2.exists() and spot_24m.exists():
            spot_candles = load_validated_cached_csv(spot_full, symbol_id, None, check_funding=True)
            if spot_candles is not None:
                spot_source = "local_existing"
        if spot_candles is None:
            spot_candles = fetch_klines("spot", symbol)
            spot_source = "bybit_v5_api"
        spot_force_write = force or spot_source != "local_existing"
    else:
        spot_candles = []
        if (
            not force
            and is_valid_header_only_csv(spot_full)
            and is_valid_header_only_csv(spot_r1)
            and is_valid_header_only_csv(spot_r2)
            and is_valid_header_only_csv(spot_24m)
        ):
            spot_source = "local_existing_header_only"
            spot_force_write = False
        else:
            spot_source = "bybit_spot_absent_header_only"
            spot_force_write = True

    lin_ts_set, lin_r1_cnt, lin_r2_cnt, lin_24m_cnt, lin_csv_sum_bps, lin_aligned_buckets = write_split_csvs(
        symbol_id,
        lin_candles,
        bucket_bps_by_ts,
        lin_full,
        lin_r1,
        lin_r2,
        lin_24m,
        force_write=lin_force_write,
    )
    spot_ts_set, spot_r1_cnt, spot_r2_cnt, spot_24m_cnt, _, _ = write_split_csvs(
        symbol_id,
        spot_candles,
        None,
        spot_full,
        spot_r1,
        spot_r2,
        spot_24m,
        force_write=spot_force_write,
    )

    lin_lead, lin_trail, lin_gaps, lin_missing, lin_gap_details = compute_series_stats(lin_candles)
    spot_lead, spot_trail, spot_gaps, spot_missing, spot_gap_details = compute_series_stats(spot_candles)

    unmatched_events = []
    unmatched_events_count = 0
    for f_ts, target_open_ts, f_bps in mapped_events:
        if target_open_ts in lin_ts_set:
            continue
        unmatched_events_count += 1
        unmatched_events.append(
            {
                "ts_ms": f_ts,
                "utc": ms_to_iso(f_ts),
                "target_open_ts": target_open_ts,
                "target_open_utc": ms_to_iso(target_open_ts),
                "funding_rate_bps": format_decimal(f_bps),
                "classification": "unmatched_gap_or_out_of_range",
            }
        )
    aligned_funding_count = len(mapped_events) - unmatched_events_count

    overlap_ts = sorted(lin_ts_set & spot_ts_set)
    dual_overlap_days = len(overlap_ts)
    dual_overlap_r1_days = sum(1 for ts in overlap_ts if ts < SPLIT_MS)
    dual_overlap_r2_days = sum(1 for ts in overlap_ts if ts >= SPLIT_MS)
    dual_overlap_24m_days = sum(1 for ts in overlap_ts if ts >= COHORT24_MS)

    lin_min_ts = lin_candles[0][0]
    lin_max_ts = lin_candles[-1][0]
    spot_min_ts = spot_candles[0][0] if spot_candles else None
    spot_max_ts = spot_candles[-1][0] if spot_candles else None

    reconciliation_diff_bps = raw_funding_sum_bps - lin_csv_sum_bps

    return {
        "symbol_id": symbol_id,
        "symbol": symbol,
        "spot_listed_on_bybit": spot_listed,
        "dual_overlap_days": dual_overlap_days,
        "dual_overlap_r1_days": dual_overlap_r1_days,
        "dual_overlap_r2_days": dual_overlap_r2_days,
        "dual_overlap_24m_days": dual_overlap_24m_days,
        "dual_overlap_min_utc": ms_to_iso(overlap_ts[0]) if overlap_ts else None,
        "dual_overlap_max_utc": ms_to_iso(overlap_ts[-1]) if overlap_ts else None,
        "linear": {
            "source": lin_source,
            "funding_source": funding_source,
            "candle_count": len(lin_candles),
            "expected_candles": EXPECTED_CANDLES,
            "coverage_pct": round((len(lin_candles) / EXPECTED_CANDLES) * 100.0, 4),
            "r1_count": lin_r1_cnt,
            "r2_count": lin_r2_cnt,
            "c24_count": lin_24m_cnt,
            "c24_coverage_pct": round((lin_24m_cnt / EXPECTED_24M_CANDLES) * 100.0, 4),
            "min_ts_ms": lin_min_ts,
            "min_utc": ms_to_iso(lin_min_ts),
            "max_ts_ms": lin_max_ts,
            "max_utc": ms_to_iso(lin_max_ts),
            "leading_missing_bars": lin_lead,
            "trailing_missing_bars": lin_trail,
            "gap_count": lin_gaps,
            "missing_bars_total": lin_missing,
            "gaps_sample": lin_gap_details[:10],
            "dual_overlap_days": dual_overlap_days,
            "effective_funding_start_ms": effective_fund_start_ms,
            "effective_funding_start_utc": ms_to_iso(effective_fund_start_ms),
            "pre_listing_filtered_count": len(pre_listing_events),
            "pre_listing_filtered_sum_bps": format_decimal(pre_listing_sum_bps),
            "pre_listing_filtered_events_sample": pre_listing_events[:25],
            "raw_funding_count": len(fund_by_ts),
            "subbar_aggregated_events": subbar_aggregated_events,
            "aligned_funding_count": aligned_funding_count,
            "aligned_funding_buckets": lin_aligned_buckets,
            "unmatched_events_count": unmatched_events_count,
            "unmatched_events_sample": unmatched_events[:10],
            "raw_funding_sum_bps": format_decimal(raw_funding_sum_bps),
            "csv_funding_sum_bps": format_decimal(lin_csv_sum_bps),
            "reconciliation_diff_bps": format_decimal(reconciliation_diff_bps),
            "observed_funding_intervals_h": observed_fund_intervals_h,
        },
        "spot": {
            "spot_listed_on_bybit": spot_listed,
            "source": spot_source,
            "candle_count": len(spot_candles),
            "expected_candles": EXPECTED_CANDLES,
            "coverage_pct": round((len(spot_candles) / EXPECTED_CANDLES) * 100.0, 4) if spot_candles else 0.0,
            "r1_count": spot_r1_cnt,
            "r2_count": spot_r2_cnt,
            "c24_count": spot_24m_cnt,
            "c24_coverage_pct": round((spot_24m_cnt / EXPECTED_24M_CANDLES) * 100.0, 4) if spot_candles else 0.0,
            "min_ts_ms": spot_min_ts,
            "min_utc": ms_to_iso(spot_min_ts) if spot_min_ts is not None else None,
            "max_ts_ms": spot_max_ts,
            "max_utc": ms_to_iso(spot_max_ts) if spot_max_ts is not None else None,
            "leading_missing_bars": spot_lead,
            "trailing_missing_bars": spot_trail,
            "gap_count": spot_gaps,
            "missing_bars_total": spot_missing,
            "gaps_sample": spot_gap_details[:10],
            "dual_overlap_days": dual_overlap_days,
            "raw_funding_count": 0,
            "reconciliation_diff_bps": "0",
            "observed_funding_intervals_h": [],
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Fetch 5y Bybit 1d linear+spot klines and funding for Track 6 (30 pairs)."
    )
    parser.add_argument("--data-root", default="data_local", help="Root output directory")
    parser.add_argument("--workers", type=int, default=4, help="Concurrent symbol workers")
    parser.add_argument("--force", action="store_true", help="Re-download existing CSV/JSON files")
    args = parser.parse_args()

    assert int(datetime(2021, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == START_MS
    assert int(datetime(2024, 5, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == SPLIT_MS
    assert int(datetime(2024, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == COHORT24_MS
    assert int(datetime(2026, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == END_MS
    assert EXPECTED_CANDLES == 1826
    assert EXPECTED_R1_CANDLES == 912
    assert EXPECTED_R2_CANDLES == 914
    assert EXPECTED_24M_CANDLES == 730
    assert FUND_START_MS == 1637712000001
    assert FUND_END_MS == 1795478400000

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
            lin = res["linear"]
            spot = res["spot"]
            spot_desc = (
                f"spot={spot['candle_count']}/{EXPECTED_CANDLES} (r1={spot['r1_count']}, r2={spot['r2_count']}, 24m={spot['c24_count']}, {spot['min_utc']}..{spot['max_utc']}, gaps={spot['gap_count']})"
                if res["spot_listed_on_bybit"]
                else "spot=0/1826 (NOT_LISTED_ON_BYBIT_SPOT, header_only)"
            )
            print(
                f"[{sym_id:02d}] {sym:10s} | "
                f"lin={lin['candle_count']}/{EXPECTED_CANDLES} (r1={lin['r1_count']}, r2={lin['r2_count']}, 24m={lin['c24_count']}, {lin['min_utc']}..{lin['max_utc']}, gaps={lin['gap_count']}) | "
                f"{spot_desc} | "
                f"overlap={res['dual_overlap_days']}d (24m={res['dual_overlap_24m_days']}d) | "
                f"fund_raw={lin['raw_funding_count']} pre_filt={lin['pre_listing_filtered_count']} unmatched={lin['unmatched_events_count']} diff_bps={lin['reconciliation_diff_bps']}",
                flush=True,
            )

    ordered = [results[sym_id] for sym_id, _ in SYMBOLS]
    dual_listed_count = sum(1 for r in ordered if r["spot_listed_on_bybit"])
    perp_only_symbols = [r["symbol"] for r in ordered if not r["spot_listed_on_bybit"]]
    manifest = {
        "generated_utc": datetime.now(tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "timeframe": TF_NAME,
        "interval": INTERVAL,
        "interval_ms": INTERVAL_MS,
        "expected_candles": EXPECTED_CANDLES,
        "expected_r1_candles": EXPECTED_R1_CANDLES,
        "expected_r2_candles": EXPECTED_R2_CANDLES,
        "expected_24m_candles": EXPECTED_24M_CANDLES,
        "symbols_count": len(SYMBOLS),
        "dual_listed_symbols_count": dual_listed_count,
        "perp_only_symbols": perp_only_symbols,
        "funding_alignment": "BarOpenSettlement_CausalHalfOpen_(bar_open_ts-24h,bar_open_ts]_ContractLifeBounded",
        "window_start_ms": START_MS,
        "window_start_utc": ms_to_iso(START_MS),
        "split_ms": SPLIT_MS,
        "split_utc": ms_to_iso(SPLIT_MS),
        "cohort24_ms": COHORT24_MS,
        "cohort24_utc": ms_to_iso(COHORT24_MS),
        "window_end_ms": END_MS,
        "window_end_utc": ms_to_iso(END_MS),
        "funding_start_ms": FUND_START_MS,
        "funding_start_utc": ms_to_iso(FUND_START_MS),
        "funding_end_ms": FUND_END_MS,
        "funding_end_utc": ms_to_iso(FUND_END_MS),
        "elapsed_sec": round(time.monotonic() - t0, 2),
        "symbols": ordered,
    }
    manifest_path = data_root / "bybit_carry30_5y" / "download_manifest.json"
    write_json_atomic(manifest_path, manifest)
    print(f"Manifest written to {manifest_path} in {manifest['elapsed_sec']}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())

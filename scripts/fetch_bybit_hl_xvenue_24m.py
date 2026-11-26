#!/usr/bin/env python3
"""Fetch and align 24 months of 4h cross-venue Bybit + Hyperliquid perpetual datasets.

Strict Law compliance:
- Pure Python stdlib only (0 pip dependencies).
- Law 1: Only downloads, validates, remaps symbol_id (0..15), aggregates causal
  BarOpenSettlement (bar_open_ts - 4h, bar_open_ts] funding buckets in Decimal,
  splits 24m/y1/y2, and builds manifest. Zero signals, turnover, fees, NW, or DSR.
- Law 4: Never synthesizes prices or pads missing bars.
"""

from __future__ import annotations

import argparse
from collections import deque
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

TF_NAME = "4h"
BYBIT_INTERVAL = "240"
HL_INTERVAL = "4h"
INTERVAL_MS = 14_400_000
HOUR_MS = 3_600_000

START_MS = 1732492800000           # 2024-11-25T00:00:00Z
SPLIT_MS = 1764028800000           # 2025-11-25T00:00:00Z
END_MS = 1795564800000             # 2026-11-25T00:00:00Z
LAST_BAR_OPEN_MS = 1795550400000   # 2026-11-24T20:00:00Z
EXPECTED_CANDLES = (END_MS - START_MS) // INTERVAL_MS  # 4380

HL_FUNDING_QUERY_START_MS = START_MS - 3 * HOUR_MS - 60_000   # 2024-11-24T20:59:00Z
HL_FUNDING_QUERY_END_MS = LAST_BAR_OPEN_MS + 60_000           # 2026-11-24T20:01:00Z
HL_FUNDING_KEEP_MIN_MS = START_MS - 3 * HOUR_MS               # 2024-11-24T21:00:00Z
# Nearest-hour Voronoi radius (< 30 min = 1,800,000 ms) safely handles real HL validator
# delays (e.g. 2025-07-19 +887.6s, 2025-07-27 +111.0s) while keeping unique snapped UTC hours.
MAX_JITTER_MS = 1_799_999
WARN_JITTER_MS = 60_000

BYBIT_BASE_URL = "https://api.bybit.com"
HL_INFO_URL = "https://api.hyperliquid.xyz/info"

HL_RATE_MAX_WEIGHT = 860
HL_RATE_WINDOW_SEC = 61.0
HL_CANDLE_PAGE_LIMIT = 5000
HL_FUNDING_PAGE_LIMIT = 500

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


def validate_ohlcv_tuple(
    o_raw: str, h_raw: str, l_raw: str, c_raw: str, v_raw: str
) -> bool:
    try:
        o_dec = Decimal(o_raw)
        h_dec = Decimal(h_raw)
        l_dec = Decimal(l_raw)
        c_dec = Decimal(c_raw)
        v_dec = Decimal(v_raw)
        o, h, l, c, vol = float(o_dec), float(h_dec), float(l_dec), float(c_dec), float(v_dec)
    except (ValueError, InvalidOperation):
        return False
    if not all(math.isfinite(x) for x in (o, h, l, c, vol)):
        return False
    if (
        o <= 0.0
        or h <= 0.0
        or l <= 0.0
        or c <= 0.0
        or round(o * 1e8) < 1
        or round(h * 1e8) < 1
        or round(l * 1e8) < 1
        or round(c * 1e8) < 1
        or vol < 0.0
    ):
        return False
    if h_dec < max(o_dec, c_dec) or l_dec > min(o_dec, c_dec) or h_dec < l_dec:
        return False
    return True


def aggregate_funding_causal_buckets(
    fund_by_ts: dict[int, Decimal]
) -> tuple[dict[int, Decimal], int, list[tuple[int, int, Decimal]]]:
    """Map each funding event onto causal half-open (bar_open_ts - 4h, bar_open_ts] buckets."""
    bucket_bps_by_ts: dict[int, Decimal] = {}
    subbar_aggregated_events = 0
    mapped_events: list[tuple[int, int, Decimal]] = []
    for f_ts, rate in sorted(fund_by_ts.items()):
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


def build_raw_bybit_funding_payload(
    symbol_id: int, symbol: str, fund_by_ts: dict[int, Decimal]
) -> dict:
    fund_events = [
        {
            "ts_ms": ts_ms,
            "utc": ms_to_iso(ts_ms),
            "funding_rate_decimal": format_decimal(rate),
            "funding_rate_bps": format_decimal(rate * Decimal("10000")),
        }
        for ts_ms, rate in sorted(fund_by_ts.items())
    ]
    return {
        "symbol_id": symbol_id,
        "symbol": symbol,
        "window_start_ms": START_MS,
        "window_end_ms": END_MS,
        "count": len(fund_events),
        "events": fund_events,
    }


def fetch_bybit_json(endpoint: str, params: dict[str, str | int]) -> dict:
    qs = urllib.parse.urlencode(params)
    url = f"{BYBIT_BASE_URL}{endpoint}?{qs}"
    headers = {"User-Agent": "vortex-alfa-track7/1.0"}
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
        except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError, OSError, json.JSONDecodeError) as exc:
            if attempt == max_attempts - 1:
                raise RuntimeError(f"Bybit HTTP fetch failed after {max_attempts} attempts for {url}: {exc}") from exc
            extra = 1.5 if isinstance(exc, urllib.error.HTTPError) and exc.code == 429 else 0.0
            delay = min(30.0, extra + 0.4 * (2 ** min(attempt, 6)) + random.uniform(0.05, 0.35))
            time.sleep(delay)
    raise RuntimeError(f"Unreachable retry exhaustion for {url}")


def fetch_bybit_klines(symbol: str) -> list[tuple[int, str, str, str, str, str]]:
    by_ts: dict[int, tuple[int, str, str, str, str, str]] = {}
    cur_end = LAST_BAR_OPEN_MS
    while cur_end >= START_MS:
        payload = fetch_bybit_json(
            "/v5/market/kline",
            {
                "category": "linear",
                "symbol": symbol,
                "interval": BYBIT_INTERVAL,
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
            if START_MS <= open_ts <= LAST_BAR_OPEN_MS and (open_ts - START_MS) % INTERVAL_MS == 0:
                o_s, h_s, l_s, c_s, v_s = (
                    str(row[1]).strip(),
                    str(row[2]).strip(),
                    str(row[3]).strip(),
                    str(row[4]).strip(),
                    str(row[5]).strip(),
                )
                if not validate_ohlcv_tuple(o_s, h_s, l_s, c_s, v_s):
                    raise RuntimeError(f"Invalid Bybit kline OHLCV for {symbol} at {open_ts}")
                by_ts[open_ts] = (open_ts, o_s, h_s, l_s, c_s, v_s)
        if oldest_ts is None or oldest_ts <= START_MS or oldest_ts - 1 >= cur_end:
            break
        cur_end = oldest_ts - 1
        time.sleep(0.05)
    return [by_ts[k] for k in sorted(by_ts.keys())]


def fetch_bybit_funding(symbol: str) -> dict[int, Decimal]:
    by_ts: dict[int, Decimal] = {}
    cur_end = END_MS - 1
    while cur_end >= START_MS:
        payload = fetch_bybit_json(
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
        time.sleep(0.05)
    return dict(sorted(by_ts.items()))


def load_validated_raw_bybit_funding(
    path: Path, symbol: str, required_symbol_id: int | None
) -> dict[int, Decimal] | None:
    if not path.exists():
        return None
    try:
        raw_bytes = path.read_bytes()
        if raw_bytes.startswith(b"\xef\xbb\xbf") or b"\r" in raw_bytes:
            return None
        payload = json.loads(raw_bytes.decode("utf-8"))
        if not isinstance(payload, dict):
            return None
        if required_symbol_id is not None and payload.get("symbol_id") != required_symbol_id:
            return None
        if not isinstance(payload.get("symbol_id"), int):
            return None
        if payload.get("symbol") != symbol:
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
    except (OSError, UnicodeDecodeError, ValueError, KeyError, TypeError, InvalidOperation, json.JSONDecodeError):
        return None


def load_validated_bybit_csv(
    path: Path,
    required_symbol_id: int | None,
    expected_bucket_bps_by_ts: dict[int, Decimal],
    require_full_4380: bool = True,
) -> list[tuple[int, str, str, str, str, str]] | None:
    if not path.exists():
        return None
    try:
        raw_bytes = path.read_bytes()
        if raw_bytes.startswith(b"\xef\xbb\xbf") or b"\r" in raw_bytes:
            return None
        candles: list[tuple[int, str, str, str, str, str]] = []
        prev_ts: int | None = None
        observed_sym_id: int | None = required_symbol_id
        csv_funding_sum = Decimal("0")
        with path.open("r", encoding="utf-8", newline="") as fh:
            reader = csv.reader(fh)
            header = next(reader, None)
            if header != CSV_HEADER:
                return None
            for row in reader:
                if len(row) != len(CSV_HEADER):
                    return None
                row_sym_id = int(row[0])
                if observed_sym_id is None:
                    observed_sym_id = row_sym_id
                elif row_sym_id != observed_sym_id:
                    return None
                ts_ms = int(row[1])
                if ts_ms < START_MS or ts_ms > LAST_BAR_OPEN_MS:
                    return None
                if (ts_ms - START_MS) % INTERVAL_MS != 0:
                    return None
                if prev_ts is not None and ts_ms <= prev_ts:
                    return None
                prev_ts = ts_ms
                if not validate_ohlcv_tuple(row[2], row[3], row[4], row[5], row[6]):
                    return None
                f_bps = Decimal(row[7])
                expected_bps = expected_bucket_bps_by_ts.get(ts_ms, Decimal("0"))
                if f_bps != expected_bps:
                    return None
                csv_funding_sum += f_bps
                candles.append((ts_ms, row[2], row[3], row[4], row[5], row[6]))
        if not candles:
            return None
        if require_full_4380 and len(candles) != EXPECTED_CANDLES:
            return None
        expected_sum = sum(expected_bucket_bps_by_ts.values(), Decimal("0"))
        if csv_funding_sum != expected_sum:
            return None
        return candles
    except (OSError, UnicodeDecodeError, ValueError, InvalidOperation):
        return None


class SlidingWindowRateLimiter:
    """Sliding-window API weight limiter strictly enforcing <= max_weight per window_sec."""

    def __init__(self, max_weight: int = HL_RATE_MAX_WEIGHT, window_sec: float = HL_RATE_WINDOW_SEC) -> None:
        self.max_weight = max_weight
        self.window_sec = window_sec
        self.events: deque[tuple[float, int]] = deque()
        self.total_weight_used = 0

    def _prune(self, now: float) -> int:
        cutoff = now - self.window_sec
        while self.events and self.events[0][0] <= cutoff:
            self.events.popleft()
        return sum(w for _, w in self.events)

    def wait_for_budget(self, est_weight: int, label: str = "") -> None:
        while True:
            now = time.monotonic()
            used = self._prune(now)
            if used + est_weight <= self.max_weight:
                return
            if not self.events:
                return
            oldest_ts = self.events[0][0]
            sleep_for = max(0.05, (oldest_ts + self.window_sec + 0.05) - now)
            if sleep_for >= 2.0 and label:
                print(
                    f"  [rate-limit] {label}: used={used}/{self.max_weight} in {self.window_sec:.0f}s, sleeping {sleep_for:.1f}s...",
                    flush=True,
                )
            time.sleep(sleep_for)

    def record(self, actual_weight: int) -> None:
        now = time.monotonic()
        self._prune(now)
        self.events.append((now, actual_weight))
        self.total_weight_used += actual_weight


def post_hl_info(
    payload: dict,
    est_weight: int,
    weight_fn,
    limiter: SlidingWindowRateLimiter,
    label: str = "",
):
    body = json.dumps(payload).encode("utf-8")
    headers = {
        "Content-Type": "application/json",
        "User-Agent": "vortex-alfa-track7/1.0",
    }
    max_attempts = 10
    for attempt in range(max_attempts):
        limiter.wait_for_budget(est_weight, label=label)
        req = urllib.request.Request(HL_INFO_URL, data=body, headers=headers, method="POST")
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                data = json.loads(resp.read().decode("utf-8"))
            actual_weight = weight_fn(data)
            limiter.record(actual_weight)
            return data
        except urllib.error.HTTPError as exc:
            limiter.record(est_weight)
            if exc.code not in (408, 425, 429, 500, 502, 503, 504) or attempt == max_attempts - 1:
                raise RuntimeError(
                    f"Hyperliquid HTTP {exc.code} for {payload.get('type')} ({label}): {exc}"
                ) from exc
            base = 4.0 if exc.code == 429 else 0.75
            delay = min(65.0, base * (2 ** min(attempt, 4)) + random.uniform(0.15, 0.65))
            print(f"  [retry] HL HTTP {exc.code} ({label}), attempt {attempt + 1}/{max_attempts}, sleeping {delay:.1f}s", flush=True)
            time.sleep(delay)
        except (urllib.error.URLError, TimeoutError, OSError, json.JSONDecodeError) as exc:
            limiter.record(est_weight)
            if attempt == max_attempts - 1:
                raise RuntimeError(
                    f"Hyperliquid request failed after {max_attempts} attempts for {payload.get('type')} ({label}): {exc}"
                ) from exc
            delay = min(60.0, 0.75 * (2 ** min(attempt, 5)) + random.uniform(0.15, 0.65))
            print(f"  [retry] HL error ({label}): {exc}, attempt {attempt + 1}/{max_attempts}, sleeping {delay:.1f}s", flush=True)
            time.sleep(delay)
    raise RuntimeError(f"Unreachable retry exhaustion for HL {payload.get('type')} ({label})")


def check_hl_meta_universe(
    coins: list[str], limiter: SlidingWindowRateLimiter
) -> dict[str, dict]:
    meta = post_hl_info(
        {"type": "meta"},
        est_weight=20,
        weight_fn=lambda _: 20,
        limiter=limiter,
        label="meta",
    )
    if not isinstance(meta, dict) or not isinstance(meta.get("universe"), list):
        raise RuntimeError("Invalid Hyperliquid meta response")
    by_coin = {
        str(entry["name"]): entry
        for entry in meta["universe"]
        if isinstance(entry, dict) and "name" in entry
    }
    missing = [c for c in coins if c not in by_coin]
    if missing:
        raise RuntimeError(f"Coins missing from Hyperliquid meta universe: {missing}")
    return by_coin


def load_validated_hl_candles_cache(
    path: Path, symbol_id: int, coin: str
) -> list[tuple[int, str, str, str, str, str]] | None:
    if not path.exists():
        return None
    try:
        raw_bytes = path.read_bytes()
        if raw_bytes.startswith(b"\xef\xbb\xbf") or b"\r" in raw_bytes:
            return None
        payload = json.loads(raw_bytes.decode("utf-8"))
        if not isinstance(payload, dict):
            return None
        if (
            payload.get("symbol_id") != symbol_id
            or payload.get("coin") != coin
            or payload.get("interval") != HL_INTERVAL
            or payload.get("window_start_ms") != START_MS
            or payload.get("last_bar_open_ms") != LAST_BAR_OPEN_MS
            or payload.get("window_end_ms") != END_MS
        ):
            return None
        items = payload.get("candles")
        if not isinstance(items, list) or not items or payload.get("count") != len(items):
            return None
        candles: list[tuple[int, str, str, str, str, str]] = []
        prev_t: int | None = None
        for item in items:
            t_ms = int(item["t"])
            if t_ms < START_MS or t_ms > LAST_BAR_OPEN_MS:
                return None
            if (t_ms - START_MS) % INTERVAL_MS != 0:
                return None
            if prev_t is not None and t_ms <= prev_t:
                return None
            prev_t = t_ms
            o_s, h_s, l_s, c_s, v_s = (
                str(item["o"]).strip(),
                str(item["h"]).strip(),
                str(item["l"]).strip(),
                str(item["c"]).strip(),
                str(item["v"]).strip(),
            )
            if not validate_ohlcv_tuple(o_s, h_s, l_s, c_s, v_s):
                return None
            candles.append((t_ms, o_s, h_s, l_s, c_s, v_s))
        return candles
    except (OSError, UnicodeDecodeError, ValueError, KeyError, TypeError, json.JSONDecodeError):
        return None


def fetch_hl_candles(
    symbol_id: int,
    coin: str,
    bybit_sym: str,
    cache_path: Path,
    limiter: SlidingWindowRateLimiter,
) -> tuple[list[tuple[int, str, str, str, str, str]], str]:
    by_ts: dict[int, dict] = {}
    cur_start = START_MS
    while cur_start <= LAST_BAR_OPEN_MS:
        items = post_hl_info(
            {
                "type": "candleSnapshot",
                "req": {
                    "coin": coin,
                    "interval": HL_INTERVAL,
                    "startTime": cur_start,
                    "endTime": LAST_BAR_OPEN_MS,
                },
            },
            est_weight=105,
            weight_fn=lambda res: 20 + (len(res) // 60 if isinstance(res, list) else 0),
            limiter=limiter,
            label=f"{coin}:candleSnapshot",
        )
        if not isinstance(items, list):
            raise RuntimeError(f"Unexpected HL candleSnapshot payload type for {coin}: {type(items)}")
        if not items:
            break
        max_t: int | None = None
        for item in items:
            t_ms = int(item["t"])
            if max_t is None or t_ms > max_t:
                max_t = t_ms
            if START_MS <= t_ms <= LAST_BAR_OPEN_MS and (t_ms - START_MS) % INTERVAL_MS == 0:
                o_s, h_s, l_s, c_s, v_s = (
                    str(item["o"]).strip(),
                    str(item["h"]).strip(),
                    str(item["l"]).strip(),
                    str(item["c"]).strip(),
                    str(item["v"]).strip(),
                )
                if not validate_ohlcv_tuple(o_s, h_s, l_s, c_s, v_s):
                    raise RuntimeError(f"Invalid HL candle OHLCV for {coin} at {t_ms}")
                by_ts[t_ms] = item
        if len(items) < HL_CANDLE_PAGE_LIMIT or max_t is None or max_t >= LAST_BAR_OPEN_MS or max_t + INTERVAL_MS <= cur_start:
            break
        cur_start = max_t + INTERVAL_MS
        time.sleep(0.15)

    if not by_ts:
        raise RuntimeError(f"Hyperliquid returned zero 4h candles in window for {coin}")

    ordered_items = [by_ts[k] for k in sorted(by_ts.keys())]
    payload = {
        "symbol_id": symbol_id,
        "coin": coin,
        "bybit_symbol": bybit_sym,
        "interval": HL_INTERVAL,
        "interval_ms": INTERVAL_MS,
        "window_start_ms": START_MS,
        "last_bar_open_ms": LAST_BAR_OPEN_MS,
        "window_end_ms": END_MS,
        "count": len(ordered_items),
        "candles": ordered_items,
    }
    write_json_atomic(cache_path, payload)
    candles = [
        (
            int(it["t"]),
            str(it["o"]).strip(),
            str(it["h"]).strip(),
            str(it["l"]).strip(),
            str(it["c"]).strip(),
            str(it["v"]).strip(),
        )
        for it in ordered_items
    ]
    return candles, "hyperliquid_api"


def load_validated_hl_funding_cache(
    path: Path, symbol_id: int, coin: str, first_candle_ts: int
) -> tuple[dict[int, Decimal], int, int, dict] | None:
    if not path.exists():
        return None
    try:
        raw_bytes = path.read_bytes()
        if raw_bytes.startswith(b"\xef\xbb\xbf") or b"\r" in raw_bytes:
            return None
        payload = json.loads(raw_bytes.decode("utf-8"))
        if not isinstance(payload, dict):
            return None
        if (
            payload.get("symbol_id") != symbol_id
            or payload.get("coin") != coin
            or payload.get("window_start_ms") != START_MS
            or payload.get("last_bar_open_ms") != LAST_BAR_OPEN_MS
            or payload.get("window_end_ms") != END_MS
            or payload.get("first_candle_ts_ms") != first_candle_ts
        ):
            return None
        events = payload.get("events")
        if not isinstance(events, list) or payload.get("count") != len(events):
            return None
        fund_by_snapped: dict[int, Decimal] = {}
        prev_snapped: int | None = None
        max_jitter_ms = 0
        for ev in events:
            raw_time = int(ev["raw_time_ms"])
            snapped_ms = int(ev["snapped_ms"])
            expected_snap = ((raw_time + 1_800_000) // HOUR_MS) * HOUR_MS
            if snapped_ms != expected_snap:
                return None
            jitter = abs(raw_time - snapped_ms)
            if jitter > MAX_JITTER_MS or jitter != int(ev["jitter_ms"]):
                return None
            max_jitter_ms = max(max_jitter_ms, jitter)
            if snapped_ms < HL_FUNDING_KEEP_MIN_MS or snapped_ms > LAST_BAR_OPEN_MS:
                return None
            if snapped_ms <= first_candle_ts - INTERVAL_MS:
                return None
            if prev_snapped is not None and snapped_ms <= prev_snapped:
                return None
            prev_snapped = snapped_ms
            rate_dec = Decimal(str(ev["funding_rate_decimal"]))
            rate_bps = Decimal(str(ev["funding_rate_bps"]))
            if rate_dec * Decimal("10000") != rate_bps:
                return None
            fund_by_snapped[snapped_ms] = rate_dec
        if payload.get("max_jitter_ms") != max_jitter_ms:
            return None
        filtered_count = int(payload.get("filtered_out_of_window_count", 0))
        return fund_by_snapped, max_jitter_ms, filtered_count, payload
    except (OSError, UnicodeDecodeError, ValueError, KeyError, TypeError, InvalidOperation, json.JSONDecodeError):
        return None


def fetch_hl_funding(
    symbol_id: int,
    coin: str,
    bybit_sym: str,
    first_candle_ts: int,
    cache_path: Path,
    limiter: SlidingWindowRateLimiter,
) -> tuple[dict[int, Decimal], int, int, str]:
    partial_path = cache_path.with_name(f"{coin}_funding.partial.json")
    raw_items: list[dict] = []
    cur_start = HL_FUNDING_QUERY_START_MS

    if partial_path.exists():
        try:
            pdata = json.loads(partial_path.read_text(encoding="utf-8"))
            if (
                isinstance(pdata, dict)
                and pdata.get("coin") == coin
                and pdata.get("query_start_ms") == HL_FUNDING_QUERY_START_MS
                and isinstance(pdata.get("raw_items"), list)
                and pdata["raw_items"]
            ):
                raw_items = pdata["raw_items"]
                cur_start = max(int(it["time"]) for it in raw_items) + 1
                print(
                    f"  [resume] {coin} funding partial cache ({len(raw_items)} raw events, next_start={ms_to_iso(cur_start)})",
                    flush=True,
                )
        except (OSError, ValueError, KeyError, TypeError, json.JSONDecodeError):
            raw_items = []
            cur_start = HL_FUNDING_QUERY_START_MS

    page = 0
    while cur_start <= HL_FUNDING_QUERY_END_MS:
        items = post_hl_info(
            {
                "type": "fundingHistory",
                "coin": coin,
                "startTime": cur_start,
                "endTime": HL_FUNDING_QUERY_END_MS,
            },
            est_weight=45,
            weight_fn=lambda res: 20 + (len(res) // 20 if isinstance(res, list) else 0),
            limiter=limiter,
            label=f"{coin}:fundingHistory",
        )
        if not isinstance(items, list):
            raise RuntimeError(f"Unexpected HL fundingHistory payload type for {coin}: {type(items)}")
        if not items:
            break
        max_raw_time: int | None = None
        for it in items:
            t_raw = int(it["time"])
            if max_raw_time is None or t_raw > max_raw_time:
                max_raw_time = t_raw
            raw_items.append(it)
        page += 1
        if max_raw_time is None or max_raw_time < cur_start:
            raise RuntimeError(f"Non-monotonic HL funding pagination for {coin}: cur_start={cur_start}, max_raw_time={max_raw_time}")
        if page % 10 == 0:
            write_json_atomic(
                partial_path,
                {
                    "coin": coin,
                    "query_start_ms": HL_FUNDING_QUERY_START_MS,
                    "raw_items": raw_items,
                },
            )
        if len(items) < HL_FUNDING_PAGE_LIMIT or max_raw_time >= HL_FUNDING_QUERY_END_MS:
            break
        cur_start = max_raw_time + 1
        time.sleep(0.15)

    # Deduplicate exact raw_time_ms only if identical (e.g. if partial resume overlapped), otherwise enforce strict order
    raw_items.sort(key=lambda x: int(x["time"]))
    dedup_raw: list[dict] = []
    seen_raw_time: set[int] = set()
    for it in raw_items:
        rt = int(it["time"])
        if rt in seen_raw_time:
            continue
        seen_raw_time.add(rt)
        dedup_raw.append(it)

    seen_snapped: set[int] = set()
    retained_events: list[dict] = []
    filtered_events: list[dict] = []
    jitter_gt_60s_events: list[dict] = []
    fund_by_snapped: dict[int, Decimal] = {}
    max_jitter_ms = 0

    for it in dedup_raw:
        raw_time = int(it["time"])
        snapped_ms = ((raw_time + 1_800_000) // HOUR_MS) * HOUR_MS
        jitter_ms = abs(raw_time - snapped_ms)
        if jitter_ms > MAX_JITTER_MS:
            raise RuntimeError(
                f"HL funding jitter {jitter_ms}ms > {MAX_JITTER_MS}ms for {coin} at raw_time={raw_time}"
            )
        if snapped_ms in seen_snapped:
            raise RuntimeError(
                f"Duplicate snapped UTC hour {snapped_ms} ({ms_to_iso(snapped_ms)}) for {coin}"
            )
        seen_snapped.add(snapped_ms)

        rate_dec = Decimal(str(it["fundingRate"]).strip())
        rate_bps = rate_dec * Decimal("10000")
        ev_obj = {
            "raw_time_ms": raw_time,
            "snapped_ms": snapped_ms,
            "jitter_ms": jitter_ms,
            "utc": ms_to_iso(snapped_ms),
            "funding_rate_decimal": format_decimal(rate_dec),
            "funding_rate_bps": format_decimal(rate_bps),
            "premium": str(it.get("premium", "")).strip(),
        }
        if (
            HL_FUNDING_KEEP_MIN_MS <= snapped_ms <= LAST_BAR_OPEN_MS
            and snapped_ms > first_candle_ts - INTERVAL_MS
        ):
            retained_events.append(ev_obj)
            fund_by_snapped[snapped_ms] = rate_dec
            max_jitter_ms = max(max_jitter_ms, jitter_ms)
            if jitter_ms > WARN_JITTER_MS:
                jitter_gt_60s_events.append(
                    {
                        "raw_time_ms": raw_time,
                        "snapped_ms": snapped_ms,
                        "utc": ms_to_iso(snapped_ms),
                        "jitter_ms": jitter_ms,
                    }
                )
        else:
            filtered_events.append(ev_obj)

    payload = {
        "symbol_id": symbol_id,
        "coin": coin,
        "bybit_symbol": bybit_sym,
        "window_start_ms": START_MS,
        "last_bar_open_ms": LAST_BAR_OPEN_MS,
        "window_end_ms": END_MS,
        "first_candle_ts_ms": first_candle_ts,
        "effective_funding_start_ms": max(HL_FUNDING_KEEP_MIN_MS, first_candle_ts - INTERVAL_MS + HOUR_MS),
        "max_jitter_ms": max_jitter_ms,
        "jitter_gt_60s_count": len(jitter_gt_60s_events),
        "jitter_gt_60s_events": jitter_gt_60s_events,
        "filtered_out_of_window_count": len(filtered_events),
        "filtered_out_of_window_sample": filtered_events[:20],
        "count": len(retained_events),
        "events": retained_events,
    }
    write_json_atomic(cache_path, payload)
    if partial_path.exists():
        partial_path.unlink()
    return fund_by_snapped, max_jitter_ms, len(filtered_events), "hyperliquid_api"


def build_csv_rows_and_reconcile(
    symbol_id: int,
    candles: list[tuple[int, str, str, str, str, str]],
    fund_by_ts: dict[int, Decimal],
) -> tuple[list[list[str | int]], list[list[str | int]], list[list[str | int]], dict]:
    bucket_bps_by_ts, subbar_aggregated_events, mapped_events = (
        aggregate_funding_causal_buckets(fund_by_ts)
    )
    raw_funding_sum_bps = sum((f_bps for _, _, f_bps in mapped_events), Decimal("0"))

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

    gap_count = 0
    missing_bars_total = 0
    for idx in range(1, len(candles)):
        diff = candles[idx][0] - candles[idx - 1][0]
        if diff > INTERVAL_MS:
            gap_count += 1
            missing_bars_total += (diff // INTERVAL_MS) - 1

    leading_missing_bars = (
        (candles[0][0] - START_MS) // INTERVAL_MS if candles else EXPECTED_CANDLES
    )
    trailing_missing_bars = (
        (LAST_BAR_OPEN_MS - candles[-1][0]) // INTERVAL_MS if candles else 0
    )

    unmatched_events = []
    for f_ts, target_open_ts, f_bps in mapped_events:
        if target_open_ts not in candle_ts_set:
            unmatched_events.append(
                {
                    "ts_ms": f_ts,
                    "utc": ms_to_iso(f_ts),
                    "target_open_ts": target_open_ts,
                    "target_open_utc": ms_to_iso(target_open_ts),
                    "funding_rate_bps": format_decimal(f_bps),
                }
            )

    fund_ts_list = sorted(fund_by_ts.keys())
    observed_fund_intervals_h = sorted(
        {
            round((fund_ts_list[i] - fund_ts_list[i - 1]) / 3_600_000.0, 6)
            for i in range(1, len(fund_ts_list))
        }
    )
    diff_bps = raw_funding_sum_bps - csv_funding_sum_bps
    stats = {
        "candle_count": len(candles),
        "expected_candles": EXPECTED_CANDLES,
        "y1_count": len(y1_rows),
        "y2_count": len(y2_rows),
        "min_ts_ms": candles[0][0] if candles else None,
        "min_utc": ms_to_iso(candles[0][0]) if candles else None,
        "max_ts_ms": candles[-1][0] if candles else None,
        "max_utc": ms_to_iso(candles[-1][0]) if candles else None,
        "leading_missing_bars": leading_missing_bars,
        "trailing_missing_bars": trailing_missing_bars,
        "gap_count": gap_count,
        "missing_bars_total": missing_bars_total,
        "observed_funding_intervals_h": observed_fund_intervals_h,
        "raw_funding_count": len(fund_by_ts),
        "subbar_aggregated_events": subbar_aggregated_events,
        "aligned_funding_count": len(mapped_events) - len(unmatched_events),
        "aligned_funding_buckets": aligned_funding_buckets,
        "unmatched_events_count": len(unmatched_events),
        "unmatched_events_sample": unmatched_events[:10],
        "raw_funding_sum_bps": format_decimal(raw_funding_sum_bps),
        "csv_funding_sum_bps": format_decimal(csv_funding_sum_bps),
        "reconciliation_diff_bps": format_decimal(diff_bps),
    }
    return full_rows, y1_rows, y2_rows, stats


def process_bybit_symbol(
    symbol_id: int, bybit_sym: str, data_root: Path, force: bool
) -> tuple[dict, set[int]]:
    src_xs30_dir = data_root / "bybit_xs30_4h"
    src_raw_fund_path = src_xs30_dir / "raw_funding" / f"{bybit_sym}.json"
    src_csv_path = src_xs30_dir / f"{bybit_sym}.csv"

    dst_24m = data_root / "bybit_hl_xvenue_24m"
    dst_y1 = data_root / "bybit_hl_xvenue_y1"
    dst_y2 = data_root / "bybit_hl_xvenue_y2"

    dst_raw_fund_path = dst_24m / "raw_bybit_funding" / f"{bybit_sym}.json"
    dst_csv_24m = dst_24m / "bybit" / f"{bybit_sym}.csv"
    dst_csv_y1 = dst_y1 / "bybit" / f"{bybit_sym}.csv"
    dst_csv_y2 = dst_y2 / "bybit" / f"{bybit_sym}.csv"

    funding_source = "bybit_v5_api"
    fund_by_ts: dict[int, Decimal] | None = None
    if not force:
        fund_by_ts = load_validated_raw_bybit_funding(
            dst_raw_fund_path, bybit_sym, required_symbol_id=symbol_id
        )
        if fund_by_ts is not None:
            funding_source = "local_xvenue_remapped"
        else:
            fund_by_ts = load_validated_raw_bybit_funding(
                src_raw_fund_path, bybit_sym, required_symbol_id=None
            )
            if fund_by_ts is not None:
                funding_source = "reused_bybit_xs30_4h"

    if fund_by_ts is None:
        fund_by_ts = fetch_bybit_funding(bybit_sym)
        funding_source = "bybit_v5_api"

    if force or funding_source != "local_xvenue_remapped" or not dst_raw_fund_path.exists():
        write_json_atomic(
            dst_raw_fund_path,
            build_raw_bybit_funding_payload(symbol_id, bybit_sym, fund_by_ts),
        )

    bucket_bps_by_ts, _, _ = aggregate_funding_causal_buckets(fund_by_ts)

    kline_source = "bybit_v5_api"
    candles: list[tuple[int, str, str, str, str, str]] | None = None
    if not force:
        if dst_csv_24m.exists() and dst_csv_y1.exists() and dst_csv_y2.exists():
            candles = load_validated_bybit_csv(
                dst_csv_24m,
                required_symbol_id=symbol_id,
                expected_bucket_bps_by_ts=bucket_bps_by_ts,
                require_full_4380=True,
            )
            if candles is not None:
                kline_source = "local_xvenue_remapped"
        if candles is None:
            candles = load_validated_bybit_csv(
                src_csv_path,
                required_symbol_id=None,
                expected_bucket_bps_by_ts=bucket_bps_by_ts,
                require_full_4380=True,
            )
            if candles is not None:
                kline_source = "reused_bybit_xs30_4h"

    if candles is None:
        candles = fetch_bybit_klines(bybit_sym)
        kline_source = "bybit_v5_api"

    full_rows, y1_rows, y2_rows, stats = build_csv_rows_and_reconcile(
        symbol_id, candles, fund_by_ts
    )
    if (
        force
        or kline_source != "local_xvenue_remapped"
        or not (dst_csv_24m.exists() and dst_csv_y1.exists() and dst_csv_y2.exists())
    ):
        write_csv_atomic(dst_csv_24m, full_rows)
        write_csv_atomic(dst_csv_y1, y1_rows)
        write_csv_atomic(dst_csv_y2, y2_rows)

    stats["funding_source"] = funding_source
    stats["kline_source"] = kline_source
    return stats, {row[0] for row in candles}


def process_hl_symbol(
    symbol_id: int,
    bybit_sym: str,
    coin: str,
    data_root: Path,
    force: bool,
    limiter: SlidingWindowRateLimiter,
) -> tuple[dict, set[int]]:
    dst_24m = data_root / "bybit_hl_xvenue_24m"
    dst_y1 = data_root / "bybit_hl_xvenue_y1"
    dst_y2 = data_root / "bybit_hl_xvenue_y2"

    raw_hl_dir = dst_24m / "raw_hl"
    candles_cache_path = raw_hl_dir / f"{coin}_candles.json"
    funding_cache_path = raw_hl_dir / f"{coin}_funding.json"

    dst_csv_24m = dst_24m / "hyperliquid" / f"{coin}.csv"
    dst_csv_y1 = dst_y1 / "hyperliquid" / f"{coin}.csv"
    dst_csv_y2 = dst_y2 / "hyperliquid" / f"{coin}.csv"

    candles: list[tuple[int, str, str, str, str, str]] | None = None
    kline_source = "hyperliquid_api"
    if not force:
        candles = load_validated_hl_candles_cache(candles_cache_path, symbol_id, coin)
        if candles is not None:
            kline_source = "local_cache"

    if candles is None:
        candles, kline_source = fetch_hl_candles(
            symbol_id, coin, bybit_sym, candles_cache_path, limiter
        )

    first_candle_ts = candles[0][0]

    fund_tuple = None
    funding_source = "hyperliquid_api"
    if not force:
        fund_tuple = load_validated_hl_funding_cache(
            funding_cache_path, symbol_id, coin, first_candle_ts
        )
        if fund_tuple is not None:
            funding_source = "local_cache"

    if fund_tuple is None:
        fund_by_snapped, max_jitter_ms, filtered_count, funding_source = fetch_hl_funding(
            symbol_id, coin, bybit_sym, first_candle_ts, funding_cache_path, limiter
        )
    else:
        fund_by_snapped, max_jitter_ms, filtered_count, _ = fund_tuple

    full_rows, y1_rows, y2_rows, stats = build_csv_rows_and_reconcile(
        symbol_id, candles, fund_by_snapped
    )
    write_csv_atomic(dst_csv_24m, full_rows)
    write_csv_atomic(dst_csv_y1, y1_rows)
    write_csv_atomic(dst_csv_y2, y2_rows)

    stats["funding_source"] = funding_source
    stats["kline_source"] = kline_source
    stats["max_jitter_ms"] = max_jitter_ms
    stats["filtered_out_of_window_count"] = filtered_count
    return stats, {row[0] for row in candles}


def rebuild_manifest(data_root: Path) -> Path:
    dst_24m = data_root / "bybit_hl_xvenue_24m"
    raw_hl_dir = dst_24m / "raw_hl"
    completed_summaries: list[dict] = []
    missing_coins: list[str] = []

    for sym_id, bybit_sym, coin in UNIVERSE:
        summary_path = raw_hl_dir / f"{coin}_summary.json"
        if not summary_path.exists():
            missing_coins.append(coin)
            continue
        try:
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            missing_coins.append(coin)
            continue
        if (
            summary.get("symbol_id") != sym_id
            or summary.get("bybit_symbol") != bybit_sym
            or summary.get("hl_coin") != coin
        ):
            missing_coins.append(coin)
            continue
        completed_summaries.append(summary)

    manifest = {
        "generated_utc": datetime.now(tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "timeframe": TF_NAME,
        "interval_ms": INTERVAL_MS,
        "expected_candles": EXPECTED_CANDLES,
        "universe_count": len(UNIVERSE),
        "completed_symbols_count": len(completed_summaries),
        "pending_coins": missing_coins,
        "funding_alignment": "BarOpenSettlement_CausalHalfOpen_(bar_open_ts-4h,bar_open_ts]",
        "window_start_ms": START_MS,
        "window_start_utc": ms_to_iso(START_MS),
        "split_ms": SPLIT_MS,
        "split_utc": ms_to_iso(SPLIT_MS),
        "last_bar_open_ms": LAST_BAR_OPEN_MS,
        "last_bar_open_utc": ms_to_iso(LAST_BAR_OPEN_MS),
        "window_end_ms": END_MS,
        "window_end_utc": ms_to_iso(END_MS),
        "universe": [
            {"symbol_id": sid, "bybit_symbol": bsym, "hl_coin": hcoin}
            for sid, bsym, hcoin in UNIVERSE
        ],
        "symbols": completed_summaries,
    }
    manifest_path = dst_24m / "download_manifest.json"
    write_json_atomic(manifest_path, manifest)
    return manifest_path


def parse_symbol_filter(raw: str | None) -> list[tuple[int, str, str]]:
    if raw is None or not raw.strip():
        return list(UNIVERSE)
    by_key: dict[str, tuple[int, str, str]] = {}
    for entry in UNIVERSE:
        by_key[entry[1].upper()] = entry
        by_key[entry[2].upper()] = entry

    selected_ids: set[int] = set()
    for token in raw.split(","):
        tok = token.strip().upper()
        if not tok:
            continue
        if tok not in by_key:
            valid = ", ".join(c for _, _, c in UNIVERSE)
            raise ValueError(f"Unknown symbol '{token}' in --symbols. Valid HL coins: {valid}")
        selected_ids.add(by_key[tok][0])
    if not selected_ids:
        raise ValueError("--symbols filter produced empty selection")
    return [entry for entry in UNIVERSE if entry[0] in selected_ids]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Fetch and align 24m 4h Bybit + Hyperliquid datasets for Track 7."
    )
    parser.add_argument("--data-root", default="data_local", help="Root data directory")
    parser.add_argument(
        "--symbols",
        default=None,
        help="Optional comma-separated filter by HL coin or Bybit symbol (e.g. BTC,ETH,SOL,AVAX)",
    )
    parser.add_argument("--force", action="store_true", help="Re-download/rebuild selected symbols")
    args = parser.parse_args()

    assert int(datetime(2024, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == START_MS
    assert int(datetime(2025, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == SPLIT_MS
    assert int(datetime(2026, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == END_MS
    assert LAST_BAR_OPEN_MS == END_MS - INTERVAL_MS
    assert EXPECTED_CANDLES == 4380

    selected = parse_symbol_filter(args.symbols)
    data_root = Path(args.data_root)
    limiter = SlidingWindowRateLimiter()
    t0 = time.monotonic()

    print(
        f"Checking Hyperliquid meta universe for {len(selected)} coin(s): "
        f"{[c for _, _, c in selected]}",
        flush=True,
    )
    hl_meta = check_hl_meta_universe([c for _, _, c in selected], limiter)

    failures: list[str] = []
    for sym_id, bybit_sym, coin in selected:
        t_sym = time.monotonic()
        print(f"[{sym_id:02d}] Processing {bybit_sym} <-> HL {coin}...", flush=True)

        bybit_stats, bybit_ts = process_bybit_symbol(sym_id, bybit_sym, data_root, args.force)
        hl_stats, hl_ts = process_hl_symbol(
            sym_id, bybit_sym, coin, data_root, args.force, limiter
        )

        matched_bars = len(bybit_ts & hl_ts)
        summary = {
            "symbol_id": sym_id,
            "bybit_symbol": bybit_sym,
            "hl_coin": coin,
            "hl_sz_decimals": hl_meta[coin].get("szDecimals"),
            "hl_max_leverage": hl_meta[coin].get("maxLeverage"),
            "matched_4h_bars": matched_bars,
            "bybit_only_bars": len(bybit_ts - hl_ts),
            "hl_only_bars": len(hl_ts - bybit_ts),
            "bybit": bybit_stats,
            "hyperliquid": hl_stats,
            "updated_utc": datetime.now(tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        }
        summary_path = data_root / "bybit_hl_xvenue_24m" / "raw_hl" / f"{coin}_summary.json"
        write_json_atomic(summary_path, summary)

        if (
            bybit_stats["unmatched_events_count"] != 0
            or Decimal(bybit_stats["reconciliation_diff_bps"]) != 0
        ):
            failures.append(
                f"{bybit_sym} Bybit funding reconciliation error: "
                f"unmatched={bybit_stats['unmatched_events_count']}, diff={bybit_stats['reconciliation_diff_bps']}"
            )
        if (
            hl_stats["unmatched_events_count"] != 0
            or Decimal(hl_stats["reconciliation_diff_bps"]) != 0
        ):
            failures.append(
                f"{coin} HL funding reconciliation error: "
                f"unmatched={hl_stats['unmatched_events_count']}, diff={hl_stats['reconciliation_diff_bps']}"
            )

        elapsed_sym = round(time.monotonic() - t_sym, 1)
        print(
            f"[{sym_id:02d}] {coin:6s}/{bybit_sym:10s} | "
            f"matched={matched_bars}/{EXPECTED_CANDLES} | "
            f"Bybit(4h={bybit_stats['candle_count']}, fund={bybit_stats['raw_funding_count']}, diff={bybit_stats['reconciliation_diff_bps']}, src={bybit_stats['kline_source']}) | "
            f"HL(4h={hl_stats['candle_count']}, fund={hl_stats['raw_funding_count']}, jitter_max={hl_stats['max_jitter_ms']}ms, diff={hl_stats['reconciliation_diff_bps']}, src={hl_stats['kline_source']}) | "
            f"{elapsed_sym}s",
            flush=True,
        )

    manifest_path = rebuild_manifest(data_root)
    total_elapsed = round(time.monotonic() - t0, 2)
    print(
        f"Manifest updated at {manifest_path} in {total_elapsed}s "
        f"(HL API weight recorded this run: {limiter.total_weight_used}).",
        flush=True,
    )
    if failures:
        for msg in failures:
            print(f"ERROR: {msg}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())

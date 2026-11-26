#!/usr/bin/env python3
"""Fetch 24 months of Bybit V5 public linear perpetual klines and funding history.

Strict Law 1 compliance: only downloads, validates, aligns BarOpenSettlement funding,
and writes normalized CSVs for the Rust `vortex-alfa` binary. Performs zero signal,
fee, Newey-West, or DSR computations.
"""

from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import csv
from datetime import datetime, timezone
from decimal import Decimal
import json
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
]

TIMEFRAMES: list[tuple[str, str, int]] = [
    ("15m", "15", 900_000),
    ("1h", "60", 3_600_000),
    ("4h", "240", 14_400_000),
]

START_MS = 1732492800000  # 2024-11-25T00:00:00Z
SPLIT_MS = 1764028800000  # 2025-11-25T00:00:00Z
END_MS = 1795564800000    # 2026-11-25T00:00:00Z

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
    headers = {"User-Agent": "vortex-alfa-phase1a/1.0"}
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


def fetch_klines(symbol: str, interval: str, interval_ms: int) -> list[tuple[int, str, str, str, str, str]]:
    by_ts: dict[int, tuple[int, str, str, str, str, str]] = {}
    cur_end = END_MS - interval_ms
    while cur_end >= START_MS:
        payload = fetch_json(
            "/v5/market/kline",
            {
                "category": "linear",
                "symbol": symbol,
                "interval": interval,
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
            if START_MS <= open_ts and open_ts + interval_ms <= END_MS:
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
        time.sleep(0.025)
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
        time.sleep(0.025)
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


def process_symbol(
    symbol_id: int,
    symbol: str,
    data_root: Path,
    force: bool,
) -> dict:
    raw_fund_path = data_root / "bybit_24m" / "raw_funding" / f"{symbol}.json"
    if raw_fund_path.exists() and not force:
        raw_fund_data = json.loads(raw_fund_path.read_text(encoding="utf-8"))
        fund_by_ts = {
            int(item["ts_ms"]): Decimal(item["funding_rate_decimal"])
            for item in raw_fund_data["events"]
        }
    else:
        fund_by_ts = fetch_funding(symbol)
        fund_events = [
            {
                "ts_ms": ts_ms,
                "utc": ms_to_iso(ts_ms),
                "funding_rate_decimal": format_decimal(rate),
                "funding_rate_bps": format_decimal(rate * Decimal("10000")),
            }
            for ts_ms, rate in fund_by_ts.items()
        ]
        write_json_atomic(
            raw_fund_path,
            {
                "symbol_id": symbol_id,
                "symbol": symbol,
                "window_start_ms": START_MS,
                "window_end_ms": END_MS,
                "count": len(fund_events),
                "events": fund_events,
            },
        )

    fund_ts_list = sorted(fund_by_ts.keys())
    observed_fund_intervals_h = sorted(
        {
            round((fund_ts_list[i] - fund_ts_list[i - 1]) / 3_600_000.0, 6)
            for i in range(1, len(fund_ts_list))
        }
    )
    fund_bps_by_ts = {
        ts_ms: rate * Decimal("10000") for ts_ms, rate in fund_by_ts.items()
    }
    raw_funding_sum_bps = sum(fund_bps_by_ts.values(), Decimal("0"))

    tf_summaries: dict[str, dict] = {}
    for tf_name, interval_str, interval_ms in TIMEFRAMES:
        p_full = data_root / "bybit_24m" / tf_name / f"{symbol}.csv"
        p_y1 = data_root / "bybit_24m_y1" / tf_name / f"{symbol}.csv"
        p_y2 = data_root / "bybit_24m_y2" / tf_name / f"{symbol}.csv"
        p_exbtc = data_root / "bybit_24m_exbtc" / tf_name / f"{symbol}.csv"

        expected_outputs_exist = (
            p_full.exists()
            and p_y1.exists()
            and p_y2.exists()
            and (symbol_id == 0 or p_exbtc.exists())
        )

        if expected_outputs_exist and not force:
            candles = []
            with p_full.open("r", encoding="utf-8", newline="") as fh:
                reader = csv.DictReader(fh)
                for r in reader:
                    candles.append(
                        (
                            int(r["ts_ms"]),
                            r["open"],
                            r["high"],
                            r["low"],
                            r["close"],
                            r["volume"],
                        )
                    )
        else:
            candles = fetch_klines(symbol, interval_str, interval_ms)

        full_rows: list[list[str | int]] = []
        y1_rows: list[list[str | int]] = []
        y2_rows: list[list[str | int]] = []
        candle_ts_set: set[int] = set()
        csv_funding_sum_bps = Decimal("0")
        aligned_funding_count = 0

        for open_ts, o, h, l, c, vol in candles:
            candle_ts_set.add(open_ts)
            f_bps = fund_bps_by_ts.get(open_ts, Decimal("0"))
            if open_ts in fund_bps_by_ts:
                aligned_funding_count += 1
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

        if not expected_outputs_exist or force:
            write_csv_atomic(p_full, full_rows)
            write_csv_atomic(p_y1, y1_rows)
            write_csv_atomic(p_y2, y2_rows)
            if symbol_id != 0:
                write_csv_atomic(p_exbtc, full_rows)

        gap_count = 0
        missing_bars_total = 0
        for idx in range(1, len(candles)):
            diff = candles[idx][0] - candles[idx - 1][0]
            if diff > interval_ms:
                gap_count += 1
                missing_bars_total += (diff // interval_ms) - 1

        unmatched_events = []
        unmatched_interior_count = 0
        unmatched_gap_count = 0
        for f_ts, f_bps in fund_bps_by_ts.items():
            if f_ts in candle_ts_set:
                continue
            rem = (f_ts - START_MS) % interval_ms
            if rem != 0:
                enclosing_open = f_ts - rem
                kind = (
                    "interior_to_existing_bar"
                    if enclosing_open in candle_ts_set
                    else "interior_and_missing_bar"
                )
                unmatched_interior_count += 1
            else:
                kind = "missing_candle_gap"
                unmatched_gap_count += 1
            unmatched_events.append(
                {
                    "ts_ms": f_ts,
                    "utc": ms_to_iso(f_ts),
                    "funding_rate_bps": format_decimal(f_bps),
                    "classification": kind,
                }
            )

        min_ts = candles[0][0] if candles else None
        max_ts = candles[-1][0] if candles else None
        tf_summaries[tf_name] = {
            "candle_count": len(candles),
            "expected_candles": (END_MS - START_MS) // interval_ms,
            "y1_count": len(y1_rows),
            "y2_count": len(y2_rows),
            "min_ts_ms": min_ts,
            "min_utc": ms_to_iso(min_ts) if min_ts is not None else None,
            "max_ts_ms": max_ts,
            "max_utc": ms_to_iso(max_ts) if max_ts is not None else None,
            "gap_count": gap_count,
            "missing_bars_total": missing_bars_total,
            "raw_funding_count": len(fund_bps_by_ts),
            "aligned_funding_count": aligned_funding_count,
            "unmatched_interior_count": unmatched_interior_count,
            "unmatched_gap_count": unmatched_gap_count,
            "unmatched_events_sample": unmatched_events[:10],
            "raw_funding_sum_bps": format_decimal(raw_funding_sum_bps),
            "csv_funding_sum_bps": format_decimal(csv_funding_sum_bps),
            "reconciliation_diff_bps": format_decimal(raw_funding_sum_bps - csv_funding_sum_bps),
        }

    return {
        "symbol_id": symbol_id,
        "symbol": symbol,
        "observed_funding_intervals_h": observed_fund_intervals_h,
        "raw_funding_count": len(fund_by_ts),
        "raw_funding_sum_bps": format_decimal(raw_funding_sum_bps),
        "timeframes": tf_summaries,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Fetch 24m Bybit public klines & funding.")
    parser.add_argument("--data-root", default="data_local", help="Root output directory")
    parser.add_argument("--workers", type=int, default=4, help="Concurrent symbol workers")
    parser.add_argument("--force", action="store_true", help="Re-download existing CSV/JSON files")
    args = parser.parse_args()

    assert int(datetime(2024, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == START_MS
    assert int(datetime(2025, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == SPLIT_MS
    assert int(datetime(2026, 11, 25, 0, 0, 0, tzinfo=timezone.utc).timestamp() * 1000) == END_MS

    t0 = time.monotonic()
    data_root = Path(args.data_root)
    results: dict[int, dict] = {}

    with ThreadPoolExecutor(max_workers=max(1, min(args.workers, 6))) as pool:
        future_map = {
            pool.submit(process_symbol, sym_id, sym, data_root, args.force): (sym_id, sym)
            for sym_id, sym in SYMBOLS
        }
        for fut in as_completed(future_map):
            sym_id, sym = future_map[fut]
            res = fut.result()
            results[sym_id] = res
            tfs = res["timeframes"]
            print(
                f"[{sym_id:02d}] {sym:10s} | "
                f"15m={tfs['15m']['candle_count']} 1h={tfs['1h']['candle_count']} 4h={tfs['4h']['candle_count']} | "
                f"fund_raw={res['raw_funding_count']} ints_h={res['observed_funding_intervals_h']} | "
                f"4h_diff_bps={tfs['4h']['reconciliation_diff_bps']}",
                flush=True,
            )

    ordered = [results[sym_id] for sym_id, _ in SYMBOLS]
    manifest = {
        "generated_utc": datetime.now(tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "window_start_ms": START_MS,
        "window_start_utc": ms_to_iso(START_MS),
        "split_ms": SPLIT_MS,
        "split_utc": ms_to_iso(SPLIT_MS),
        "window_end_ms": END_MS,
        "window_end_utc": ms_to_iso(END_MS),
        "elapsed_sec": round(time.monotonic() - t0, 2),
        "symbols": ordered,
    }
    manifest_path = data_root / "bybit_24m" / "download_manifest.json"
    write_json_atomic(manifest_path, manifest)
    print(f"Manifest written to {manifest_path} in {manifest['elapsed_sec']}s")
    return 0


if __name__ == "__main__":
    sys.exit(main())

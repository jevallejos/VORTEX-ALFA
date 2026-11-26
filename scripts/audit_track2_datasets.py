#!/usr/bin/env python3
"""Audit downloaded Track 2 CSV datasets and funding reconciliation."""

from __future__ import annotations

import argparse
import csv
from decimal import Decimal
import json
import math
from pathlib import Path
import sys

SYMBOLS: dict[str, int] = {
    "BTCUSDT": 0,
    "ETHUSDT": 1,
    "SOLUSDT": 2,
    "AVAXUSDT": 3,
    "TAOUSDT": 4,
    "SUIUSDT": 5,
    "ORDIUSDT": 6,
    "PENDLEUSDT": 7,
    "WIFUSDT": 8,
    "LINKUSDT": 9,
    "NEARUSDT": 10,
    "LTCUSDT": 11,
}

TIMEFRAMES: dict[str, int] = {
    "15m": 900_000,
    "1h": 3_600_000,
    "4h": 14_400_000,
}

START_MS = 1732492800000
SPLIT_MS = 1764028800000
END_MS = 1795564800000
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


def audit_csv(
    path: Path,
    expected_sym_id: int,
    interval_ms: int,
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
    row_count = 0
    prev_ts: int | None = None
    funding_sum = Decimal("0")
    ts_set: set[int] = set()

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
            if ts_ms < win_start or ts_ms + interval_ms > win_end:
                out_of_window_count += 1
            if prev_ts is not None:
                if ts_ms == prev_ts:
                    duplicate_ts_count += 1
                elif ts_ms < prev_ts:
                    backward_ts_count += 1
            prev_ts = ts_ms
            ts_set.add(ts_ms)
            funding_sum += f_dec

    return {
        "file": str(path),
        "rows": row_count,
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
        "funding_sum_bps": str(funding_sum.normalize() if funding_sum != 0 else Decimal("0")),
        "ts_set": ts_set,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Audit Track 2 CSV datasets.")
    parser.add_argument("--data-root", default="data_local", help="Root data directory")
    args = parser.parse_args()
    root = Path(args.data_root)

    datasets = [
        ("bybit_24m", START_MS, END_MS, list(SYMBOLS.items())),
        ("bybit_24m_y1", START_MS, SPLIT_MS, list(SYMBOLS.items())),
        ("bybit_24m_y2", SPLIT_MS, END_MS, list(SYMBOLS.items())),
        (
            "bybit_24m_exbtc",
            START_MS,
            END_MS,
            [(s, sid) for s, sid in SYMBOLS.items() if sid != 0],
        ),
    ]

    totals = {
        "files_checked": 0,
        "rows_checked": 0,
        "nan_inf_total": 0,
        "non_pos_price_total": 0,
        "invalid_ohlc_total": 0,
        "neg_vol_total": 0,
        "duplicate_ts_total": 0,
        "backward_ts_total": 0,
        "wrong_sym_id_total": 0,
        "out_of_window_total": 0,
        "header_failures": 0,
        "encoding_failures": 0,
        "split_mismatch_total": 0,
        "exbtc_mismatch_total": 0,
    }

    full_counts: dict[tuple[str, str], int] = {}
    y1_counts: dict[tuple[str, str], int] = {}
    y2_counts: dict[tuple[str, str], int] = {}
    exbtc_counts: dict[tuple[str, str], int] = {}
    reconciliation_rows = []

    raw_funding_by_sym: dict[str, dict[int, Decimal]] = {}
    for sym in SYMBOLS:
        raw_path = root / "bybit_24m" / "raw_funding" / f"{sym}.json"
        if not raw_path.exists():
            print(f"ERROR: missing raw funding file {raw_path}", file=sys.stderr)
            return 2
        payload = json.loads(raw_path.read_text(encoding="utf-8"))
        raw_funding_by_sym[sym] = {
            int(ev["ts_ms"]): Decimal(ev["funding_rate_bps"])
            for ev in payload["events"]
        }

    for ds_name, w_start, w_end, sym_items in datasets:
        for tf_name, interval_ms in TIMEFRAMES.items():
            for sym, sym_id in sym_items:
                csv_path = root / ds_name / tf_name / f"{sym}.csv"
                if not csv_path.exists():
                    print(f"ERROR: missing dataset file {csv_path}", file=sys.stderr)
                    return 2
                res = audit_csv(csv_path, sym_id, interval_ms, w_start, w_end)
                totals["files_checked"] += 1
                totals["rows_checked"] += res["rows"]
                totals["nan_inf_total"] += res["nan_inf_count"]
                totals["non_pos_price_total"] += res["non_pos_price_count"]
                totals["invalid_ohlc_total"] += res["invalid_ohlc_count"]
                totals["neg_vol_total"] += res["neg_vol_count"]
                totals["duplicate_ts_total"] += res["duplicate_ts_count"]
                totals["backward_ts_total"] += res["backward_ts_count"]
                totals["wrong_sym_id_total"] += res["wrong_sym_id_count"]
                totals["out_of_window_total"] += res["out_of_window_count"]
                if not res["header_ok"]:
                    totals["header_failures"] += 1
                if not (res["utf8_no_bom"] and res["lf_only"]):
                    totals["encoding_failures"] += 1

                if ds_name == "bybit_24m":
                    full_counts[(tf_name, sym)] = res["rows"]
                    raw_map = raw_funding_by_sym[sym]
                    raw_sum = sum(raw_map.values(), Decimal("0"))
                    csv_sum = Decimal(res["funding_sum_bps"])
                    unmatched_ts = [ts for ts in raw_map if ts not in res["ts_set"]]
                    reconciliation_rows.append(
                        {
                            "symbol": sym,
                            "tf": tf_name,
                            "rows": res["rows"],
                            "raw_funding_events": len(raw_map),
                            "unmatched_funding_events": len(unmatched_ts),
                            "raw_sum_bps": format(raw_sum, "f"),
                            "csv_sum_bps": format(csv_sum, "f"),
                            "diff_bps": format(raw_sum - csv_sum, "f"),
                        }
                    )
                elif ds_name == "bybit_24m_y1":
                    y1_counts[(tf_name, sym)] = res["rows"]
                elif ds_name == "bybit_24m_y2":
                    y2_counts[(tf_name, sym)] = res["rows"]
                elif ds_name == "bybit_24m_exbtc":
                    exbtc_counts[(tf_name, sym)] = res["rows"]

    for tf_name in TIMEFRAMES:
        for sym, sym_id in SYMBOLS.items():
            full_n = full_counts[(tf_name, sym)]
            if y1_counts[(tf_name, sym)] + y2_counts[(tf_name, sym)] != full_n:
                totals["split_mismatch_total"] += 1
            if sym_id != 0 and exbtc_counts[(tf_name, sym)] != full_n:
                totals["exbtc_mismatch_total"] += 1

    hard_errors = (
        totals["nan_inf_total"]
        + totals["non_pos_price_total"]
        + totals["invalid_ohlc_total"]
        + totals["neg_vol_total"]
        + totals["duplicate_ts_total"]
        + totals["backward_ts_total"]
        + totals["wrong_sym_id_total"]
        + totals["out_of_window_total"]
        + totals["header_failures"]
        + totals["encoding_failures"]
        + totals["split_mismatch_total"]
        + totals["exbtc_mismatch_total"]
    )

    report = {
        "status": "PASS" if hard_errors == 0 else "FAIL",
        "totals": totals,
        "funding_reconciliation": reconciliation_rows,
    }
    out_path = root / "bybit_24m" / "audit_report.json"
    with out_path.open("w", encoding="utf-8", newline="\n") as fh:
        json.dump(report, fh, indent=2)
        fh.write("\n")

    print(json.dumps({"status": report["status"], "totals": totals}, indent=2))
    nonzero_diffs = [r for r in reconciliation_rows if Decimal(r["diff_bps"]) != 0]
    print(f"Non-zero funding reconciliation entries: {len(nonzero_diffs)}")
    for item in nonzero_diffs:
        print(
            f"  {item['symbol']:10s} {item['tf']:3s}: unmatched={item['unmatched_funding_events']}, "
            f"raw_sum={item['raw_sum_bps']}, csv_sum={item['csv_sum_bps']}, diff={item['diff_bps']} bps"
        )
    return 0 if hard_errors == 0 else 1


if __name__ == "__main__":
    sys.exit(main())

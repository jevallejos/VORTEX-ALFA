use std::path::PathBuf;

use crate::application::falsifier_xvenue_carry::run_xvenue_carry_falsification;
use crate::application::xvenue_carry_types::{XVenueCarryConfig, XVenueCarryReport};
use crate::domain::error::DomainError;
use crate::domain::types::Bps;
use crate::infrastructure::csv_candle_reader::CsvCandleReader;
use crate::interfaces::cli::{fmt_opt, take_arg, write_json_pretty};

fn parse_f64_flag(raw: &str, flag: &str, allow_zero: bool) -> Result<f64, DomainError> {
    let parsed: f64 = raw
        .parse()
        .map_err(|_| DomainError::Cli(format!("invalid {flag} '{raw}'")))?;
    let valid_min = if allow_zero {
        parsed >= 0.0
    } else {
        parsed > 0.0
    };
    if !parsed.is_finite() || !valid_min {
        let cmp = if allow_zero { ">=" } else { ">" };
        return Err(DomainError::Cli(format!(
            "{flag} must be finite and {cmp} 0"
        )));
    }
    Ok(parsed)
}

pub(crate) fn run_scan_xvenue(args: &[String]) -> Result<String, DomainError> {
    let (mut bybit_dir, mut hl_dir, mut out) = (None, None, None);
    let mut config = XVenueCarryConfig::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--bybit-dir" => {
                bybit_dir = Some(PathBuf::from(take_arg(args, &mut index, "--bybit-dir")?));
            }
            "--hl-dir" => {
                hl_dir = Some(PathBuf::from(take_arg(args, &mut index, "--hl-dir")?));
            }
            "--slippage-bps" => {
                let raw = take_arg(args, &mut index, "--slippage-bps")?;
                config.slippage_bps = Bps::new(parse_f64_flag(&raw, "--slippage-bps", true)?)?;
            }
            "--capital-usdt" => {
                let raw = take_arg(args, &mut index, "--capital-usdt")?;
                config.capital_usdt = parse_f64_flag(&raw, "--capital-usdt", false)?;
            }
            "--bridge-fee-usdt" => {
                let raw = take_arg(args, &mut index, "--bridge-fee-usdt")?;
                config.bridge_fee_usdt = parse_f64_flag(&raw, "--bridge-fee-usdt", true)?;
            }
            "--out" => out = Some(PathBuf::from(take_arg(args, &mut index, "--out")?)),
            flag => {
                return Err(DomainError::Cli(format!(
                    "unknown flag '{flag}' for scan-xvenue"
                )))
            }
        }
        index += 1;
    }
    let bybit_path =
        bybit_dir.ok_or_else(|| DomainError::Cli("missing --bybit-dir".to_string()))?;
    let hl_path = hl_dir.ok_or_else(|| DomainError::Cli("missing --hl-dir".to_string()))?;
    let mut bybit_reader = CsvCandleReader::open(&bybit_path)?;
    let mut hl_reader = CsvCandleReader::open(&hl_path)?;
    let report = run_xvenue_carry_falsification(&mut bybit_reader, &mut hl_reader, &config)?;
    if let Some(path) = out.as_deref() {
        write_json_pretty(path, &report)?;
    }
    Ok(format_xvenue_report_ascii(&report))
}

#[must_use]
pub fn format_xvenue_report_ascii(xvenue: &XVenueCarryReport) -> String {
    let base = &xvenue.report;
    let mut lines = vec![
        format!(
            "VORTEX-ALFA {} | slices={} | bybit={} | hl={} | slip={:.2}bps | cap=${:.0} | bridge=${:.2} | M={} | t_crit={:.4} | t_crit116={:.4} | sr_std(VIP0)={} | sr_std(MakerDiag)={}",
            base.track,
            xvenue.slices_processed,
            xvenue.bybit_candles_processed,
            xvenue.hl_candles_processed,
            xvenue.slippage_bps,
            xvenue.capital_usdt,
            xvenue.bridge_fee_usdt,
            base.m_hypotheses,
            base.t_crit,
            xvenue.t_crit_cum116,
            fmt_opt(base.sr_cross_std_vip0, 4),
            fmt_opt(base.sr_cross_std_diag, 4),
        ),
        format!(
            "{:<20} {:>5} {:>7} {:>8} {:>7} {:>8} {:>6} {:>7} {:>8} {:>8} {:>7} {:>6} {:>7} {:>7} {:>7} {:<18} {:<18}",
            "Strategy",
            "N",
            "Basis",
            "FundSpd",
            "Bridge",
            "CostVIP0",
            "RecCnt",
            "BrdgCnt",
            "AvgUtil%",
            "NetVIP0",
            "CAGR%",
            "AnnSR",
            "MaxDD%",
            "t_NW",
            "DSR",
            "Verdict",
            "Cum116"
        ),
        "-".repeat(178),
    ];

    for (vip0_cell, diag) in base.vip0_cells.iter().zip(xvenue.diagnostics.iter()) {
        lines.push(format!(
            "{:<20} {:>5} {:>7} {:>8} {:>7} {:>8} {:>6} {:>7} {:>8} {:>8} {:>7} {:>6} {:>6.2}% {:>7} {:>7} {:<18} {:<18}",
            vip0_cell.hypothesis,
            vip0_cell.n,
            fmt_opt(diag.mean_basis_bps, 3),
            fmt_opt(diag.mean_fund_spread_bps, 3),
            fmt_opt(diag.mean_bridge_bps, 3),
            fmt_opt(diag.mean_cost_vip0_bps, 3),
            diag.recenter_count,
            diag.bridge_count,
            fmt_opt(diag.avg_util_pct, 2),
            fmt_opt(vip0_cell.mean_bps, 3),
            fmt_opt(diag.vip0_metrics.cagr_pct, 2),
            fmt_opt(diag.vip0_metrics.ann_sharpe, 2),
            diag.vip0_metrics.max_drawdown_pct,
            fmt_opt(vip0_cell.t_nw, 3),
            fmt_opt(vip0_cell.dsr, 4),
            format!("{:?}", vip0_cell.verdict),
            format!("{:?}", diag.verdict_cum116),
        ));
    }
    lines.join("\n")
}

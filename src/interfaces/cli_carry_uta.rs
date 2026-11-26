use std::path::PathBuf;

use crate::application::falsifier_carry_uta::{
    run_carry_uta_falsification, CarryUtaConfig, CarryUtaReport,
};
use crate::domain::error::DomainError;
use crate::domain::types::Bps;
use crate::infrastructure::csv_candle_reader::CsvCandleReader;
use crate::interfaces::cli::{fmt_opt, take_arg, write_json_pretty};

fn parse_non_negative_f64(raw: &str, flag: &str) -> Result<f64, DomainError> {
    let parsed: f64 = raw
        .parse()
        .map_err(|_| DomainError::Cli(format!("invalid {flag} '{raw}'")))?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(DomainError::Cli(format!("{flag} must be finite and >= 0")));
    }
    Ok(parsed)
}

pub(crate) fn run_scan_carry_uta(args: &[String]) -> Result<String, DomainError> {
    let (mut linear_dir, mut spot_dir, mut out) = (None, None, None);
    let mut config = CarryUtaConfig::default();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--linear-dir" => {
                linear_dir = Some(PathBuf::from(take_arg(args, &mut index, "--linear-dir")?));
            }
            "--spot-dir" => {
                spot_dir = Some(PathBuf::from(take_arg(args, &mut index, "--spot-dir")?));
            }
            "--slippage-bps" => {
                let raw = take_arg(args, &mut index, "--slippage-bps")?;
                config.slippage_bps = Bps::new(parse_non_negative_f64(&raw, "--slippage-bps")?)?;
            }
            "--borrow-apr-pct" => {
                let raw = take_arg(args, &mut index, "--borrow-apr-pct")?;
                config.borrow_apr_pct = parse_non_negative_f64(&raw, "--borrow-apr-pct")?;
            }
            "--out" => out = Some(PathBuf::from(take_arg(args, &mut index, "--out")?)),
            flag => {
                return Err(DomainError::Cli(format!(
                    "unknown flag '{flag}' for scan-carry-uta"
                )))
            }
        }
        index += 1;
    }
    let linear_path =
        linear_dir.ok_or_else(|| DomainError::Cli("missing --linear-dir".to_string()))?;
    let spot_path = spot_dir.ok_or_else(|| DomainError::Cli("missing --spot-dir".to_string()))?;
    let mut linear_reader = CsvCandleReader::open(&linear_path)?;
    let mut spot_reader = CsvCandleReader::open(&spot_path)?;
    let report = run_carry_uta_falsification(&mut linear_reader, &mut spot_reader, &config)?;
    if let Some(path) = out.as_deref() {
        write_json_pretty(path, &report)?;
    }
    Ok(format_carry_uta_report_ascii(&report))
}

#[must_use]
pub fn format_carry_uta_report_ascii(uta_report: &CarryUtaReport) -> String {
    let base = &uta_report.report;
    let mut lines = vec![
        format!(
            "VORTEX-ALFA {} | slices={} | linear={} | spot={} | slip={:.2}bps | borrow={:.2}% | M={} | t_crit={:.4} | sr_std(VIP0)={} | sr_std(MakerDiag)={}",
            base.track,
            uta_report.slices_processed,
            uta_report.linear_candles_processed,
            uta_report.spot_candles_processed,
            uta_report.slippage_bps,
            uta_report.borrow_apr_pct,
            base.m_hypotheses,
            base.t_crit,
            fmt_opt(base.sr_cross_std_vip0, 4),
            fmt_opt(base.sr_cross_std_diag, 4),
        ),
        format!(
            "{:<22} {:>5} {:>7} {:>8} {:>7} {:>8} {:>8} {:>7} {:>8} {:>6} {:>8} {:>7} {:>6} {:>7} {:>7} {:>7} {:<18}",
            "Strategy",
            "N",
            "n_min",
            "BasisBps",
            "FundBps",
            "BorrowBp",
            "CostVIP0",
            "TurnL",
            "AvgUtil%",
            "RecCnt",
            "NetVIP0",
            "CAGR%",
            "AnnSR",
            "MaxDD%",
            "t_NW",
            "DSR",
            "Verdict"
        ),
        "-".repeat(162),
    ];

    for (vip0_cell, diag) in base.vip0_cells.iter().zip(uta_report.diagnostics.iter()) {
        let n_min_str = vip0_cell
            .n_min
            .map_or_else(|| "-".to_string(), |val| val.to_string());
        lines.push(format!(
            "{:<22} {:>5} {:>7} {:>8} {:>7} {:>8} {:>8} {:>7} {:>7} {:>6} {:>8} {:>7} {:>6} {:>6.2}% {:>7} {:>7} {:<18}",
            vip0_cell.hypothesis,
            vip0_cell.n,
            n_min_str,
            fmt_opt(diag.mean_basis_bps, 3),
            fmt_opt(diag.mean_fund_bps, 3),
            fmt_opt(diag.mean_borrow_bps, 3),
            fmt_opt(diag.mean_cost_vip0_bps, 3),
            fmt_opt(diag.avg_daily_turn_l, 4),
            fmt_opt(diag.avg_coverage_pct, 2),
            diag.recenter_count,
            fmt_opt(vip0_cell.mean_bps, 3),
            fmt_opt(diag.vip0_metrics.cagr_pct, 2),
            fmt_opt(diag.vip0_metrics.ann_sharpe, 2),
            diag.vip0_metrics.max_drawdown_pct,
            fmt_opt(vip0_cell.t_nw, 3),
            fmt_opt(vip0_cell.dsr, 4),
            format!("{:?}", vip0_cell.verdict),
        ));
    }
    lines.join("\n")
}

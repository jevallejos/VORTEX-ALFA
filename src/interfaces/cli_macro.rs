use std::path::PathBuf;

use crate::application::falsifier_macro::{
    run_macro_falsification, MacroConfig, MacroFalsificationReport,
};
use crate::domain::error::DomainError;
use crate::domain::types::Bps;
use crate::infrastructure::csv_candle_reader::CsvCandleReader;
use crate::interfaces::cli::{fmt_opt, take_arg, write_json_pretty};

fn parse_non_negative_bps(raw: &str, flag: &str) -> Result<Bps, DomainError> {
    let parsed: f64 = raw
        .parse()
        .map_err(|_| DomainError::Cli(format!("invalid {flag} '{raw}'")))?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(DomainError::Cli(format!("{flag} must be finite and >= 0")));
    }
    Bps::new(parsed)
}

pub(crate) fn run_scan_macro(args: &[String]) -> Result<String, DomainError> {
    let (mut linear_dir, mut spot_dir, mut out) = (None, None, None);
    let mut config = MacroConfig::default();
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
                config.slippage_bps = parse_non_negative_bps(&raw, "--slippage-bps")?;
            }
            "--out" => out = Some(PathBuf::from(take_arg(args, &mut index, "--out")?)),
            flag => {
                return Err(DomainError::Cli(format!(
                    "unknown flag '{flag}' for scan-macro"
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
    let report = run_macro_falsification(&mut linear_reader, &mut spot_reader, &config)?;
    if let Some(path) = out.as_deref() {
        write_json_pretty(path, &report)?;
    }
    Ok(format_macro_report_ascii(&report))
}

#[must_use]
pub fn format_macro_report_ascii(macro_report: &MacroFalsificationReport) -> String {
    let base = &macro_report.report;
    let mut lines = vec![
        format!(
            "VORTEX-ALFA {} | slices={} | linear={} | spot={} | slip={:.2}bps | M={} | t_crit={:.4} | sr_std(VIP0)={} | sr_std(MakerDiag)={}",
            base.track,
            macro_report.slices_processed,
            macro_report.linear_candles_processed,
            macro_report.spot_candles_processed,
            macro_report.slippage_bps,
            base.m_hypotheses,
            base.t_crit,
            fmt_opt(base.sr_cross_std_vip0, 4),
            fmt_opt(base.sr_cross_std_diag, 4),
        ),
        format!(
            "{:<26} {:>5} {:>7} {:>8} {:>7} {:>7} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7} {:>7} {:<18}",
            "Strategy",
            "N",
            "n_min",
            "Px/Bas",
            "Fund",
            "Turn",
            "VIP0Net",
            "MkrNet",
            "CAGR%",
            "AnnSR",
            "MaxDD%",
            "Calmar",
            "t_nw",
            "DSR",
            "Verdict(VIP0)"
        ),
        "-".repeat(148),
    ];

    for ((vip0_cell, maker_cell), diag) in base
        .vip0_cells
        .iter()
        .zip(base.diag_zero_cells.iter())
        .zip(macro_report.diagnostics.iter())
    {
        let n_min_str = vip0_cell
            .n_min
            .map_or_else(|| "-".to_string(), |val| val.to_string());
        lines.push(format!(
            "{:<26} {:>5} {:>7} {:>8} {:>7} {:>7} {:>8} {:>8} {:>8} {:>7} {:>6.2}% {:>7} {:>7} {:>7} {:<18}",
            vip0_cell.hypothesis,
            vip0_cell.n,
            n_min_str,
            fmt_opt(diag.mean_price_or_basis_bps, 2),
            fmt_opt(diag.mean_fund_bps, 2),
            fmt_opt(diag.avg_daily_turnover, 3),
            fmt_opt(vip0_cell.mean_bps, 2),
            fmt_opt(maker_cell.mean_bps, 2),
            fmt_opt(diag.vip0_metrics.cagr_pct, 2),
            fmt_opt(diag.vip0_metrics.ann_sharpe, 2),
            diag.vip0_metrics.max_drawdown_pct,
            fmt_opt(diag.vip0_metrics.calmar, 2),
            fmt_opt(vip0_cell.t_nw, 3),
            fmt_opt(vip0_cell.dsr, 4),
            format!("{:?}", vip0_cell.verdict),
        ));
    }

    lines.push(String::new());
    lines.push("PASSIVE SPOT BENCHMARKS (VIP-0 Spot 10.0 bps + slippage, EOF exit)".to_string());
    lines.push(format!(
        "{:<26} {:>5} {:>8} {:>7} {:>8} {:>8} {:>8} {:>7} {:>7} {:>7} {:>7}",
        "Baseline",
        "N",
        "PxBps",
        "Turn",
        "VIP0Net",
        "MkrNet",
        "CAGR%",
        "AnnSR",
        "MaxDD%",
        "Calmar",
        "t_nw"
    ));
    lines.push("-".repeat(110));
    for row in &macro_report.baselines {
        lines.push(format!(
            "{:<26} {:>5} {:>8} {:>7} {:>8} {:>8} {:>8} {:>7} {:>6.2}% {:>7} {:>7}",
            row.name,
            row.days_n,
            fmt_opt(row.mean_price_bps, 2),
            fmt_opt(row.avg_daily_turnover, 4),
            fmt_opt(row.mean_net_vip0_bps, 2),
            fmt_opt(row.mean_net_maker_bps, 2),
            fmt_opt(row.vip0_metrics.cagr_pct, 2),
            fmt_opt(row.vip0_metrics.ann_sharpe, 2),
            row.vip0_metrics.max_drawdown_pct,
            fmt_opt(row.vip0_metrics.calmar, 2),
            fmt_opt(row.t_nw_vip0, 3),
        ));
    }
    lines.join("\n")
}

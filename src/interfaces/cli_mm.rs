use std::path::PathBuf;

use crate::application::falsifier_mm::{
    default_mm_grid, run_mm_falsification, MmFalsificationReport, MmFalsifierConfig,
};
use crate::domain::error::DomainError;
use crate::infrastructure::tape_reader::StreamingTapeReader;
use crate::interfaces::cli::{fmt_opt, parse_tape_format, take_arg, write_json_pretty};

fn parse_pos_f64(raw: &str, flag: &str) -> Result<f64, DomainError> {
    let val: f64 = raw
        .parse()
        .map_err(|_| DomainError::Cli(format!("invalid {flag} '{raw}'")))?;
    if !val.is_finite() || val <= 0.0 {
        return Err(DomainError::Cli(format!("{flag} must be finite and > 0")));
    }
    Ok(val)
}

pub(crate) fn run_scan_mm(args: &[String]) -> Result<String, DomainError> {
    let (mut format, mut input, mut out) = (None, None, None);
    let mut cfg = MmFalsifierConfig::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--format" => format = Some(parse_tape_format(&take_arg(args, &mut i, "--format")?)?),
            "--input" => input = Some(PathBuf::from(take_arg(args, &mut i, "--input")?)),
            "--capital-usdt" => {
                cfg.capital_usdt =
                    parse_pos_f64(&take_arg(args, &mut i, "--capital-usdt")?, "--capital-usdt")?;
            }
            "--clip-usdt" => {
                cfg.clip_usdt =
                    parse_pos_f64(&take_arg(args, &mut i, "--clip-usdt")?, "--clip-usdt")?;
            }
            "--max-inv-clips" => {
                cfg.max_inv_clips = parse_pos_f64(
                    &take_arg(args, &mut i, "--max-inv-clips")?,
                    "--max-inv-clips",
                )?;
            }
            "--out" => out = Some(PathBuf::from(take_arg(args, &mut i, "--out")?)),
            flag => {
                return Err(DomainError::Cli(format!(
                    "unknown flag '{flag}' for scan-mm"
                )))
            }
        }
        i += 1;
    }
    let fmt = format.ok_or_else(|| DomainError::Cli("missing --format".to_string()))?;
    let inp = input.ok_or_else(|| DomainError::Cli("missing --input".to_string()))?;
    cfg.grid = default_mm_grid(cfg.clip_usdt, cfg.max_inv_clips);
    cfg.validate()?;
    let mut reader = StreamingTapeReader::open(&inp, fmt)?;
    let report = run_mm_falsification(&mut reader, &cfg)?;
    if let Some(path) = out.as_deref() {
        write_json_pretty(path, &report)?;
    }
    Ok(format_mm_report_ascii(&report))
}

#[must_use]
pub fn format_mm_report_ascii(r: &MmFalsificationReport) -> String {
    let mut lines = vec![
        format!(
            "VORTEX-ALFA {} | events={} | capital=${:.2} | clip=${:.2} | max_clips={:.1} | bucket={:.0}s | rtt={:.1}ms | M={} | t_crit={:.4} | sr_std(VIP0)={}",
            r.track,
            r.events_processed,
            r.capital_usdt,
            r.clip_usdt,
            r.max_inv_clips,
            r.bucket_s,
            r.rtt_ms,
            r.m_hypotheses,
            r.t_crit,
            fmt_opt(r.sr_cross_std_vip0, 4),
        ),
        format!(
            "{:<15} {:>4} {:>7} {:>5} {:>4} {:>10} {:>9} {:>9} {:>7} {:>7} {:>7} {:>7} {:>8} {:>7} {:>7} {:>5} {:>7} {:>7} {:<16}",
            "Config", "Bkt", "Fills", "Rej", "Adv", "VolUSDT", "Gross$", "NetVIP0$", "SpCap", "MO1s", "MO5s", "LVR5s", "Peak$", "EOF$", "BrkMkr", "Drift", "t_nw", "DSR", "Verdict(VIP0)"
        ),
        "-".repeat(170),
    ];
    for (cfg, cell) in r.configs.iter().zip(r.vip0_cells.iter()) {
        lines.push(format!(
            "{:<15} {:>4} {:>7} {:>5} {:>4} {:>10.1} {:>9.2} {:>9.2} {:>7} {:>7} {:>7} {:>7} {:>8.1} {:>7.2} {:>7} {:>5} {:>7} {:>7} {:<16}",
            cfg.config_name,
            cfg.buckets_n,
            cfg.fills_total,
            cfg.post_only_rejects,
            cfg.adverse_in_flight_fills,
            cfg.total_maker_volume_usdt,
            cfg.gross_pnl_usdt,
            cfg.net_pnl_vip0_usdt,
            fmt_opt(cfg.mean_spread_capture_bps, 2),
            fmt_opt(cfg.mean_markout_1s_bps, 2),
            fmt_opt(cfg.mean_markout_5s_bps, 2),
            fmt_opt(cfg.mean_lvr_5s_bps, 2),
            cfg.peak_gross_inv_usdt,
            cfg.eof_liq_cost_usdt,
            fmt_opt(cfg.breakeven_maker_bps, 2),
            cfg.accounting_drift_e8,
            fmt_opt(cell.t_nw, 3),
            fmt_opt(cell.dsr, 4),
            format!("{:?}", cell.verdict),
        ));
    }
    lines.push(String::new());
    lines.push(
        "FEE FRONTIER DIAGNOSTICS (only VIP0_Maker* is binding for GateVerdict::Pass)".to_string(),
    );
    lines.push(format!(
        "{:<15} {:<11} {:<5} {:>7} {:>10} {:>9} {:>8} {:>8} {:>7} {:<18}",
        "Config", "Profile", "Bind", "MkrBps", "Net$", "MeanBps", "MaxDD", "t_nw", "DSR", "Verdict"
    ));
    lines.push("-".repeat(108));
    for cfg in &r.configs {
        for f in &cfg.frontier {
            lines.push(format!(
                "{:<15} {:<11} {:<5} {:>7.2} {:>10.2} {:>9} {:>8.1} {:>8} {:>7} {:<18}",
                cfg.config_name,
                f.profile,
                if f.is_binding { "YES*" } else { "no" },
                f.maker_fee_bps,
                f.net_pnl_usdt,
                fmt_opt(f.mean_bucket_bps, 3),
                f.max_drawdown_bps,
                fmt_opt(f.t_nw, 3),
                fmt_opt(f.dsr, 4),
                format!("{:?}", f.verdict),
            ));
        }
    }
    lines.join("\n")
}

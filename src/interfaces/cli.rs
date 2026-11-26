use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::application::falsifier_meso::{run_meso_falsification, MesoFalsifierConfig};
use crate::application::falsifier_micro::{run_micro_falsification, MicroFalsifierConfig};
use crate::application::falsifier_xs::{
    run_xs_falsification, XsFalsificationReport, XsFalsifierConfig,
};
use crate::domain::error::DomainError;
use crate::domain::ports::ReportSink;
use crate::domain::stats::{CellEvaluation, FalsificationReport};
use crate::domain::types::Bps;
use crate::infrastructure::csv_candle_reader::CsvCandleReader;
use crate::infrastructure::tape_reader::{StreamingTapeReader, TapeFormat};
use crate::infrastructure::vkl1_reader::StreamingVkl1Reader;

fn write_json_pretty<T: Serialize>(path: &Path, value: &T) -> Result<(), DomainError> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| DomainError::Io(format!("json serialize failed: {e}")))?;
    let mut file = File::create(path)
        .map_err(|e| DomainError::Io(format!("create {}: {e}", path.display())))?;
    file.write_all(json.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|e| DomainError::Io(format!("write {}: {e}", path.display())))
}

struct JsonFileReportSink {
    path: PathBuf,
}

impl ReportSink for JsonFileReportSink {
    fn write_report(&mut self, report: &FalsificationReport) -> Result<(), DomainError> {
        write_json_pretty(&self.path, report)
    }
}

pub fn run_cli<I, S>(args: I) -> Result<String, DomainError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let raw: Vec<String> = args.into_iter().map(Into::into).collect();
    if raw.len() < 2 {
        return Err(DomainError::Cli(usage_text()));
    }
    match raw[1].as_str() {
        "scan-micro" => run_scan_micro(&raw[2..]),
        "scan-meso" => run_scan_meso(&raw[2..]),
        "scan-xs" => run_scan_xs(&raw[2..]),
        "-h" | "--help" | "help" => Ok(usage_text()),
        other => Err(DomainError::Cli(format!(
            "unknown subcommand '{other}'\n{}",
            usage_text()
        ))),
    }
}

fn usage_text() -> String {
    "Usage:\n  vortex-alfa scan-micro --format <par2|vpz1|jsonl> --input <path> [--out <report.json>]\n  vortex-alfa scan-meso --format <csv|vkl1> --input <path> [--slippage-bps <f64>] [--out <report.json>]\n  vortex-alfa scan-xs --format <csv|vkl1> --input <path> [--top-k <usize[,usize...]>] [--slippage-bps <f64>] [--include-btc] [--out <report.json>]"
        .to_string()
}

fn take_arg(args: &[String], idx: &mut usize, flag: &str) -> Result<String, DomainError> {
    *idx += 1;
    args.get(*idx)
        .cloned()
        .ok_or_else(|| DomainError::Cli(format!("missing value for {flag}")))
}

fn parse_slippage_bps(raw: &str) -> Result<Bps, DomainError> {
    let parsed: f64 = raw
        .parse()
        .map_err(|_| DomainError::Cli(format!("invalid --slippage-bps '{raw}'")))?;
    if !parsed.is_finite() || parsed < 0.0 {
        return Err(DomainError::Cli("--slippage-bps must be >= 0".to_string()));
    }
    Bps::new(parsed)
}

fn parse_top_ks(raw: &str) -> Result<Vec<usize>, DomainError> {
    let mut out = Vec::new();
    for part in raw.split(',').map(str::trim) {
        let k: usize = part
            .parse()
            .map_err(|_| DomainError::Cli(format!("invalid --top-k '{part}'")))?;
        if k == 0 {
            return Err(DomainError::Cli("--top-k values must be >= 1".to_string()));
        }
        if out.contains(&k) {
            return Err(DomainError::Cli(format!("duplicate --top-k value {k}")));
        }
        out.push(k);
    }
    if out.is_empty() {
        return Err(DomainError::Cli("--top-k must not be empty".to_string()));
    }
    Ok(out)
}

fn run_scan_micro(args: &[String]) -> Result<String, DomainError> {
    let (mut format, mut input, mut out) = (None, None, None);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--format" => format = Some(parse_tape_format(&take_arg(args, &mut i, "--format")?)?),
            "--input" => input = Some(PathBuf::from(take_arg(args, &mut i, "--input")?)),
            "--out" => out = Some(PathBuf::from(take_arg(args, &mut i, "--out")?)),
            flag => {
                return Err(DomainError::Cli(format!(
                    "unknown flag '{flag}' for scan-micro"
                )))
            }
        }
        i += 1;
    }
    let fmt = format.ok_or_else(|| DomainError::Cli("missing --format".to_string()))?;
    let inp = input.ok_or_else(|| DomainError::Cli("missing --input".to_string()))?;
    let mut reader = StreamingTapeReader::open(&inp, fmt)?;
    let report = run_micro_falsification(&mut reader, &MicroFalsifierConfig::default())?;
    emit_report(&report, out.as_deref())
}

fn run_scan_meso(args: &[String]) -> Result<String, DomainError> {
    let (mut format, mut input, mut out) = (None, None, None);
    let mut slippage_bps = Bps::new(1.5)?;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--format" => format = Some(take_arg(args, &mut i, "--format")?),
            "--input" => input = Some(PathBuf::from(take_arg(args, &mut i, "--input")?)),
            "--slippage-bps" => {
                slippage_bps = parse_slippage_bps(&take_arg(args, &mut i, "--slippage-bps")?)?;
            }
            "--out" => out = Some(PathBuf::from(take_arg(args, &mut i, "--out")?)),
            flag => {
                return Err(DomainError::Cli(format!(
                    "unknown flag '{flag}' for scan-meso"
                )))
            }
        }
        i += 1;
    }
    let fmt = format.ok_or_else(|| DomainError::Cli("missing --format".to_string()))?;
    let inp = input.ok_or_else(|| DomainError::Cli("missing --input".to_string()))?;
    let cfg = MesoFalsifierConfig {
        slippage_bps,
        ..MesoFalsifierConfig::default()
    };
    let report = match fmt.to_ascii_lowercase().as_str() {
        "csv" => run_meso_falsification(&mut CsvCandleReader::open(&inp)?, &cfg)?,
        "vkl1" => run_meso_falsification(&mut StreamingVkl1Reader::open(&inp)?, &cfg)?,
        other => {
            return Err(DomainError::Cli(format!(
                "unsupported meso format '{other}'"
            )))
        }
    };
    emit_report(&report, out.as_deref())
}

fn run_scan_xs(args: &[String]) -> Result<String, DomainError> {
    let (mut format, mut input, mut out) = (None, None, None);
    let mut cfg = XsFalsifierConfig::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--format" => format = Some(take_arg(args, &mut i, "--format")?),
            "--input" => input = Some(PathBuf::from(take_arg(args, &mut i, "--input")?)),
            "--top-k" => cfg.top_ks = parse_top_ks(&take_arg(args, &mut i, "--top-k")?)?,
            "--slippage-bps" => {
                cfg.slippage_bps = parse_slippage_bps(&take_arg(args, &mut i, "--slippage-bps")?)?;
            }
            "--include-btc" => cfg.signal.exclude_benchmark_from_trading = false,
            "--out" => out = Some(PathBuf::from(take_arg(args, &mut i, "--out")?)),
            flag => {
                return Err(DomainError::Cli(format!(
                    "unknown flag '{flag}' for scan-xs"
                )))
            }
        }
        i += 1;
    }
    let fmt = format.ok_or_else(|| DomainError::Cli("missing --format".to_string()))?;
    let inp = input.ok_or_else(|| DomainError::Cli("missing --input".to_string()))?;
    let report = match fmt.to_ascii_lowercase().as_str() {
        "csv" => run_xs_falsification(&mut CsvCandleReader::open(&inp)?, &cfg)?,
        "vkl1" => run_xs_falsification(&mut StreamingVkl1Reader::open(&inp)?, &cfg)?,
        other => return Err(DomainError::Cli(format!("unsupported xs format '{other}'"))),
    };
    if let Some(path) = out.as_deref() {
        write_json_pretty(path, &report)?;
    }
    Ok(format_xs_report_ascii(&report))
}

fn parse_tape_format(raw: &str) -> Result<TapeFormat, DomainError> {
    match raw.to_ascii_lowercase().as_str() {
        "par2" => Ok(TapeFormat::Par2),
        "vpz1" | "par2z" => Ok(TapeFormat::Vpz1),
        "jsonl" => Ok(TapeFormat::Jsonl),
        other => Err(DomainError::Cli(format!(
            "unsupported micro format '{other}'"
        ))),
    }
}

fn emit_report(report: &FalsificationReport, out: Option<&Path>) -> Result<String, DomainError> {
    if let Some(path) = out {
        let mut sink = JsonFileReportSink {
            path: path.to_path_buf(),
        };
        sink.write_report(report)?;
    }
    Ok(format_report_ascii(report))
}

#[must_use]
pub fn format_report_ascii(report: &FalsificationReport) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "VORTEX-ALFA {} | events={} | M={} | t_crit={:.4} | sr_std(VIP-0)={} | sr_std(DiagZero)={}",
        report.track,
        report.events_processed,
        report.m_hypotheses,
        report.t_crit,
        fmt_opt(report.sr_cross_std_vip0, 4),
        fmt_opt(report.sr_cross_std_diag, 4),
    ));
    lines.push(format!(
        "{:<24} {:<8} {:<10} {:>6} {:>8} {:>9} {:>8} {:>8} {:>7} {:<18}",
        "Hypothesis",
        "Horizon",
        "Profile",
        "N",
        "n_min",
        "MeanBps",
        "t_nw",
        "t_crit",
        "DSR",
        "Verdict"
    ));
    lines.push("-".repeat(118));
    for (vip, diag) in report.vip0_cells.iter().zip(report.diag_zero_cells.iter()) {
        push_cell_line(&mut lines, vip);
        push_cell_line(&mut lines, diag);
    }
    lines.join("\n")
}

#[must_use]
pub fn format_xs_report_ascii(xs: &XsFalsificationReport) -> String {
    let r = &xs.report;
    let mut lines = vec![
        format!(
            "VORTEX-ALFA {} | slices={} | events={} | M={} | t_crit={:.4} | sr_std(VIP0)={} | sr_std(MakerDiag)={}",
            r.track,
            xs.slices_processed,
            r.events_processed,
            r.m_hypotheses,
            r.t_crit,
            fmt_opt(r.sr_cross_std_vip0, 4),
            fmt_opt(r.sr_cross_std_diag, 4),
        ),
        format!(
            "{:<22} {:<4} {:>5} {:>8} {:>8} {:>6} {:>9} {:>9} {:>7} {:>9} {:>7} {:>7} {:<18}",
            "Hypothesis", "Hor", "N", "Price", "Fund", "Turn", "VIP0Net", "MkrNet", "AnnSR", "MaxDD", "t_nw", "DSR", "Verdict"
        ),
        "-".repeat(134),
    ];
    for ((v, m), d) in r
        .vip0_cells
        .iter()
        .zip(r.diag_zero_cells.iter())
        .zip(xs.diagnostics.iter())
    {
        lines.push(format!(
            "{:<22} {:<4} {:>5} {:>8} {:>8} {:>6} {:>9} {:>9} {:>7} {:>9.1} {:>7} {:>7} {:<18}",
            v.hypothesis,
            v.horizon,
            v.n,
            fmt_opt(d.mean_price_bps, 2),
            fmt_opt(d.mean_fund_bps, 2),
            fmt_opt(d.mean_turnover, 3),
            fmt_opt(v.mean_bps, 2),
            fmt_opt(m.mean_bps, 2),
            fmt_opt(d.ann_sharpe, 2),
            d.max_drawdown_bps,
            fmt_opt(v.t_nw, 3),
            fmt_opt(v.dsr, 4),
            format!("{:?}", v.verdict),
        ));
    }
    lines.join("\n")
}

fn push_cell_line(lines: &mut Vec<String>, c: &CellEvaluation) {
    let tag = if c.is_binding { "VIP-0*" } else { "DiagZero" };
    let n_min_str = c.n_min.map_or_else(|| "-".to_string(), |v| v.to_string());
    lines.push(format!(
        "{:<24} {:<8} {:<10} {:>6} {:>8} {:>9} {:>8} {:>8} {:>7} {:<18}",
        c.hypothesis,
        c.horizon,
        tag,
        c.n,
        n_min_str,
        fmt_opt(c.mean_bps, 3),
        fmt_opt(c.t_nw, 3),
        format!("{:.3}", c.t_crit),
        fmt_opt(c.dsr, 4),
        format!("{:?}", c.verdict),
    ));
}

fn fmt_opt(val: Option<f64>, prec: usize) -> String {
    match val {
        Some(v) => format!("{v:.prec$}"),
        None => "-".to_string(),
    }
}

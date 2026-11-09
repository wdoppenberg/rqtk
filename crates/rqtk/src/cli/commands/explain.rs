use console::style;
use rqtk_core::Severity;
use rqtk_core::rules::{RULES, Rule, rule};

use crate::cli::error::CliError;
use crate::cli::output::{self, Ctx, Exit, Usage};

/// `rqtk explain --json`: the rules asked for, or all of them.
#[derive(serde::Serialize)]
pub struct Rules<'a> {
    pub rules: &'a [&'a Rule],
}

pub fn run(ctx: &Ctx, code: Option<&str>) -> Result<Exit, CliError> {
    let rules: Vec<&Rule> = match code {
        Some(code) => {
            let code = code.to_ascii_uppercase();
            vec![rule(&code).ok_or_else(|| {
                Usage(format!(
                    "unknown rule `{code}`; run `rqtk explain` to list rules"
                ))
            })?]
        }
        None => RULES.iter().collect(),
    };
    if ctx.json() {
        output::document("explain", &Rules { rules: &rules })?;
        return Ok(Exit::Ok);
    }
    for r in rules {
        let severity = match r.severity {
            Severity::Error => style("error").red(),
            Severity::Warning => style("warning").yellow(),
        };
        println!("  {}  {:<8} {}", style(r.code).bold(), severity, r.summary);
        if code.is_some() {
            println!("\n  Fix: {}", r.fix);
        }
    }
    Ok(Exit::Ok)
}

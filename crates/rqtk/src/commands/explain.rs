use std::error::Error;

use console::style;
use rqtk_core::Severity;
use rqtk_core::rules::{RULES, Rule, rule};

use crate::output::{self, Ctx, Exit, Usage};

pub fn run(ctx: &Ctx, code: Option<&str>) -> Result<Exit, Box<dyn Error>> {
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
        match rules.as_slice() {
            [one] if code.is_some() => output::json(one)?,
            all => output::json(all)?,
        }
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

use rqtk_core::RequirementSet;

use crate::cli::error::CliError;
use crate::cli::output::{Ctx, Exit};

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum GraphFormat {
    /// Graphviz DOT.
    Dot,
}

pub fn run(ctx: &Ctx, format: GraphFormat) -> Result<Exit, CliError> {
    ctx.require_text("graph")?;
    let (set, _) = RequirementSet::load_from_repo_root(&ctx.root)?.validate();
    match format {
        GraphFormat::Dot => print!("{}", set.to_dot()),
    }
    Ok(Exit::Ok)
}

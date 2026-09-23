use std::error::Error;

use rqtk_core::RequirementSet;

use crate::output::{Ctx, Exit};

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum GraphFormat {
    /// Graphviz DOT.
    Dot,
}

pub fn run(ctx: &Ctx, format: GraphFormat) -> Result<Exit, Box<dyn Error>> {
    ctx.require_text("graph")?;
    let (set, _) = RequirementSet::load_from_repo_root(&ctx.root)?.validate();
    match format {
        GraphFormat::Dot => print!("{}", set.to_dot()),
    }
    Ok(Exit::Ok)
}

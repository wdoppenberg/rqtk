use rqtk_core::{Loaded, RequirementSet};

fn main() {
    let loaded: RequirementSet<Loaded> = todo!();
    let _ = loaded.coverage_gaps();
}

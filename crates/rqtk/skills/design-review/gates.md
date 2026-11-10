# Gates

The default gates, in order. Each criterion names where its facts come from: **lint**, **coverage**, **export** (`rqtk export --format json`), **trace**, **impact**, or **judgement** when only reading can decide it. A criterion holds for every item in scope or it is not met.

## requirements: Requirements review

Are the right things asked for, of the system as a whole? Its own items: stakeholders, needs and root-category requirements.

1. Every stakeholder in scope raises at least one need, and every need names its stakeholders. (export)
2. Every need in scope is satisfied by at least one root-category requirement. (coverage)
3. Every requirement has a rationale, a verification method and success criteria, and lints clean. (lint)
4. Every statement is one sentence with the shall keyword and a criterion a test, inspection, analysis or demonstration could decide. (judgement)
5. No two requirements contradict or duplicate each other, and none demands something no need asks for. (judgement)
6. Behaviour other systems rely on is an interface requirement; what this system takes from other repositories is a `consumes` link. (export, judgement)
7. Every TBD and TBR is flagged, with the open question in `notes`. (export)

## pdr: Preliminary design review

Is the system split into parts that together do what it must? Its own items: non-leaf requirements below the root.

1. The requirements review passed for the scope. (records)
2. Every root-category requirement in scope has children, or the record says why one component can implement it and one level can verify it as it stands. (trace, judgement)
3. The children of each requirement together demand everything it demands, and each child's rationale says which part it takes. (judgement)
4. No orphans and no circular traces; each child's category sits one level below its parent's. (lint)
5. Every boundary between components is an interface requirement with `[[interface.specs]]`; a draft spec is enough here. (export, judgement)
6. Every requirement is allocated to `components` or `subsystems`. (export)
7. Every requirement has a verification method, level and success criteria, and every leaf has at least one activity: no Gap. (coverage)
8. Every safety-critical or high-criticality requirement has `[risk]` with hazards and mitigations. (export)
9. Every TBD and TBR has a plan to close it by CDR, in `notes`. (export)
10. Design documents, where there are any, agree with the decomposition. (judgement)

## cdr: Critical design review

Is every leaf ready to build and to verify? Its own items: leaves.

1. PDR passed for the scope. (records)
2. No TBD or TBR remains. (export)
3. Every leaf is allocated to `software_modules` or `source_files`. (export)
4. Every leaf's activities fit its statement in method and level, and each can be decided pass or fail. Planned is fine; Gap is not. (coverage, judgement)
5. Interface specs are final: every spec entry resolves, and its content agrees with the statement. (lint, judgement)
6. Each leaf can be built as written, with no open design choice that would change its statement. (judgement)
7. Code that already exists for the scope does what its leaves say. (judgement)
8. Lint has no errors in scope. (lint)

## readiness: Readiness review

Is it built, proven, and what was asked for? It closes the scope.

1. CDR passed for the scope. (records)
2. Lint has no findings in scope. (lint)
3. Every requirement in scope is Verified: nothing Suspect, Failed, In Progress, Planned or Gap. (coverage)
4. Every activity no test can check has `status`, `executed_at` and `evidence`. (export)
5. Every need in scope is satisfied by Verified requirements. (coverage)
6. Every change since the last baseline has been judged: changed requirements re-verified, downstream ones reviewed, changed consumed interfaces checked. (impact, judgement)
7. The linked tests check what each statement demands, through the public interface, rather than passing by construction. Sample the leaves, and read every changed one. (judgement)
8. Every action from earlier records that cover the scope is closed. (records)

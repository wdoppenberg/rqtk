use chrono::Local;
use rqtk_core::{RequirementSet, Validated};
use std::collections::BTreeMap;

// ----------------------------------------------------------------
// Static Typst template (appended after the generated data block).
// r##"..."## is required because Typst color literals contain "#RRGGBB"
// which would prematurely terminate r#"..."#.
// Requires typst >= 0.12 (context keyword, outlined heading param).
// ----------------------------------------------------------------
const TEMPLATE: &str = r##"
// ============================================================
// rqtk Requirements Specification Report
// ============================================================

// ---- Colour palette ----
#let c-navy    = rgb("#1a3a5c")
#let c-blue    = rgb("#2563a8")
#let c-sky     = rgb("#3b82c4")
#let c-muted   = luma(120)
#let c-surface = luma(248)
#let c-border  = luma(215)
#let c-ok      = rgb("#15803d")
#let c-warn    = rgb("#92400e")
#let c-danger  = rgb("#991b1b")
#let c-info    = rgb("#1e40af")

// ---- Helper functions ----
#let chip(label, fill-col) = box(
  fill:   fill-col.lighten(75%),
  stroke: 0.5pt + fill-col.lighten(30%),
  radius: 2pt,
  inset:  (x: 5pt, y: 2pt),
  text(size: 7.5pt, weight: "semibold", fill: fill-col, label),
)

#let state-col(s) = {
  if s in ("Approved", "Verified", "Implemented") { c-ok }
  else if s == "Review"     { c-warn }
  else if s == "Deprecated" { c-danger }
  else { c-muted }
}

#let priority-col(p) = {
  if p == "Critical" { c-danger }
  else if p == "High"   { c-warn }
  else if p == "Medium" { c-info }
  else { c-muted }
}

#let fl(label) = text(size: 7.5pt, weight: "semibold", fill: c-muted, upper(label))

// ---- Heading styles ----
#show heading.where(level: 1): it => {
  pagebreak(weak: true)
  v(0.4em)
  block(
    width:  100%,
    fill:   c-navy,
    radius: 5pt,
    inset:  (x: 18pt, y: 14pt),
  )[
    #set text(fill: white, size: 17pt, weight: "bold")
    #if it.numbering != none [
      #context counter(heading).display(it.numbering)
      #h(0.4em)
    ]
    #it.body
  ]
  v(0.4em)
}

#show heading.where(level: 2): it => {
  v(0.8em)
  block(width: 100%)[
    #set text(size: 13pt, weight: "semibold", fill: c-blue)
    #if it.numbering != none [
      #context counter(heading).display(it.numbering)
      #h(0.3em)
    ]
    #it.body
    #v(0.15em)
    #line(length: 100%, stroke: 1pt + c-sky.lighten(50%))
  ]
  v(0.3em)
}

#show heading.where(level: 3): it => {
  v(0.5em)
  text(size: 11pt, weight: "semibold", fill: luma(50), it.body)
  v(0.15em)
}

// ---- Requirement card ----
#let req-card(req) = {
  let sc = state-col(req.state)
  block(
    width:     100%,
    breakable: true,
    stroke:    (left: 3pt + sc, rest: 0.5pt + c-border),
    radius:    (top-right: 4pt, bottom-right: 4pt),
    inset:     0pt,
    fill:      c-surface,
    below:     1em,
  )[
    // header
    #block(
      width:  100%,
      fill:   white,
      inset:  (x: 14pt, top: 10pt, bottom: 8pt),
      stroke: (bottom: 0.5pt + c-border),
      radius: (top-right: 4pt),
    )[
      #grid(
        columns: (auto, 1fr, auto),
        column-gutter: 10pt,
        align: (left + horizon, left + horizon, right + horizon),
        text(size: 10pt, weight: "bold", fill: c-blue, font: "Courier New", req.id),
        text(size: 10.5pt, weight: "semibold", req.title),
        [#chip(req.state, state-col(req.state)) #h(4pt) #chip(req.priority, priority-col(req.priority)) #h(4pt) #chip(req.req-type, c-info)],
      )
    ]
    // body
    #block(width: 100%, inset: (x: 14pt, y: 12pt))[
      #fl("Statement")
      #v(0.2em)
      #req.text

      #if req.rationale != "" [
        #v(0.6em)
        #fl("Rationale")
        #v(0.2em)
        #text(fill: luma(60), req.rationale)
      ]

      #v(0.8em)
      #rect(
        width:  100%,
        fill:   luma(242),
        stroke: none,
        radius: 3pt,
        inset:  (x: 12pt, y: 9pt),
      )[
        #grid(
          columns: (1fr, 1fr, 1fr, 1fr),
          column-gutter: 8pt,
          stack(dir: ttb, fl("Category"),    v(3pt), text(size: 9.5pt, req.category)),
          stack(dir: ttb, fl("Hash"),        v(3pt), text(size: 9.5pt, font: "Courier New", req.hash)),
          stack(dir: ttb, fl("Criticality"), v(3pt), text(size: 9.5pt, req.criticality)),
          stack(dir: ttb, fl("Verify"),      v(3pt), text(size: 9.5pt, req.verification-method)),
        )
        #if req.parents.len() > 0 [
          #v(6pt)
          #stack(dir: ttb, fl("Parents"), v(3pt),
            text(size: 9pt, font: "Courier New", fill: c-blue, req.parents.join("  \u{b7}  ")))
        ]
        #if req.notes != "" [
          #v(6pt)
          #stack(dir: ttb, fl("Notes"), v(3pt),
            text(size: 9pt, fill: luma(55), req.notes))
        ]
      ]

      #if req.parameters.len() > 0 [
        #v(0.6em)
        #fl("Parameters")
        #v(0.3em)
        #table(
          columns: (2fr, 1fr, 2fr, 1fr, 1fr),
          fill: (_, row) => if row == 0 { luma(230) } else if calc.odd(row) { white } else { luma(250) },
          stroke: 0.5pt + c-border,
          inset: (x: 8pt, y: 5pt),
          [*Name*], [*Op*], [*Value*], [*Unit*], [*Tolerance*],
          ..req.parameters.map(p => (p.name, p.operator, p.value, p.unit, p.tolerance)).flatten(),
        )
      ]

      #if req.activities.len() > 0 [
        #v(0.6em)
        #fl("Verification Activities")
        #v(0.3em)
        #for act in req.activities [
          #block(
            width:  100%,
            inset:  (left: 10pt, y: 4pt),
            stroke: (left: 2pt + c-border),
            below:  5pt,
          )[
            #grid(
              columns: (auto, 1fr, auto),
              column-gutter: 8pt,
              text(size: 9pt, weight: "semibold", font: "Courier New", act.id),
              text(size: 9pt, act.name),
              chip(act.status,
                if act.status in ("Passed", "Completed") { c-ok }
                else if act.status == "Failed"           { c-danger }
                else                                     { c-muted }),
            )
            #if act.expected-result != "" [
              #v(2pt)
              #text(size: 8.5pt, fill: c-muted)[Expected: #act.expected-result]
            ]
          ]
        ]
      ]
    ]
  ]
}

// ================================================================
// COVER PAGE
// ================================================================
#set page(
  paper:     "a4",
  margin:    (x: 3cm, y: 3cm),
  numbering: none,
  header:    none,
  footer:    none,
)
#set text(font: ("Linux Libertine", "Georgia", "serif"), size: 10.5pt, lang: "en")

#block(
  width:  100%,
  fill:   c-navy,
  radius: 8pt,
  inset:  (x: 2.5cm, top: 2.5cm, bottom: 2.5cm),
)[
  #set text(fill: white)
  #text(size: 8pt, tracking: 2pt, fill: c-sky.lighten(30%), upper("Requirements Specification"))
  #v(0.8em)
  #text(size: 34pt, weight: "bold", meta.project)
  #if meta.description != "" [
    #v(0.4em)
    #text(size: 12pt, fill: c-sky.lighten(20%), meta.description)
  ]
]

#v(2.5cm)

#grid(
  columns:       (1fr, 1fr),
  row-gutter:    20pt,
  column-gutter: 24pt,
  stack(dir: ttb, fl("Version"),        v(5pt), text(size: 11pt, meta.version)),
  stack(dir: ttb, fl("Date"),           v(5pt), text(size: 11pt, meta.date)),
  stack(dir: ttb, fl("Classification"), v(5pt), text(size: 11pt, meta.classification)),
  stack(dir: ttb, fl("Mission Phase"),  v(5pt), text(size: 11pt, if meta.mission-phase == "" { "\u{2014}" } else { meta.mission-phase })),
  stack(dir: ttb, fl("Organization"),   v(5pt), text(size: 11pt, if meta.organization   == "" { "\u{2014}" } else { meta.organization   })),
  stack(dir: ttb, fl("Requirements"),   v(5pt), [
    #text(size: 22pt, weight: "bold", fill: c-navy, str(stats.total))
    #text(size: 9pt,  fill: c-muted,  "  total")
  ]),
)

#v(3cm)
#line(length: 100%, stroke: 0.5pt + c-border)
#v(0.5em)
#text(size: 8pt, fill: c-muted)[Generated by rqtk-report #sym.dot #meta.date]

#pagebreak()

// ================================================================
// MAIN PAGE SETUP
// ================================================================
#counter(page).update(1)
#set page(
  paper:     "a4",
  margin:    (top: 2.5cm, bottom: 2.5cm, left: 2.5cm, right: 2.0cm),
  numbering: "1",
  header: context {
    if counter(page).get().first() >= 1 [
      #set text(size: 8.5pt, fill: c-muted)
      #meta.project #sym.space #sym.dash.en #sym.space Requirements Specification
      #h(1fr)
      v#meta.version #h(0.2em)#sym.dot#h(0.2em)#meta.classification
      #v(-7pt)
      #line(length: 100%, stroke: 0.3pt + c-border)
    ]
  },
  footer: context [
    #line(length: 100%, stroke: 0.3pt + c-border)
    #v(-5pt)
    #set text(size: 8.5pt, fill: c-muted)
    #h(1fr)
    #counter(page).display("1 of 1", both: true)
  ],
)
#set par(justify: true, leading: 0.65em, spacing: 1.1em)
#set heading(numbering: "1.1")

// ================================================================
// TABLE OF CONTENTS
// ================================================================
#block(
  width:  100%,
  fill:   c-navy,
  radius: 5pt,
  inset:  (x: 18pt, y: 14pt),
)[
  #set text(fill: white, size: 17pt, weight: "bold")
  Table of Contents
]
#v(0.6em)
#outline(indent: 1.5em, depth: 2)
#pagebreak()

// ================================================================
// EXECUTIVE SUMMARY
// ================================================================
= Executive Summary

== Status Overview

#let n-cols = stats.by-state.len()
#grid(
  columns: if n-cols > 0 { (1fr,) * n-cols } else { (1fr,) },
  column-gutter: 10pt,
  ..stats.by-state.map(s => rect(
    width:  100%,
    fill:   state-col(s.state).lighten(88%),
    stroke: 0.5pt + state-col(s.state).lighten(50%),
    radius: 5pt,
    inset:  (x: 14pt, y: 12pt),
  )[
    #text(size: 26pt, weight: "bold", fill: state-col(s.state), str(s.count))
    #linebreak()
    #text(size: 9pt, fill: c-muted, s.state)
  ]),
)

#v(1.5em)

== Coverage by Category

#table(
  columns: (auto, 1fr, auto, auto, auto),
  fill: (_, row) => if row == 0 { c-navy } else if calc.odd(row) { white } else { luma(248) },
  stroke: 0.5pt + c-border,
  inset: (x: 12pt, y: 8pt),
  text(fill: white, weight: "semibold")[Category],
  text(fill: white, weight: "semibold")[Description],
  text(fill: white, weight: "semibold")[Total],
  text(fill: white, weight: "semibold")[Approved],
  text(fill: white, weight: "semibold")[Draft],
  ..stats.by-category.map(c => (
    text(weight: "semibold", font: "Courier New", c.name),
    text(c.description),
    str(c.total),
    text(fill: c-ok, weight: "semibold", str(c.approved)),
    text(fill: if c.draft > 0 { c-warn } else { c-muted }, str(c.draft)),
  )).flatten(),
)

// ================================================================
// REQUIREMENTS BY CATEGORY
// ================================================================
#for cat in categories [
  = #(cat.id + if cat.name != "" and cat.name != cat.id { " \u{2014} " + cat.name } else { "" })

  #if cat.description != "" [
    #text(fill: luma(60), style: "italic", cat.description)
    #v(0.8em)
  ]

  #let cat-reqs = requirements.filter(r => r.category == cat.id)

  #if cat-reqs.len() == 0 [
    #text(fill: c-muted, style: "italic")[No requirements in this category.]
    #v(0.5em)
  ] else [
    #for req in cat-reqs [
      #req-card(req)
    ]
  ]
]

// ================================================================
// APPENDIX -- TRACEABILITY MATRIX
// ================================================================
= Traceability Matrix

#table(
  columns: (auto, 2fr, auto, auto),
  fill: (_, row) => if row == 0 { c-navy } else if calc.odd(row) { white } else { luma(248) },
  stroke: 0.5pt + c-border,
  inset: (x: 10pt, y: 7pt),
  text(fill: white, weight: "semibold")[ID],
  text(fill: white, weight: "semibold")[Title],
  text(fill: white, weight: "semibold")[Category],
  text(fill: white, weight: "semibold")[Parent(s)],
  ..requirements.map(r => (
    text(font: "Courier New", size: 9pt, r.id),
    r.title,
    r.category,
    if r.parents.len() > 0 {
      text(font: "Courier New", size: 9pt, r.parents.join(", "))
    } else {
      text(fill: c-muted, "\u{2014}")
    },
  )).flatten(),
)

// ================================================================
// APPENDIX -- VERIFICATION SUMMARY
// ================================================================
= Verification Summary

#table(
  columns: (auto, 2fr, auto, auto, auto),
  fill: (_, row) => if row == 0 { c-navy } else if calc.odd(row) { white } else { luma(248) },
  stroke: 0.5pt + c-border,
  inset: (x: 10pt, y: 7pt),
  text(fill: white, weight: "semibold")[ID],
  text(fill: white, weight: "semibold")[Title],
  text(fill: white, weight: "semibold")[Method],
  text(fill: white, weight: "semibold")[Level],
  text(fill: white, weight: "semibold")[Activities],
  ..requirements.map(r => (
    text(font: "Courier New", size: 9pt, r.id),
    r.title,
    r.verification-method,
    r.verification-level,
    text(
      fill:   if r.activities.len() > 0 { c-ok } else { c-warn },
      weight: "semibold",
      str(r.activities.len()),
    ),
  )).flatten(),
)
"##;

// ----------------------------------------------------------------
// Public entry point
// ----------------------------------------------------------------

pub fn generate_typst_source(set: &RequirementSet<Validated>) -> String {
    let mut out = String::with_capacity(128 * 1024);
    write_meta(&mut out, set);
    write_stats(&mut out, set);
    write_categories(&mut out, set);
    write_requirements(&mut out, set);
    out.push_str(TEMPLATE);
    out
}

// ----------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------

fn ts(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
        .replace('\r', "");
    format!("\"{escaped}\"")
}

fn ts_bool(b: bool) -> &'static str {
    if b { "true" } else { "false" }
}

fn ts_id_array(ids: &[rqtk_core::RequirementId]) -> String {
    match ids.len() {
        0 => "()".to_string(),
        1 => format!("({},)", ts(&ids[0].to_string())),
        _ => {
            let inner: Vec<_> = ids.iter().map(|id| ts(&id.to_string())).collect();
            format!("({})", inner.join(", "))
        }
    }
}

// ----------------------------------------------------------------
// Data-section writers
// ----------------------------------------------------------------

fn write_meta(out: &mut String, set: &RequirementSet<Validated>) {
    let meta = &set.config.project;
    let org = meta
        .organization
        .as_ref()
        .and_then(|o| {
            o.program
                .as_deref()
                .or(o.responsible_engineer.as_deref())
                .or(o.center.as_deref())
        })
        .unwrap_or("");

    out.push_str("#let meta = (\n");
    out.push_str(&format!("  project: {},\n", ts(&meta.name)));
    out.push_str(&format!(
        "  description: {},\n",
        ts(meta.description.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!("  version: {},\n", ts(&meta.version.to_string())));
    out.push_str(&format!(
        "  date: {},\n",
        ts(&Local::now().format("%B %d, %Y").to_string())
    ));
    out.push_str(&format!(
        "  classification: {},\n",
        ts(meta.classification.as_deref().unwrap_or("Unclassified"))
    ));
    out.push_str(&format!(
        "  mission-phase: {},\n",
        ts(meta.mission_phase.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!("  organization: {},\n", ts(org)));
    out.push_str(")\n\n");
}

fn write_stats(out: &mut String, set: &RequirementSet<Validated>) {
    let mut by_state: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_cat: BTreeMap<String, [usize; 3]> = BTreeMap::new();

    for req_file in set.requirements.values() {
        let r = &req_file.requirement;
        *by_state.entry(r.status.state.clone()).or_default() += 1;
        let e = by_cat.entry(r.category.clone()).or_insert([0; 3]);
        e[0] += 1;
        if r.status.state == "Approved" {
            e[1] += 1;
        }
        if r.status.state == "Draft" {
            e[2] += 1;
        }
    }

    out.push_str("#let stats = (\n");
    out.push_str(&format!("  total: {},\n", set.requirements.len()));

    out.push_str("  by-state: (\n");
    for (state, count) in &by_state {
        out.push_str(&format!("    (state: {}, count: {}),\n", ts(state), count));
    }
    out.push_str("  ),\n");

    out.push_str("  by-category: (\n");
    for (cat_key, [total, approved, draft]) in &by_cat {
        let desc = set
            .config
            .categories
            .get(cat_key)
            .and_then(|c| c.description.as_deref())
            .unwrap_or("");
        out.push_str(&format!(
            "    (name: {}, description: {}, total: {}, approved: {}, draft: {}),\n",
            ts(cat_key),
            ts(desc),
            total,
            approved,
            draft
        ));
    }
    out.push_str("  ),\n");

    out.push_str(")\n\n");
}

fn write_categories(out: &mut String, set: &RequirementSet<Validated>) {
    let mut cats: Vec<_> = set.config.categories.iter().collect();
    cats.sort_by_key(|(key, c)| (c.level, key.as_str()));

    out.push_str("#let categories = (\n");
    for (key, cat) in &cats {
        out.push_str(&format!(
            "  (id: {}, name: {}, description: {}, level: {}),\n",
            ts(key),
            ts(&cat.name),
            ts(cat.description.as_deref().unwrap_or("")),
            cat.level,
        ));
    }
    out.push_str(")\n\n");
}

fn write_requirements(out: &mut String, set: &RequirementSet<Validated>) {
    out.push_str("#let requirements = (\n");

    for req_file in set.requirements.values() {
        let r = &req_file.requirement;

        let params_typst = format_parameters(r);
        let acts_typst = format_activities(r);
        let parents = ts_id_array(&r.traceability.parents);

        out.push_str("  (\n");
        out.push_str(&format!("    id: {},\n", ts(&r.id.to_string())));
        out.push_str(&format!("    title: {},\n", ts(&r.title)));
        out.push_str(&format!("    category: {},\n", ts(&r.category)));
        out.push_str(&format!("    req-type: {},\n", ts(&r.req_type)));
        out.push_str(&format!(
            "    hash: {},\n",
            ts(&r.content_hash.as_deref().unwrap_or("")
                [..8.min(r.content_hash.as_deref().unwrap_or("").len())])
        ));
        out.push_str(&format!("    state: {},\n", ts(&r.status.state)));
        out.push_str(&format!("    priority: {},\n", ts(&r.status.priority)));
        out.push_str(&format!(
            "    criticality: {},\n",
            ts(r.status.criticality.as_deref().unwrap_or(""))
        ));
        out.push_str(&format!("    text: {},\n", ts(&r.statement.text)));
        out.push_str(&format!(
            "    rationale: {},\n",
            ts(r.statement.rationale.as_deref().unwrap_or(""))
        ));
        out.push_str(&format!(
            "    notes: {},\n",
            ts(r.statement.notes.as_deref().unwrap_or(""))
        ));
        out.push_str(&format!("    parents: {},\n", parents));
        out.push_str(&format!(
            "    verification-method: {},\n",
            ts(&r.verification.method)
        ));
        out.push_str(&format!(
            "    verification-level: {},\n",
            ts(&r.verification.level)
        ));
        out.push_str(&format!("    tbd: {},\n", ts_bool(r.status.tbd)));
        out.push_str(&format!("    tbr: {},\n", ts_bool(r.status.tbr)));
        out.push_str(&format!("    parameters: {},\n", params_typst));
        out.push_str(&format!("    activities: {},\n", acts_typst));
        out.push_str("  ),\n");
    }

    out.push_str(")\n\n");
}

fn format_parameters(r: &rqtk_core::RequirementBody) -> String {
    if r.parameters.is_empty() {
        return "()".to_string();
    }
    let mut s = String::from("(\n");
    for p in &r.parameters {
        s.push_str(&format!(
            "      (name: {}, operator: {}, value: {}, unit: {}, tolerance: {}),\n",
            ts(&p.name),
            ts(&p.operator),
            ts(&p.value.to_string()),
            ts(p.unit.as_deref().unwrap_or("")),
            ts(p.tolerance.map(|t| t.to_string()).as_deref().unwrap_or("")),
        ));
    }
    s.push_str("    )");
    s
}

fn format_activities(r: &rqtk_core::RequirementBody) -> String {
    if r.verification.activities.is_empty() {
        return "()".to_string();
    }
    let mut s = String::from("(\n");
    for act in &r.verification.activities {
        s.push_str(&format!(
            "      (id: {}, name: {}, status: {}, expected-result: {}),\n",
            ts(&act.id),
            ts(&act.name),
            ts(act.status.as_deref().unwrap_or("Planned")),
            ts(act.expected_result.as_deref().unwrap_or("")),
        ));
    }
    s.push_str("    )");
    s
}

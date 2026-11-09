//! OpenAPI 3.0 and 3.1 documents, parsed as generic JSON values.
//!
//! An `openapi` spec entry is hashed as the canonical form of its element (or of the whole
//! document) together with every node that element references through `$ref`,
//! transitively and across files. An operation also carries what governs it from outside
//! its node (see [`closure`]). References to URLs or to files outside the root are
//! refused and never fetched. Parsing is bounded: the YAML parser rejects alias expansion
//! beyond a fixed budget, and both parsers reject nesting deeper than 128 levels.

use crate::spec::{SpecFiles, SpecProblem, codes, normalize};
use percent_encoding::percent_decode_str;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

/// Every file and node an OpenAPI entry reaches, keyed `file#pointer`.
#[derive(Debug, Default)]
pub(crate) struct Closure {
    pub files: BTreeSet<String>,
    /// Canonical JSON of each reached node.
    pub nodes: BTreeMap<String, String>,
}

/// Tag hashed with every OpenAPI digest: change it whenever the canonical form or what a
/// closure includes changes, so the change is deliberate.
pub(crate) const CANONICAL_FORM: &str = "openapi-closure/1";

const METHODS: [&str; 8] = [
    "get", "put", "post", "delete", "options", "head", "patch", "trace",
];

/// Follow the entry at `file`, `element` from the root, adding problems for entry `entry`.
///
/// When `element` is an operation (`#/paths/<path>/<method>`), the closure also holds what
/// governs that operation from outside it: the path item's `parameters` and `servers`, the
/// document's `servers` and `security`, and the security schemes either `security` names.
pub(crate) fn closure(
    files: &dyn SpecFiles,
    file: &str,
    element: Option<&str>,
    entry: usize,
    problems: &mut Vec<SpecProblem>,
) -> Closure {
    let mut docs = Documents {
        files,
        cache: HashMap::new(),
    };
    let mut out = Closure::default();
    let Some(doc) = docs.load(file, entry, problems) else {
        return out;
    };
    if let Err(message) = check_version(doc) {
        problems.push(SpecProblem::new(
            codes::UNPARSEABLE,
            entry,
            format!("`{file}`: {message}"),
        ));
        return out;
    }
    let pointer = match element {
        None => String::new(),
        Some(e) => match fragment_pointer(e) {
            Some(p) => p,
            None => {
                problems.push(SpecProblem::new(
                    codes::UNRESOLVED,
                    entry,
                    format!("element `{e}` is not a JSON pointer such as `#/paths/~1users/get`"),
                ));
                return out;
            }
        },
    };
    if doc.pointer(&pointer).is_none() {
        problems.push(SpecProblem::new(
            codes::UNRESOLVED,
            entry,
            format!(
                "element `{}` does not resolve in `{file}`",
                element.unwrap_or_default()
            ),
        ));
        return out;
    }

    let mut queue = operation_context(doc, &pointer)
        .into_iter()
        .map(|p| (file.to_owned(), p))
        .collect::<Vec<_>>();
    queue.push((file.to_owned(), pointer));
    let mut seen = BTreeSet::new();
    while let Some((file, pointer)) = queue.pop() {
        if !seen.insert((file.clone(), pointer.clone())) {
            continue;
        }
        let Some(doc) = docs.load(&file, entry, problems) else {
            continue;
        };
        out.files.insert(file.clone());
        let Some(node) = doc.pointer(&pointer) else {
            continue;
        };
        let mut refs = Vec::new();
        collect_refs(node, &mut refs);
        out.nodes
            .insert(format!("{file}#{pointer}"), canonical(node));

        for reference in refs {
            let (target_file, fragment) = reference.split_once('#').unwrap_or((&reference, ""));
            let target_file = if target_file.is_empty() {
                file.clone()
            } else {
                let dir = file.rsplit_once('/').map_or("", |(d, _)| d);
                match normalize(dir, target_file) {
                    Some(f) => f,
                    None => {
                        problems.push(SpecProblem::new(
                            codes::ESCAPES,
                            entry,
                            format!(
                                "`$ref: {reference}` in `{file}` points to a URL or outside the repository; rqtk does not follow it"
                            ),
                        ));
                        continue;
                    }
                }
            };
            let Some(target_pointer) = fragment_pointer(&format!("#{fragment}")) else {
                problems.push(SpecProblem::new(
                    codes::UNRESOLVED,
                    entry,
                    format!("`$ref: {reference}` in `{file}` is not a JSON pointer"),
                ));
                continue;
            };
            let Some(target_doc) = docs.load(&target_file, entry, problems) else {
                continue;
            };
            if target_doc.pointer(&target_pointer).is_none() {
                problems.push(SpecProblem::new(
                    codes::UNRESOLVED,
                    entry,
                    format!("`$ref: {reference}` in `{file}` does not resolve"),
                ));
                continue;
            }
            queue.push((target_file, target_pointer));
        }
    }
    out
}

/// Parsed documents by root-relative path; a document that failed to load is `None`, so
/// its problem is reported once.
struct Documents<'a> {
    files: &'a dyn SpecFiles,
    cache: HashMap<String, Option<Value>>,
}

impl Documents<'_> {
    fn load(
        &mut self,
        file: &str,
        entry: usize,
        problems: &mut Vec<SpecProblem>,
    ) -> Option<&Value> {
        if !self.cache.contains_key(file) {
            let parsed = match self.files.read(file) {
                Ok(None) => {
                    problems.push(SpecProblem::no_match(entry, file));
                    None
                }
                Err(e) => {
                    problems.push(SpecProblem::new(
                        codes::NO_MATCH,
                        entry,
                        format!("cannot read spec file `{file}`: {e}"),
                    ));
                    None
                }
                Ok(Some(bytes)) => match parse(&bytes) {
                    Ok(value) => Some(value),
                    Err(message) => {
                        problems.push(SpecProblem::new(
                            codes::UNPARSEABLE,
                            entry,
                            format!("cannot parse `{file}`: {message}"),
                        ));
                        None
                    }
                },
            };
            self.cache.insert(file.to_owned(), parsed);
        }
        self.cache[file].as_ref()
    }
}

/// Parse `bytes` as JSON when they start with `{` or `[`, as YAML otherwise; the format
/// is decided by content, so a YAML and a JSON rendering of a document hash the same.
pub(crate) fn parse(bytes: &[u8]) -> Result<Value, String> {
    let first = bytes.iter().find(|b| !b.is_ascii_whitespace());
    if matches!(first, Some(b'{' | b'[')) {
        serde_json::from_slice(bytes).map_err(|e| e.to_string())
    } else {
        yaml_serde::from_slice(bytes).map_err(|e| e.to_string())
    }
}

fn check_version(doc: &Value) -> Result<(), String> {
    if let Some(v) = doc.get("swagger") {
        return Err(format!(
            "Swagger {} is not supported; rqtk parses OpenAPI 3.0 and 3.1",
            v.as_str().unwrap_or("2.0")
        ));
    }
    match doc.get("openapi").and_then(Value::as_str) {
        Some(v) if v.starts_with("3.0") || v.starts_with("3.1") => Ok(()),
        Some(v) => Err(format!(
            "OpenAPI {v} is not supported; rqtk parses OpenAPI 3.0 and 3.1"
        )),
        None => Err("not an OpenAPI document: no `openapi` version field".to_owned()),
    }
}

/// Pointers, in the same document, to what governs operation `pointer` from outside it.
/// Empty when `pointer` is not an operation.
fn operation_context(doc: &Value, pointer: &str) -> Vec<String> {
    let Some((path_item, method)) = pointer.rsplit_once('/') else {
        return Vec::new();
    };
    let is_operation = METHODS.contains(&method)
        && path_item.starts_with("/paths/")
        && path_item.matches('/').count() == 2;
    if !is_operation {
        return Vec::new();
    }
    let mut out: Vec<String> = [
        format!("{path_item}/parameters"),
        format!("{path_item}/servers"),
        "/servers".to_owned(),
        "/security".to_owned(),
    ]
    .into_iter()
    .filter(|p| doc.pointer(p).is_some())
    .collect();
    let mut schemes = BTreeSet::new();
    for security in [
        doc.pointer("/security"),
        doc.pointer(&format!("{pointer}/security")),
    ] {
        for requirement in security.and_then(Value::as_array).into_iter().flatten() {
            if let Some(names) = requirement.as_object() {
                schemes.extend(names.keys().cloned());
            }
        }
    }
    for name in schemes {
        let scheme = format!(
            "/components/securitySchemes/{}",
            name.replace('~', "~0").replace('/', "~1")
        );
        if doc.pointer(&scheme).is_some() {
            out.push(scheme);
        }
    }
    out
}

/// `#/a/b` (percent-decoded) → `/a/b`; `#` or `` → the whole document. `None` for anything
/// that is not a fragment holding a JSON pointer.
fn fragment_pointer(fragment: &str) -> Option<String> {
    let encoded = fragment.strip_prefix('#').unwrap_or(fragment);
    let pointer = percent_decode_str(encoded).decode_utf8().ok()?;
    (pointer.is_empty() || pointer.starts_with('/')).then(|| pointer.into_owned())
}

fn collect_refs(node: &Value, out: &mut Vec<String>) {
    match node {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref") {
                out.push(r.clone());
            }
            for (key, value) in map {
                if key != "$ref" {
                    collect_refs(value, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_refs(v, out)),
        _ => {}
    }
}

/// JSON with object keys sorted and no whitespace: equal for documents that differ only in
/// formatting, comments, key order, anchors or the choice of YAML or JSON.
pub(crate) fn canonical(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                let _ = write!(out, "{}", Value::String(key.clone()));
                out.push(':');
                write_canonical(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        scalar => {
            let _ = write!(out, "{scalar}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Interface, SpecEntry};
    use crate::spec::{DirFiles, SpecResolution};
    use std::path::Path;

    fn resolve(root: &Path, interface: &Interface) -> SpecResolution {
        crate::spec::resolve(&DirFiles::working_tree(root), interface)
    }

    const API: &str = r##"openapi: 3.0.3
info: {title: Users, version: "1"}
paths:
  /users/{id}:
    get:
      responses:
        "200":
          description: A user
          content:
            application/json:
              schema: {$ref: "#/components/schemas/User"}
  /health:
    get:
      responses:
        200: {description: ok}
components:
  schemas:
    User:
      type: object
      properties:
        id: {type: string}
        friend: {$ref: "#/components/schemas/User"}
"##;

    fn spec(path: &str, element: Option<&str>) -> Interface {
        Interface {
            specs: vec![SpecEntry {
                format: "openapi".to_owned(),
                path: path.to_owned(),
                element: element.map(str::to_owned),
            }],
        }
    }

    fn write(dir: &Path, file: &str, text: &str) {
        let path = dir.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    const GET_USER: &str = "#/paths/~1users~1{id}/get";

    // rqtk: verifies VA-CORE-0023-01
    #[test]
    fn parses_openapi_3_0_and_3_1_in_yaml_and_json() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.yaml", API);
        let json = serde_json::to_string(&parse(API.as_bytes()).unwrap()).unwrap();
        write(dir.path(), "a.json", &json);
        write(dir.path(), "b.yaml", &API.replace("3.0.3", "3.1.0"));
        for file in ["a.yaml", "a.json", "b.yaml"] {
            let r = resolve(dir.path(), &spec(file, None));
            assert!(r.problems.is_empty(), "{file}: {:?}", r.problems);
        }
    }

    // rqtk: verifies VA-CORE-0023-02
    #[test]
    fn unparseable_documents_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "broken.yaml",
            "openapi: 3.0.0\npaths: [unclosed\n",
        );
        write(dir.path(), "swagger.yaml", "swagger: \"2.0\"\npaths: {}\n");
        for file in ["broken.yaml", "swagger.yaml"] {
            let r = resolve(dir.path(), &spec(file, None));
            assert_eq!(r.problems.len(), 1, "{file}: {:?}", r.problems);
            assert_eq!(r.problems[0].code, "RQ035");
            assert_eq!(r.problems[0].entry, Some(0));
        }
    }

    // rqtk: verifies VA-CORE-0024-01
    #[test]
    fn relative_file_refs_are_followed_transitively() {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "api/openapi.yaml",
            "openapi: 3.1.0\ninfo: {title: t, version: '1'}\npaths:\n  /a:\n    get:\n      responses:\n        '200': {$ref: '../shared/responses.yaml#/Ok'}\n",
        );
        write(
            dir.path(),
            "shared/responses.yaml",
            "Ok:\n  description: ok\n  content:\n    application/json:\n      schema: {$ref: 'schemas.yaml#/Thing'}\n",
        );
        write(
            dir.path(),
            "shared/schemas.yaml",
            "Thing:\n  type: object\n  properties:\n    child: {$ref: '#/Thing'}\n",
        );
        let r = resolve(
            dir.path(),
            &spec("api/openapi.yaml", Some("#/paths/~1a/get")),
        );
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        assert_eq!(
            r.files.iter().map(String::as_str).collect::<Vec<_>>(),
            [
                "api/openapi.yaml",
                "shared/responses.yaml",
                "shared/schemas.yaml"
            ]
        );
    }

    // rqtk: verifies VA-CORE-0024-02
    #[test]
    fn reference_cycles_terminate() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.yaml", API);
        write(
            dir.path(),
            "loop.yaml",
            "openapi: 3.0.0\ninfo: {title: t, version: '1'}\npaths: {}\ncomponents:\n  schemas:\n    A: {$ref: '#/components/schemas/B'}\n    B: {$ref: '#/components/schemas/A'}\n",
        );
        assert!(
            resolve(dir.path(), &spec("a.yaml", Some(GET_USER)))
                .problems
                .is_empty()
        );
        assert!(
            resolve(dir.path(), &spec("loop.yaml", None))
                .problems
                .is_empty()
        );
    }

    // rqtk: verifies VA-CORE-0025-01
    #[test]
    fn absolute_url_refs_are_rejected_unfetched() {
        let dir = tempfile::tempdir().unwrap();
        for (i, target) in ["https://example.invalid/x.yaml#/X", "file:///etc/passwd"]
            .iter()
            .enumerate()
        {
            let file = format!("u{i}.yaml");
            write(
                dir.path(),
                &file,
                &format!(
                    "openapi: 3.0.0\ninfo: {{title: t, version: '1'}}\npaths:\n  /a: {{$ref: '{target}'}}\n"
                ),
            );
            let r = resolve(dir.path(), &spec(&file, None));
            assert_eq!(r.problems.len(), 1, "{:?}", r.problems);
            assert_eq!(r.problems[0].code, "RQ036");
        }
    }

    // rqtk: verifies VA-CORE-0025-02
    #[test]
    fn escaping_relative_refs_are_rejected() {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("repo");
        write(outer.path(), "secret.yaml", "X: {type: string}\n");
        write(
            &root,
            "api.yaml",
            "openapi: 3.0.0\ninfo: {title: t, version: '1'}\npaths:\n  /a: {$ref: '../secret.yaml#/X'}\n",
        );
        let r = resolve(&root, &spec("api.yaml", None));
        assert_eq!(r.problems.len(), 1, "{:?}", r.problems);
        assert_eq!(r.problems[0].code, "RQ036");
        assert!(!r.files.contains("../secret.yaml"));
    }

    // rqtk: verifies VA-CORE-0026-01
    #[test]
    fn unresolvable_element_pointer_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.yaml", API);
        assert!(
            resolve(dir.path(), &spec("a.yaml", Some(GET_USER)))
                .problems
                .is_empty()
        );
        for element in ["#/paths/~1user~1{id}/get", "#/paths/~1users~1{id}/post"] {
            let r = resolve(dir.path(), &spec("a.yaml", Some(element)));
            assert_eq!(r.problems.len(), 1, "{element}: {:?}", r.problems);
            assert_eq!(r.problems[0].code, "RQ037");
        }
    }

    #[test]
    fn element_pointers_are_percent_decoded() {
        assert_eq!(
            fragment_pointer("#/paths/~1users~1%7Bid%7D/get").as_deref(),
            Some("/paths/~1users~1{id}/get")
        );
        assert_eq!(fragment_pointer("#").as_deref(), Some(""));
        assert_eq!(fragment_pointer("#paths"), None);
        assert_eq!(fragment_pointer("#/%FF"), None, "not UTF-8");
    }

    // rqtk: verifies VA-CORE-0027-01
    #[test]
    fn changes_to_the_element_or_its_references_change_the_hash() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.yaml", API);
        let before = resolve(dir.path(), &spec("a.yaml", Some(GET_USER))).digest;
        write(dir.path(), "a.yaml", &API.replace("A user", "The user"));
        let operation_edit = resolve(dir.path(), &spec("a.yaml", Some(GET_USER))).digest;
        assert_ne!(operation_edit, before);
        write(
            dir.path(),
            "a.yaml",
            &API.replace("id: {type: string}", "id: {type: integer}"),
        );
        let schema_edit = resolve(dir.path(), &spec("a.yaml", Some(GET_USER))).digest;
        assert_ne!(schema_edit, before);
        assert_ne!(schema_edit, operation_edit);
    }

    // rqtk: verifies VA-CORE-0027-03
    #[test]
    fn what_governs_an_operation_from_outside_it_changes_the_hash() {
        const SECURED: &str = r##"openapi: 3.0.3
info: {title: Users, version: "1"}
servers: [{url: "https://api.example.com"}]
security: [{bearer: []}]
paths:
  /users/{id}:
    parameters:
      - {name: id, in: path, required: true, schema: {type: string}}
    get:
      security: [{apiKey: []}]
      responses:
        "200": {description: A user}
components:
  securitySchemes:
    bearer: {type: http, scheme: bearer}
    apiKey: {type: apiKey, in: header, name: X-Key}
    unused: {type: http, scheme: basic}
"##;
        let dir = tempfile::tempdir().unwrap();
        let digest = |text: &str| {
            write(dir.path(), "a.yaml", text);
            let r = resolve(dir.path(), &spec("a.yaml", Some(GET_USER)));
            assert!(r.problems.is_empty(), "{:?}", r.problems);
            r.digest
        };
        let before = digest(SECURED);
        for (from, to) in [
            ("schema: {type: string}}", "schema: {type: integer}}"),
            ("https://api.example.com", "https://api.example.org"),
            ("security: [{bearer: []}]", "security: []"),
            ("scheme: bearer}", "scheme: digest}"),
            ("name: X-Key}", "name: X-Api-Key}"),
        ] {
            assert_ne!(digest(&SECURED.replace(from, to)), before, "{from} → {to}");
        }
        assert_eq!(
            digest(&SECURED.replace("scheme: basic}", "scheme: negotiate}")),
            before,
            "a scheme nothing names is not part of the operation"
        );
    }

    // rqtk: verifies VA-CORE-0027-02
    #[test]
    fn unrelated_changes_leave_the_hash_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.yaml", API);
        let before = resolve(dir.path(), &spec("a.yaml", Some(GET_USER))).digest;
        write(
            dir.path(),
            "a.yaml",
            &API.replace("{description: ok}", "{description: healthy}"),
        );
        assert_eq!(
            resolve(dir.path(), &spec("a.yaml", Some(GET_USER))).digest,
            before
        );
        // The whole-document entry does see it.
        let whole = |d: &Path| resolve(d, &spec("a.yaml", None)).digest;
        let changed = whole(dir.path());
        write(dir.path(), "a.yaml", API);
        assert_ne!(whole(dir.path()), changed);
    }

    // rqtk: verifies VA-CORE-0028-01
    #[test]
    fn equivalent_yaml_and_json_documents_hash_identically() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = r#"# A comment
openapi: 3.0.0
info: &info {title: t, version: "1"}
x-copy: *info
paths:
  /a:
    get:
      summary: hi
      responses: {"200": {description: ok}}
"#;
        let json = r#"{"paths":{"/a":{"get":{"responses":{"200":{"description":"ok"}},"summary":"hi"}}},
            "x-copy":{"version":"1","title":"t"},"openapi":"3.0.0","info":{"version":"1","title":"t"}}"#;
        let yaml_dir = tempfile::tempdir().unwrap();
        write(yaml_dir.path(), "api/openapi", yaml);
        write(dir.path(), "api/openapi", json);
        let digest = |d: &Path| {
            let r = resolve(d, &spec("api/openapi", Some("#/paths/~1a")));
            assert!(r.problems.is_empty(), "{:?}", r.problems);
            r.digest
        };
        assert_eq!(digest(yaml_dir.path()), digest(dir.path()));
        let whole = |d: &Path| resolve(d, &spec("api/openapi", None)).digest;
        assert_eq!(whole(yaml_dir.path()), whole(dir.path()));
    }

    // rqtk: verifies VA-CORE-0029-01
    #[test]
    fn alias_bombs_are_rejected() {
        let mut yaml = String::from("openapi: 3.0.0\na0: &a0 [x, x, x, x, x, x, x, x, x, x]\n");
        for i in 1..12 {
            let p = i - 1;
            yaml.push_str(&format!(
                "a{i}: &a{i} [*a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}, *a{p}]\n"
            ));
        }
        let started = std::time::Instant::now();
        let err = parse(yaml.as_bytes()).unwrap_err();
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "{err}"
        );
    }

    // rqtk: verifies VA-CORE-0029-02
    #[test]
    fn excessive_nesting_is_rejected() {
        let depth = 10_000;
        let yaml = format!(
            "openapi: 3.0.0\nx: {}{}\n",
            "[".repeat(depth),
            "]".repeat(depth)
        );
        assert!(parse(yaml.as_bytes()).is_err());
        let json = format!("{{\"x\": {}{}}}", "[".repeat(depth), "]".repeat(depth));
        assert!(parse(json.as_bytes()).is_err());
    }
}

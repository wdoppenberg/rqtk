//! Every JSON document rqtk prints, and the JSON Schema it is published with.

use rqtk_client::schemas::{document_schema, type_schemas, view_schemas};
use rqtk_core::{Need, Requirement, Stakeholder, VerificationActivity};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::cli::commands;

/// The error document printed on stderr with `--json`.
#[derive(Serialize, JsonSchema)]
pub struct ErrorDoc<'a> {
    pub schema: String,
    pub error: ErrorBody<'a>,
}

#[derive(Serialize, JsonSchema)]
pub struct ErrorBody<'a> {
    /// `usage` (exit code 2) or `error` (exit code 3).
    pub kind: &'a str,
    pub message: &'a str,
    /// Set when opening or querying the repository failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<rqtk_client::ErrorCode>,
}

/// `rqtk schema --json`: everything `rqtk schema <kind>` can print.
#[derive(Serialize, JsonSchema)]
pub struct SchemaKinds {
    pub kinds: Vec<SchemaKind>,
}

#[derive(Serialize, JsonSchema)]
pub struct SchemaKind {
    pub kind: String,
    pub category: Category,
    /// The file, command or client type the schema describes.
    pub describes: String,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    /// A file under `.rqtk/`.
    File,
    /// A `--json` document a command prints.
    Document,
    /// A type the client libraries return or accept.
    Client,
}

/// The `--json` documents printed by commands other than the client's views, with the
/// command that prints each.
fn command_documents() -> Vec<(&'static str, &'static str, Value)> {
    use commands::*;
    vec![
        (
            "search",
            "rqtk search --json",
            document_schema::<search::Report<'static>>("search"),
        ),
        (
            "scan",
            "rqtk scan --json",
            document_schema::<scan::Report<'static>>("scan"),
        ),
        (
            "verify",
            "rqtk verify --json",
            document_schema::<verify::Report<'static>>("verify"),
        ),
        (
            "review",
            "rqtk review --json",
            document_schema::<review::Report<'static>>("review"),
        ),
        (
            "rehash",
            "rqtk rehash --json",
            document_schema::<rehash::Report>("rehash"),
        ),
        (
            "baseline",
            "rqtk baseline <version> --json",
            document_schema::<baseline::Report>("baseline"),
        ),
        (
            "export",
            "rqtk export --json",
            document_schema::<export::Report<'static>>("export"),
        ),
        (
            "add-requirement",
            "rqtk add --json",
            document_schema::<add::Created<'static, Requirement>>("add-requirement"),
        ),
        (
            "add-need",
            "rqtk add-need --json",
            document_schema::<add::Created<'static, Need>>("add-need"),
        ),
        (
            "add-stakeholder",
            "rqtk add-stakeholder --json",
            document_schema::<add::Created<'static, Stakeholder>>("add-stakeholder"),
        ),
        (
            "add-activity",
            "rqtk add-activity --json",
            document_schema::<add::Created<'static, VerificationActivity>>("add-activity"),
        ),
        (
            "init",
            "rqtk init --json",
            document_schema::<init::Report>("init"),
        ),
        (
            "init-agents",
            "rqtk init --agents --json",
            document_schema::<init::WithSkills<'static>>("init-agents"),
        ),
        (
            "install-hook",
            "rqtk install-hook --json",
            document_schema::<install_hook::Report<'static>>("install-hook"),
        ),
        (
            "skills",
            "rqtk skills list --json",
            document_schema::<skills::SkillList<'static>>("skills"),
        ),
        (
            "skills-install",
            "rqtk skills install --json",
            document_schema::<skills::Report>("skills-install"),
        ),
        (
            "interfaces-add",
            "rqtk interfaces add --json",
            document_schema::<interfaces::AddReport<'static>>("interfaces-add"),
        ),
        (
            "interfaces-sync",
            "rqtk interfaces sync --json",
            document_schema::<interfaces::SyncReport<'static>>("interfaces-sync"),
        ),
        (
            "interfaces-remove",
            "rqtk interfaces remove --json",
            document_schema::<interfaces::RemoveReport<'static>>("interfaces-remove"),
        ),
        (
            "interfaces-list",
            "rqtk interfaces list --json",
            document_schema::<interfaces::ListReport<'static>>("interfaces-list"),
        ),
        (
            "explain",
            "rqtk explain --json",
            document_schema::<explain::Rules<'static>>("explain"),
        ),
        (
            "schema-kinds",
            "rqtk schema --json",
            document_schema::<SchemaKinds>("schema-kinds"),
        ),
        (
            "error",
            "an error from any command, on stderr with --json",
            document_schema::<ErrorDoc<'static>>("error"),
        ),
    ]
}

/// The command that prints each client view.
fn view_command(name: &str) -> &'static str {
    match name {
        "context" => "rqtk context",
        "trace" => "rqtk trace",
        "coverage" => "rqtk coverage",
        "lint" => "rqtk lint",
        "impact" => "rqtk impact",
        "diff" => "rqtk diff",
        "baselines" => "rqtk baseline --list",
        "history" => "rqtk log",
        _ => "",
    }
}

/// Every schema `rqtk schema` knows: files, documents and client types.
pub fn all() -> Vec<(SchemaKind, Value)> {
    let mut out: Vec<(SchemaKind, Value)> = rqtk_core::schema::KINDS
        .iter()
        .map(|(kind, file)| {
            (
                SchemaKind {
                    kind: (*kind).to_owned(),
                    category: Category::File,
                    describes: (*file).to_owned(),
                },
                rqtk_core::schema::json_schema(kind).expect("every listed kind has a schema"),
            )
        })
        .collect();
    out.extend(view_schemas().into_iter().map(|(name, schema)| {
        (
            SchemaKind {
                kind: name.to_owned(),
                category: Category::Document,
                describes: format!("{} --json", view_command(name)),
            },
            schema,
        )
    }));
    out.extend(
        command_documents()
            .into_iter()
            .map(|(name, command, schema)| {
                (
                    SchemaKind {
                        kind: name.to_owned(),
                        category: Category::Document,
                        describes: command.to_owned(),
                    },
                    schema,
                )
            }),
    );
    out.extend(type_schemas().into_iter().map(|(name, schema)| {
        (
            SchemaKind {
                kind: name.to_owned(),
                category: Category::Client,
                describes: "a client library type".to_owned(),
            },
            schema,
        )
    }));
    out
}

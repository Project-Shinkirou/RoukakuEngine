use std::{collections::BTreeMap, sync::LazyLock};

use jsonc_parser::{ast, common::Ranged};
use roukaku_core::{Diagnostic, SourceRef};
use serde_json::Value;

use crate::VnError;

pub const UI_SCREEN_SCHEMA: &str = include_str!("../../../../../../Schemas/ui-screen.v1.json");
pub const UI_STYLE_SCHEMA: &str = include_str!("../../../../../../Schemas/ui-style.v1.json");

static SCREEN: LazyLock<jsonschema::Validator> = LazyLock::new(|| validator(UI_SCREEN_SCHEMA));
static STYLE: LazyLock<jsonschema::Validator> = LazyLock::new(|| validator(UI_STYLE_SCHEMA));

fn validator(schema: &str) -> jsonschema::Validator {
    jsonschema::validator_for(&serde_json::from_str(schema).expect("published UI schema is JSON"))
        .expect("published UI schema is valid")
}

/// Semantic JSON plus original token locations. Trivia never reaches the blueprint.
pub(super) struct Document<'a> {
    pub path: &'a str,
    pub text: &'a str,
    pub value: Value,
    spans: BTreeMap<String, std::ops::Range<usize>>,
}

impl<'a> Document<'a> {
    pub fn parse(path: &'a str, text: &'a str, style: bool) -> Result<Self, VnError> {
        let options = jsonc_parser::ParseOptions {
            allow_comments: true,
            allow_trailing_commas: true,
            allow_loose_object_property_names: false,
            allow_missing_commas: false,
            allow_single_quoted_strings: false,
            allow_hexadecimal_numbers: false,
            allow_unary_plus_numbers: false,
            allow_bare_decimal_point_numbers: false,
            allow_non_finite_numbers: false,
            allow_extended_string_escapes: false,
        };
        let parsed =
            jsonc_parser::parse_to_ast(text, &Default::default(), &options).map_err(|error| {
                located(
                    path,
                    text,
                    error.range().start..error.range().end,
                    "ROUKAKU_UI_JSONC_SYNTAX",
                    &error.to_string(),
                )
            })?;
        let ast = parsed.value.ok_or_else(|| {
            located(
                path,
                text,
                0..0,
                "ROUKAKU_UI_JSONC_EMPTY",
                "UI document must contain an object",
            )
        })?;
        let mut spans = BTreeMap::new();
        collect(path, text, &ast, "", &mut spans)?;
        let doc = Self {
            path,
            text,
            value: ast.into(),
            spans,
        };
        let schema = if style { &*STYLE } else { &*SCREEN };
        if let Err(error) = schema.validate(&doc.value) {
            return Err(doc.error(
                &error.instance_path().to_string(),
                "ROUKAKU_UI_SCHEMA",
                &error.to_string(),
            ));
        }
        Ok(doc)
    }

    pub fn source(&self, pointer: &str) -> SourceRef {
        let mut key = pointer;
        let range = loop {
            if let Some(range) = self.spans.get(key) {
                break range.clone();
            }
            key = key.rsplit_once('/').map_or("", |(parent, _)| parent);
        };
        source_ref(self.path, self.text, range)
    }

    pub fn error(&self, pointer: &str, code: &str, message: &str) -> VnError {
        VnError::Diagnostic(Diagnostic::blocking(code, message).with_source(self.source(pointer)))
    }

    pub fn attach(&self, pointer: &str, error: VnError) -> VnError {
        match error {
            VnError::Diagnostic(mut diagnostic) => {
                if diagnostic.source.is_none() {
                    diagnostic.source = Some(self.source(pointer));
                }
                VnError::Diagnostic(diagnostic)
            }
            other => other,
        }
    }
}

pub(super) fn pointer(parent: &str, key: &str) -> String {
    format!("{parent}/{}", key.replace('~', "~0").replace('/', "~1"))
}

fn collect(
    path: &str,
    text: &str,
    value: &ast::Value<'_>,
    key: &str,
    spans: &mut BTreeMap<String, std::ops::Range<usize>>,
) -> Result<(), VnError> {
    let range = value.range();
    spans.insert(key.to_string(), range.start..range.end);
    match value {
        ast::Value::Object(object) => {
            let mut names = std::collections::BTreeSet::new();
            for prop in &object.properties {
                let name = prop.name.as_str();
                if !names.insert(name) {
                    let range = prop.name.range();
                    return Err(located(
                        path,
                        text,
                        range.start..range.end,
                        "ROUKAKU_UI_JSONC_DUPLICATE_KEY",
                        "duplicate JSONC property",
                    ));
                }
                collect(path, text, &prop.value, &pointer(key, name), spans)?;
            }
        }
        ast::Value::Array(array) => {
            for (index, value) in array.elements.iter().enumerate() {
                collect(path, text, value, &pointer(key, &index.to_string()), spans)?;
            }
        }
        ast::Value::NumberLit(number) if serde_json::from_str::<Value>(number.value).is_err() => {
            let range = number.range;
            return Err(located(
                path,
                text,
                range.start..range.end,
                "ROUKAKU_UI_JSONC_NUMBER",
                "JSONC number is outside the supported finite range",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn located(
    path: &str,
    text: &str,
    range: std::ops::Range<usize>,
    code: &str,
    message: &str,
) -> VnError {
    VnError::Diagnostic(
        Diagnostic::blocking(code, message).with_source(source_ref(path, text, range)),
    )
}

fn source_ref(path: &str, text: &str, range: std::ops::Range<usize>) -> SourceRef {
    let start = range.start.min(text.len());
    let prefix = &text[..start];
    SourceRef {
        source: path.to_string(),
        line: prefix.bytes().filter(|&byte| byte == b'\n').count() as u32 + 1,
        column: prefix
            .rsplit('\n')
            .next()
            .unwrap_or_default()
            .chars()
            .count() as u32
            + 1,
        length: range.end.saturating_sub(start) as u32,
    }
}

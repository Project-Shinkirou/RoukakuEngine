mod ast;
mod lexer;

pub use ast::*;
pub use lexer::*;

use roukaku_core::{Diagnostic, SourceRef};

use crate::{CommandRegistry, RoukakuSource};

#[derive(Debug, Clone)]
pub struct ParsedRoukakuSource {
    pub path: String,
    pub cst: SyntaxNode,
    pub ast: RoukakuAst,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn parse_roukaku_source(path: impl Into<String>, text: &str) -> ParsedRoukakuSource {
    let path = path.into();
    let (cst, lexical_diagnostics) = build_lossless_cst(&path, text);
    let (ast, mut diagnostics) = parse_typed_ast(&path, &cst);
    diagnostics.extend(lexical_diagnostics);
    let registry = CommandRegistry::default();
    for command in ast.commands() {
        if !registry.is_known(command.keyword()) {
            diagnostics.push(
                Diagnostic::warning(
                    "ROUKAKU_VN_UNKNOWN_COMMAND",
                    "command is preserved for editing but requires an explicit provider binding",
                )
                .with_source(SourceRef {
                    source: path.clone(),
                    line: command.line() as u32,
                    column: command.column() as u32,
                    length: command.keyword().len() as u32,
                })
                .with_field("command", command.keyword()),
            );
        }
    }
    diagnostics.sort_by(|left, right| {
        left.source
            .as_ref()
            .map(|source| (source.line, source.column))
            .cmp(
                &right
                    .source
                    .as_ref()
                    .map(|source| (source.line, source.column)),
            )
            .then_with(|| left.code.cmp(&right.code))
    });
    ParsedRoukakuSource {
        path,
        cst,
        ast,
        diagnostics,
    }
}

pub fn parse_roukaku_sources<I, S>(sources: I) -> Vec<ParsedRoukakuSource>
where
    I: IntoIterator<Item = S>,
    S: Into<RoukakuSource>,
{
    sources
        .into_iter()
        .map(Into::into)
        .map(|source: RoukakuSource| parse_roukaku_source(source.path, &source.text))
        .collect()
}

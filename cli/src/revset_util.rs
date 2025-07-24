// Copyright 2022-2024 The Jujutsu Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Utility for parsing and evaluating user-provided revset expressions.

use std::io;
use std::rc::Rc;
use std::sync::Arc;

use itertools::Itertools as _;
use jj_lib::backend::CommitId;
use jj_lib::commit::Commit;
use jj_lib::config::ConfigGetError;
use jj_lib::config::ConfigNamePathBuf;
use jj_lib::config::ConfigSource;
use jj_lib::config::StackedConfig;
use jj_lib::id_prefix::IdPrefixContext;
use jj_lib::ref_name::RefNameBuf;
use jj_lib::repo::Repo;
use jj_lib::revset;
use jj_lib::revset::ResolvedRevsetExpression;
use jj_lib::revset::Revset;
use jj_lib::revset::RevsetAliasesMap;
use jj_lib::revset::RevsetDiagnostics;
use jj_lib::revset::RevsetEvaluationError;
use jj_lib::revset::RevsetExpression;
use jj_lib::revset::RevsetExtensions;
use jj_lib::revset::RevsetIteratorExt as _;
use jj_lib::revset::RevsetParseContext;
use jj_lib::revset::RevsetParseError;
use jj_lib::revset::RevsetResolutionError;
use jj_lib::revset::SymbolResolver;
use jj_lib::revset::SymbolResolverExtension;
use jj_lib::revset::UserRevsetExpression;
use thiserror::Error;

use crate::command_error::user_error;
use crate::command_error::CommandError;
use crate::formatter::Formatter;
use crate::templater::TemplateRenderer;
use crate::ui::Ui;

const USER_IMMUTABLE_HEADS: &str = "immutable_heads";

#[derive(Debug, Error)]
pub enum UserRevsetEvaluationError {
    #[error(transparent)]
    Resolution(RevsetResolutionError),
    #[error(transparent)]
    Evaluation(RevsetEvaluationError),
}

/// Wrapper around `UserRevsetExpression` to provide convenient methods.
pub struct RevsetExpressionEvaluator<'repo> {
    repo: &'repo dyn Repo,
    extensions: Arc<RevsetExtensions>,
    id_prefix_context: &'repo IdPrefixContext,
    expression: Rc<UserRevsetExpression>,
}

impl<'repo> RevsetExpressionEvaluator<'repo> {
    pub fn new(
        repo: &'repo dyn Repo,
        extensions: Arc<RevsetExtensions>,
        id_prefix_context: &'repo IdPrefixContext,
        expression: Rc<UserRevsetExpression>,
    ) -> Self {
        RevsetExpressionEvaluator {
            repo,
            extensions,
            id_prefix_context,
            expression,
        }
    }

    /// Returns the underlying expression.
    pub fn expression(&self) -> &Rc<UserRevsetExpression> {
        &self.expression
    }

    /// Intersects the underlying expression with the `other` expression.
    pub fn intersect_with(&mut self, other: &Rc<UserRevsetExpression>) {
        self.expression = self.expression.intersection(other);
    }

    /// Resolves user symbols in the expression, returns new expression.
    pub fn resolve(&self) -> Result<Rc<ResolvedRevsetExpression>, RevsetResolutionError> {
        let symbol_resolver = default_symbol_resolver(
            self.repo,
            self.extensions.symbol_resolvers(),
            self.id_prefix_context,
        );
        self.expression
            .resolve_user_expression(self.repo, &symbol_resolver)
    }

    /// Evaluates the expression.
    pub fn evaluate(&self) -> Result<Box<dyn Revset + 'repo>, UserRevsetEvaluationError> {
        self.resolve()
            .map_err(UserRevsetEvaluationError::Resolution)?
            .evaluate(self.repo)
            .map_err(UserRevsetEvaluationError::Evaluation)
    }

    /// Evaluates the expression to an iterator over commit ids. Entries are
    /// sorted in reverse topological order.
    pub fn evaluate_to_commit_ids(
        &self,
    ) -> Result<
        Box<dyn Iterator<Item = Result<CommitId, RevsetEvaluationError>> + 'repo>,
        UserRevsetEvaluationError,
    > {
        Ok(self.evaluate()?.iter())
    }

    /// Evaluates the expression to an iterator over commit objects. Entries are
    /// sorted in reverse topological order.
    pub fn evaluate_to_commits(
        &self,
    ) -> Result<
        impl Iterator<Item = Result<Commit, RevsetEvaluationError>> + use<'repo>,
        UserRevsetEvaluationError,
    > {
        Ok(self.evaluate()?.iter().commits(self.repo.store()))
    }
}

fn warn_user_redefined_builtin(
    ui: &Ui,
    source: ConfigSource,
    name: &str,
) -> Result<(), CommandError> {
    match source {
        ConfigSource::Default => (),
        ConfigSource::EnvBase
        | ConfigSource::User
        | ConfigSource::RepoManaged
        | ConfigSource::Repo
        | ConfigSource::EnvOverrides
        | ConfigSource::CommandArg => {
            let checked_mutability_builtins =
                ["mutable()", "immutable()", "builtin_immutable_heads()"];

            if checked_mutability_builtins.contains(&name) {
                writeln!(
                    ui.warning_default(),
                    "Redefining `revset-aliases.{name}` is not recommended; redefine \
                     `immutable_heads()` instead",
                )?;
            }
        }
    }

    Ok(())
}

pub fn load_revset_aliases(
    ui: &Ui,
    stacked_config: &StackedConfig,
) -> Result<RevsetAliasesMap, CommandError> {
    let table_name = ConfigNamePathBuf::from_iter(["revset-aliases"]);
    let mut aliases_map = RevsetAliasesMap::new();
    // Load from all config layers in order. 'f(x)' in default layer should be
    // overridden by 'f(a)' in user.
    for layer in stacked_config.layers() {
        let table = match layer.look_up_table(&table_name) {
            Ok(Some(table)) => table,
            Ok(None) => continue,
            Err(item) => {
                return Err(ConfigGetError::Type {
                    name: table_name.to_string(),
                    error: format!("Expected a table, but is {}", item.type_name()).into(),
                    source_path: layer.path.clone(),
                }
                .into());
            }
        };
        for (decl, item) in table.iter() {
            warn_user_redefined_builtin(ui, layer.source, decl)?;

            let r = item
                .as_str()
                .ok_or_else(|| format!("Expected a string, but is {}", item.type_name()))
                .and_then(|v| aliases_map.insert(decl, v).map_err(|e| e.to_string()));
            if let Err(s) = r {
                writeln!(
                    ui.warning_default(),
                    "Failed to load `{table_name}.{decl}`: {s}"
                )?;
            }
        }
    }
    Ok(aliases_map)
}

/// Wraps the given `IdPrefixContext` in `SymbolResolver` to be passed in to
/// `evaluate()`.
pub fn default_symbol_resolver<'a>(
    repo: &'a dyn Repo,
    extensions: &[impl AsRef<dyn SymbolResolverExtension>],
    id_prefix_context: &'a IdPrefixContext,
) -> SymbolResolver<'a> {
    SymbolResolver::new(repo, extensions).with_id_prefix_context(id_prefix_context)
}

/// Parses user-configured expression defining the heads of the immutable set.
/// Includes the root commit.
pub fn parse_immutable_heads_expression(
    diagnostics: &mut RevsetDiagnostics,
    context: &RevsetParseContext,
) -> Result<Rc<UserRevsetExpression>, RevsetParseError> {
    let (_, _, immutable_heads_str) = context
        .aliases_map
        .get_function(USER_IMMUTABLE_HEADS, 0)
        .unwrap();
    let heads = revset::parse(diagnostics, immutable_heads_str, context)?;
    Ok(heads.union(&RevsetExpression::root()))
}

/// Prints warning if `trunk()` alias cannot be resolved. This alias could be
/// generated by `jj git init`/`clone`.
pub(super) fn warn_unresolvable_trunk(
    ui: &Ui,
    repo: &dyn Repo,
    context: &RevsetParseContext,
) -> io::Result<()> {
    let (_, _, revset_str) = context
        .aliases_map
        .get_function("trunk", 0)
        .expect("trunk() should be defined by default");
    let Ok(expression) = revset::parse(&mut RevsetDiagnostics::new(), revset_str, context) else {
        // Parse error would have been reported.
        return Ok(());
    };
    // Not using IdPrefixContext since trunk() revset shouldn't contain short
    // prefixes.
    let symbol_resolver = SymbolResolver::new(repo, context.extensions.symbol_resolvers());
    if let Err(err) = expression.resolve_user_expression(repo, &symbol_resolver) {
        writeln!(
            ui.warning_default(),
            "Failed to resolve `revset-aliases.trunk()`: {err}"
        )?;
        writeln!(
            ui.hint_default(),
            "Use `jj config edit --repo` to adjust the `trunk()` alias."
        )?;
    }
    Ok(())
}

pub(super) fn evaluate_revset_to_single_commit<'a>(
    revision_str: &str,
    expression: &RevsetExpressionEvaluator<'_>,
    commit_summary_template: impl FnOnce() -> TemplateRenderer<'a, Commit>,
) -> Result<Commit, CommandError> {
    let mut iter = expression.evaluate_to_commits()?.fuse();
    match (iter.next(), iter.next()) {
        (Some(commit), None) => Ok(commit?),
        (None, _) => Err(user_error(format!(
            "Revset `{revision_str}` didn't resolve to any revisions"
        ))),
        (Some(commit0), Some(commit1)) => {
            let mut iter = [commit0, commit1].into_iter().chain(iter);
            let commits: Vec<_> = iter.by_ref().take(5).try_collect()?;
            let elided = iter.next().is_some();
            Err(format_multiple_revisions_error(
                revision_str,
                &commits,
                elided,
                &commit_summary_template(),
            ))
        }
    }
}

fn format_multiple_revisions_error(
    revision_str: &str,
    commits: &[Commit],
    elided: bool,
    template: &TemplateRenderer<'_, Commit>,
) -> CommandError {
    assert!(commits.len() >= 2);
    let mut cmd_err = user_error(format!(
        "Revset `{revision_str}` resolved to more than one revision"
    ));
    let write_commits_summary = |formatter: &mut dyn Formatter| {
        for commit in commits {
            write!(formatter, "  ")?;
            template.format(commit, formatter)?;
            writeln!(formatter)?;
        }
        if elided {
            writeln!(formatter, "  ...")?;
        }
        Ok(())
    };
    cmd_err.add_formatted_hint_with(|formatter| {
        writeln!(
            formatter,
            "The revset `{revision_str}` resolved to these revisions:"
        )?;
        write_commits_summary(formatter)
    });
    cmd_err
}

#[derive(Debug, Error)]
#[error("Failed to parse bookmark name: {}", source.kind())]
pub struct BookmarkNameParseError {
    pub input: String,
    pub source: RevsetParseError,
}

/// Parses bookmark name specified in revset syntax.
pub fn parse_bookmark_name(text: &str) -> Result<RefNameBuf, BookmarkNameParseError> {
    revset::parse_symbol(text)
        .map(Into::into)
        .map_err(|source| BookmarkNameParseError {
            input: text.to_owned(),
            source,
        })
}

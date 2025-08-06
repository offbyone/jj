// Copyright 2020-2023 The Jujutsu Authors
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

mod create;
mod delete;
mod forget;
mod list;
mod r#move;
mod rename;
mod set;
mod track;
mod untrack;

use bstr::ByteSlice;
use gix::remote;
use itertools::Itertools as _;
use jj_lib::backend::CommitId;
use jj_lib::op_store::RefTarget;
use jj_lib::op_store::RemoteRef;
use jj_lib::ref_name::RefName;
use jj_lib::ref_name::RemoteName;
use jj_lib::ref_name::RemoteRefSymbol;
use jj_lib::repo::Repo;
use jj_lib::str_util::StringPattern;
use jj_lib::view::View;

use self::create::BookmarkCreateArgs;
use self::create::cmd_bookmark_create;
use self::delete::BookmarkDeleteArgs;
use self::delete::cmd_bookmark_delete;
use self::forget::BookmarkForgetArgs;
use self::forget::cmd_bookmark_forget;
use self::list::BookmarkListArgs;
use self::list::cmd_bookmark_list;
use self::r#move::BookmarkMoveArgs;
use self::r#move::cmd_bookmark_move;
use self::rename::BookmarkRenameArgs;
use self::rename::cmd_bookmark_rename;
use self::set::BookmarkSetArgs;
use self::set::cmd_bookmark_set;
use self::track::BookmarkTrackArgs;
use self::track::cmd_bookmark_track;
use self::untrack::BookmarkUntrackArgs;
use self::untrack::cmd_bookmark_untrack;
use crate::cli_util::CommandHelper;
use crate::cli_util::RemoteBookmarkNamePattern;
use crate::command_error::CommandError;
use crate::command_error::user_error;
use crate::ui::Ui;

// Unlike most other aliases, `b` is defined in the config and can be overridden
// by the user.

/// Manage bookmarks [default alias: b]
///
/// See [`jj help -k bookmarks`] for more information.
///
/// [`jj help -k bookmarks`]:
///     https://jj-vcs.github.io/jj/latest/bookmarks
#[derive(clap::Subcommand, Clone, Debug)]
pub enum BookmarkCommand {
    #[command(visible_alias("c"))]
    Create(BookmarkCreateArgs),
    #[command(visible_alias("d"))]
    Delete(BookmarkDeleteArgs),
    #[command(visible_alias("f"))]
    Forget(BookmarkForgetArgs),
    #[command(visible_alias("l"))]
    List(BookmarkListArgs),
    #[command(visible_alias("m"))]
    Move(BookmarkMoveArgs),
    #[command(visible_alias("r"))]
    Rename(BookmarkRenameArgs),
    #[command(visible_alias("s"))]
    Set(BookmarkSetArgs),
    #[command(visible_alias("t"))]
    Track(BookmarkTrackArgs),
    Untrack(BookmarkUntrackArgs),
}

pub fn cmd_bookmark(
    ui: &mut Ui,
    command: &CommandHelper,
    subcommand: &BookmarkCommand,
) -> Result<(), CommandError> {
    match subcommand {
        BookmarkCommand::Create(args) => cmd_bookmark_create(ui, command, args),
        BookmarkCommand::Delete(args) => cmd_bookmark_delete(ui, command, args),
        BookmarkCommand::Forget(args) => cmd_bookmark_forget(ui, command, args),
        BookmarkCommand::List(args) => cmd_bookmark_list(ui, command, args),
        BookmarkCommand::Move(args) => cmd_bookmark_move(ui, command, args),
        BookmarkCommand::Rename(args) => cmd_bookmark_rename(ui, command, args),
        BookmarkCommand::Set(args) => cmd_bookmark_set(ui, command, args),
        BookmarkCommand::Track(args) => cmd_bookmark_track(ui, command, args),
        BookmarkCommand::Untrack(args) => cmd_bookmark_untrack(ui, command, args),
    }
}

fn find_local_bookmarks<'a>(
    view: &'a View,
    name_patterns: &[StringPattern],
) -> Result<Vec<(&'a RefName, &'a RefTarget)>, CommandError> {
    find_bookmarks_with(name_patterns, |pattern| {
        view.local_bookmarks_matching(pattern).map(Ok)
    })
}

fn find_bookmarks_with<'a, 'b, V, I>(
    name_patterns: &'b [StringPattern],
    mut find_matches: impl FnMut(&'b StringPattern) -> I,
) -> Result<Vec<(&'a RefName, V)>, CommandError>
where
    I: Iterator<Item = Result<(&'a RefName, V), CommandError>>,
{
    let mut matching_bookmarks: Vec<(&'a RefName, V)> = vec![];
    let mut unmatched_patterns = vec![];
    for pattern in name_patterns {
        let mut matches = find_matches(pattern).peekable();
        if matches.peek().is_none() {
            unmatched_patterns.push(pattern);
        }
        matches.process_results(|iter| matching_bookmarks.extend(iter))?;
    }
    match &unmatched_patterns[..] {
        [] => {
            matching_bookmarks.sort_unstable_by_key(|(name, _)| *name);
            matching_bookmarks.dedup_by_key(|(name, _)| *name);
            Ok(matching_bookmarks)
        }
        [pattern] if pattern.is_exact() => Err(user_error(format!("No such bookmark: {pattern}"))),
        patterns => Err(user_error(format!(
            "No matching bookmarks for patterns: {}",
            patterns.iter().join(", ")
        ))),
    }
}

fn find_remote_bookmarks<'a>(
    view: &'a View,
    name_patterns: &[RemoteBookmarkNamePattern],
    remote_names: &'a remote::Names<'_>,
) -> Result<Vec<(RemoteRefSymbol<'a>, Option<&'a RemoteRef>)>, CommandError> {
    let mut matching_bookmarks = vec![];
    let mut unmatched_patterns = vec![];
    for pattern in name_patterns {
        let mut matches = view
            .remote_bookmarks_matching(&pattern.bookmark, &pattern.remote)
            .peekable();
        if matches.peek().is_none() {
            let mut matches = view.local_bookmarks_matching(&pattern.bookmark).peekable();
            let mut remotes = remote_names
                .iter()
                .filter(|r| pattern.remote.matches(&r.to_str_lossy()))
                .peekable();
            if matches.peek().is_none() || remotes.peek().is_none() {
                unmatched_patterns.push(pattern);
            } else {
                let remotes = remotes.collect::<Vec<_>>();
                for (name, _) in matches {
                    for &remote in &remotes {
                        matching_bookmarks.push((
                            RemoteRefSymbol {
                                name,
                                remote: RemoteName::new(remote.to_str().unwrap()),
                            },
                            None,
                        ));
                    }
                }
            }
        }
        matching_bookmarks.extend(matches.map(|(symbol, remote_ref)| (symbol, Some(remote_ref))));
    }
    match &unmatched_patterns[..] {
        [] => {
            matching_bookmarks.sort_unstable_by(|(sym1, _), (sym2, _)| sym1.cmp(sym2));
            matching_bookmarks.dedup_by(|(sym1, _), (sym2, _)| sym1 == sym2);
            Ok(matching_bookmarks)
        }
        [pattern] if pattern.is_exact() => {
            Err(user_error(format!("No such remote bookmark: {pattern}")))
        }
        patterns => Err(user_error(format!(
            "No matching remote bookmarks for patterns: {}",
            patterns.iter().join(", ")
        ))),
    }
}

fn is_fast_forward(repo: &dyn Repo, old_target: &RefTarget, new_target_id: &CommitId) -> bool {
    if old_target.is_present() {
        // Strictly speaking, "all" old targets should be ancestors, but we allow
        // conflict resolution by setting bookmark to "any" of the old target
        // descendants.
        old_target
            .added_ids()
            .any(|old| repo.index().is_ancestor(old, new_target_id))
    } else {
        true
    }
}

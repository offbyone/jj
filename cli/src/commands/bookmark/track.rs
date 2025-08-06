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

use std::collections::HashMap;
use std::rc::Rc;

use clap_complete::ArgValueCandidates;
use itertools::Itertools as _;
use jj_lib::git;
use jj_lib::repo::Repo as _;

use super::find_remote_bookmarks;
use crate::cli_util::CommandHelper;
use crate::cli_util::RemoteBookmarkNamePattern;
use crate::command_error::CommandError;
use crate::commit_templater::CommitRef;
use crate::complete;
use crate::templater::TemplateRenderer;
use crate::ui::Ui;

/// Start tracking given remote bookmarks
///
/// A tracking remote bookmark will be imported as a local bookmark of the same
/// name. Changes to it will propagate to the existing local bookmark on future
/// pulls.
#[derive(clap::Args, Clone, Debug)]
pub struct BookmarkTrackArgs {
    /// Remote bookmarks to track
    ///
    /// By default, the specified name matches exactly. Use `glob:` prefix to
    /// select bookmarks by [wildcard pattern].
    ///
    /// Examples: bookmark@remote, glob:main@*, glob:jjfan-*@upstream
    ///
    /// [wildcard pattern]:
    ///     https://jj-vcs.github.io/jj/latest/revsets/#string-patterns
    #[arg(
        required = true,
        value_name = "BOOKMARK@REMOTE",
        add = ArgValueCandidates::new(complete::untracked_bookmarks),
    )]
    names: Vec<RemoteBookmarkNamePattern>,
}

pub fn cmd_bookmark_track(
    ui: &mut Ui,
    command: &CommandHelper,
    args: &BookmarkTrackArgs,
) -> Result<(), CommandError> {
    let mut workspace_command = command.workspace_helper(ui)?;
    let repo = workspace_command.repo().clone();
    let git_repo = git::get_git_repo(repo.store())?;
    let remote_names = git_repo.remote_names();
    let mut symbols = Vec::new();
    for (symbol, remote_ref) in find_remote_bookmarks(repo.view(), &args.names, &remote_names)? {
        if remote_ref.is_some_and(|r| r.is_tracked()) {
            writeln!(
                ui.warning_default(),
                "Remote bookmark already tracked: {symbol}"
            )?;
        } else {
            symbols.push(symbol);
        }
    }
    let mut tx = workspace_command.start_transaction();
    for &symbol in &symbols {
        tx.repo_mut().track_remote_bookmark(symbol);
    }
    if !symbols.is_empty() {
        writeln!(
            ui.status(),
            "Started tracking {} remote bookmarks.",
            symbols.len()
        )?;
    }
    tx.finish(
        ui,
        format!("track remote bookmark {}", symbols.iter().join(", ")),
    )?;

    //show conflicted bookmarks if there are some

    if let Some(mut formatter) = ui.status_formatter() {
        let template: TemplateRenderer<Rc<CommitRef>> = {
            let language = workspace_command.commit_template_language();
            let text = workspace_command
                .settings()
                .get::<String>("templates.bookmark_list")?;
            workspace_command
                .parse_template(ui, &language, &text)?
                .labeled(["bookmark_list"])
        };

        let mut remote_per_bookmark: HashMap<_, Vec<_>> = HashMap::new();
        for symbol in &symbols {
            remote_per_bookmark
                .entry(symbol.name)
                .or_default()
                .push(symbol.remote);
        }
        let bookmarks_to_list =
            workspace_command
                .repo()
                .view()
                .bookmarks()
                .filter(|(name, target)| {
                    remote_per_bookmark.contains_key(name) && target.local_target.has_conflict()
                });

        for (name, bookmark_target) in bookmarks_to_list {
            let local_target = bookmark_target.local_target;
            let commit_ref = CommitRef::local(
                name,
                local_target.clone(),
                bookmark_target.remote_refs.iter().map(|x| x.1),
            );
            template.format(&commit_ref, formatter.as_mut())?;

            for (remote_name, remote_ref) in bookmark_target.remote_refs {
                if remote_per_bookmark[name].contains(&remote_name) {
                    let commit_ref =
                        CommitRef::remote(name, remote_name, remote_ref.clone(), local_target);
                    template.format(&commit_ref, formatter.as_mut())?;
                }
            }
        }
    }
    Ok(())
}

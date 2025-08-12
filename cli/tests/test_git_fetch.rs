// Copyright 2023 The Jujutsu Authors
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

use testutils::git;

use crate::common::CommandOutput;
use crate::common::TestEnvironment;
use crate::common::TestWorkDir;
use crate::common::create_commit;

fn add_commit_to_branch(git_repo: &gix::Repository, branch: &str) -> gix::ObjectId {
    git::add_commit(
        git_repo,
        &format!("refs/heads/{branch}"),
        branch,            // filename
        branch.as_bytes(), // content
        "message",
        &[],
    )
    .commit_id
}

/// Creates a remote Git repo containing a bookmark with the same name
fn init_git_remote(test_env: &TestEnvironment, remote: &str) -> gix::Repository {
    let git_repo_path = test_env.env_root().join(remote);
    let git_repo = git::init(git_repo_path);
    add_commit_to_branch(&git_repo, remote);

    git_repo
}

/// Add a remote containing a bookmark with the same name
fn add_git_remote(
    test_env: &TestEnvironment,
    work_dir: &TestWorkDir,
    remote: &str,
) -> gix::Repository {
    let repo = init_git_remote(test_env, remote);
    work_dir
        .run_jj(["git", "remote", "add", remote, &format!("../{remote}")])
        .success();

    repo
}

#[must_use]
fn get_bookmark_output(work_dir: &TestWorkDir) -> CommandOutput {
    // --quiet to suppress deleted bookmarks hint
    work_dir.run_jj(["bookmark", "list", "--all-remotes", "--quiet"])
}

#[must_use]
fn get_log_output(work_dir: &TestWorkDir) -> CommandOutput {
    let template =
        r#"commit_id.short() ++ " \"" ++ description.first_line() ++ "\" " ++ bookmarks"#;
    work_dir.run_jj(["log", "-T", template, "-r", "all()"])
}

fn clone_git_remote_into(
    test_env: &TestEnvironment,
    upstream: &str,
    fork: &str,
) -> gix::Repository {
    let upstream_path = test_env.env_root().join(upstream);
    let fork_path = test_env.env_root().join(fork);
    let fork_repo = git::clone(&fork_path, upstream_path.to_str().unwrap(), Some(upstream));

    // create local branch mirroring the upstream
    let upstream_head = fork_repo
        .find_reference(&format!("refs/remotes/{upstream}/{upstream}"))
        .unwrap()
        .peel_to_id_in_place()
        .unwrap()
        .detach();

    fork_repo
        .reference(
            format!("refs/heads/{upstream}"),
            upstream_head,
            gix::refs::transaction::PreviousValue::MustNotExist,
            "create tracking head",
        )
        .unwrap();

    fork_repo
}

#[test]
fn test_git_fetch_with_default_config() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "origin");

    work_dir.run_jj(["git", "fetch"]).success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin@origin: qmyrypzk ab8b299e message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_default_remote() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "origin");

    work_dir.run_jj(["git", "fetch"]).success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin: qmyrypzk ab8b299e message
      @origin: qmyrypzk ab8b299e message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_single_remote() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");

    let output = work_dir.run_jj(["git", "fetch"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Hint: Fetching from the only existing remote: rem1
    bookmark: rem1@rem1 [new] tracked
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1: ppspxspk 4acd0343 message
      @rem1: ppspxspk 4acd0343 message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_single_remote_all_remotes_flag() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");

    work_dir.run_jj(["git", "fetch", "--all-remotes"]).success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1: ppspxspk 4acd0343 message
      @rem1: ppspxspk 4acd0343 message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_single_remote_from_arg() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");

    work_dir
        .run_jj(["git", "fetch", "--remote", "rem1"])
        .success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1: ppspxspk 4acd0343 message
      @rem1: ppspxspk 4acd0343 message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_single_remote_from_config() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");
    test_env.add_config(r#"git.fetch = "rem1""#);

    work_dir.run_jj(["git", "fetch"]).success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1: ppspxspk 4acd0343 message
      @rem1: ppspxspk 4acd0343 message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_multiple_remotes() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");
    add_git_remote(&test_env, &work_dir, "rem2");

    work_dir
        .run_jj(["git", "fetch", "--remote", "rem1", "--remote", "rem2"])
        .success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1: ppspxspk 4acd0343 message
      @rem1: ppspxspk 4acd0343 message
    rem2: pzqqpnpo 44c57802 message
      @rem2: pzqqpnpo 44c57802 message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_with_glob() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");
    add_git_remote(&test_env, &work_dir, "rem2");

    let output = work_dir.run_jj(["git", "fetch", "--remote", "glob:*"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: rem1@rem1 [new] untracked
    bookmark: rem2@rem2 [new] untracked
    [EOF]
    ");
}

#[test]
fn test_git_fetch_with_glob_and_exact_match() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");
    add_git_remote(&test_env, &work_dir, "rem2");
    add_git_remote(&test_env, &work_dir, "upstream1");
    add_git_remote(&test_env, &work_dir, "upstream2");
    add_git_remote(&test_env, &work_dir, "origin");

    let output = work_dir.run_jj(["git", "fetch", "--remote=glob:rem*", "--remote=origin"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: origin@origin [new] untracked
    bookmark: rem1@rem1     [new] untracked
    bookmark: rem2@rem2     [new] untracked
    [EOF]
    ");
}

#[test]
fn test_git_fetch_with_glob_from_config() {
    let test_env = TestEnvironment::default();
    test_env.add_config(r#"git.fetch = "glob:rem*""#);
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");
    add_git_remote(&test_env, &work_dir, "rem2");
    add_git_remote(&test_env, &work_dir, "upstream");

    let output = work_dir.run_jj(["git", "fetch"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: rem1@rem1 [new] untracked
    bookmark: rem2@rem2 [new] untracked
    [EOF]
    ");
}

#[test]
fn test_git_fetch_with_glob_with_no_matching_remotes() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "upstream");

    let output = work_dir.run_jj(["git", "fetch", "--remote=glob:rem*"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Warning: No git remotes matching 'rem*'
    Error: No git remotes to fetch from
    [EOF]
    [exit status: 1]
    ");
    // No remote should have been fetched as part of the failing transaction
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @"");
}

#[test]
fn test_git_fetch_with_tracked_single() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "origin");
    add_git_remote(&test_env, &work_dir, "upstream");
    insta::assert_snapshot!(work_dir.run_jj(["git", "fetch", "--all-remotes"]), @r"
    ------- stderr -------
    bookmark: origin@origin     [new] untracked
    bookmark: upstream@upstream [new] untracked
    [EOF]
    ");

    work_dir
        .run_jj(["bookmark", "track", "upstream@upstream"])
        .success();

    insta::assert_snapshot!(work_dir.run_jj(["git", "fetch", "--tracked"]), @r"
    ------- stderr -------
    bookmark: upstream@upstream [new] untracked
    [EOF]
    ");

    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    upstream: qmyrypzk ab8b299e message
      @upstream: qmyrypzk ab8b299e message
    rem1: ppspxspk 4acd0343 message
        @rem1: ppspxspk 4acd0343 message
    [EOF]
    [exit status: 0]
    ");
}

#[test]
fn test_git_fetch_with_tracked_multiple() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "upstream");
    add_git_remote(&test_env, &work_dir, "rem1");

    work_dir
        .run_jj(["bookmark", "track", "upstream@upstream"])
        .success();
    work_dir
        .run_jj(["bookmark", "track", "rem1@rem1"])
        .success();

    work_dir.run_jj(["git", "fetch", "--tracked"]).success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    upstream: qmyrypzk ab8b299e message
      @upstream: qmyrypzk ab8b299e message
    rem1: ppspxspk 4acd0343 message
        @rem1: ppspxspk 4acd0343 message
    [EOF]
    [exit status: 0]
    ");
}

#[test]
fn test_git_fetch_with_tracked_conflicting() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "upstream");

    let output = work_dir.run_jj(["git", "fetch", "--tracked", "--remote=glob:rem*"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    error: the argument '--tracked' cannot be used with '--remote <REMOTE>'
    
    Usage: jj git fetch --tracked
    
    For more information, try '--help'.
    [EOF]
    [exit status: 2]
    ");

    let output = work_dir.run_jj(["git", "fetch", "--tracked", "--branch", "a1"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    error: the argument '--tracked' cannot be used with '--branch <BRANCH>'
    
    Usage: jj git fetch --tracked
    
    For more information, try '--help'.
    [EOF]
    [exit status: 2]
    ");
}

#[test]
fn test_git_fetch_all_remotes() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");
    add_git_remote(&test_env, &work_dir, "rem2");

    // add empty [remote "rem3"] section to .git/config, which should be ignored
    work_dir
        .run_jj(["git", "remote", "add", "rem3", "../unknown"])
        .success();
    work_dir
        .run_jj(["git", "remote", "remove", "rem3"])
        .success();

    work_dir.run_jj(["git", "fetch", "--all-remotes"]).success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1: ppspxspk 4acd0343 message
      @rem1: ppspxspk 4acd0343 message
    rem2: pzqqpnpo 44c57802 message
      @rem2: pzqqpnpo 44c57802 message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_multiple_remotes_from_config() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");
    add_git_remote(&test_env, &work_dir, "rem2");
    test_env.add_config(r#"git.fetch = ["rem1", "rem2"]"#);

    work_dir.run_jj(["git", "fetch"]).success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1: ppspxspk 4acd0343 message
      @rem1: ppspxspk 4acd0343 message
    rem2: pzqqpnpo 44c57802 message
      @rem2: pzqqpnpo 44c57802 message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_no_matching_remote() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    let output = work_dir.run_jj(["git", "fetch", "--remote", "rem1"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Warning: No git remotes matching 'rem1'
    Error: No git remotes to fetch from
    [EOF]
    [exit status: 1]
    ");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @"");
}

#[test]
fn test_git_fetch_nonexistent_remote() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");

    let output = work_dir.run_jj(["git", "fetch", "--remote", "rem1", "--remote", "rem2"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Warning: No git remotes matching 'rem2'
    bookmark: rem1@rem1 [new] untracked
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1@rem1: ppspxspk 4acd0343 message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_nonexistent_remote_from_config() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");
    test_env.add_config(r#"git.fetch = ["rem1", "rem2"]"#);

    let output = work_dir.run_jj(["git", "fetch"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Warning: No git remotes matching 'rem2'
    bookmark: rem1@rem1 [new] untracked
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1@rem1: ppspxspk 4acd0343 message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_from_remote_named_git() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    let work_dir = test_env.work_dir("repo");
    init_git_remote(&test_env, "git");

    git::init(work_dir.root());
    git::add_remote(work_dir.root(), "git", "../git");

    // Existing remote named 'git' shouldn't block the repo initialization.
    work_dir.run_jj(["git", "init", "--git-repo=."]).success();

    // Try fetching from the remote named 'git'.
    let output = work_dir.run_jj(["git", "fetch", "--remote=git"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Error: Git remote named 'git' is reserved for local Git repository
    Hint: Run `jj git remote rename` to give a different name.
    [EOF]
    [exit status: 1]
    ");

    // Fetch remote refs by using the git CLI.
    git::fetch(work_dir.root(), "git");

    // Implicit import shouldn't fail because of the remote ref.
    let output = work_dir.run_jj(["bookmark", "list", "--all-remotes"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Warning: Failed to import some Git refs:
      refs/remotes/git/git
    Hint: Git remote named 'git' is reserved for local Git repository.
    Use `jj git remote rename` to give a different name.
    [EOF]
    ");

    // Explicit import also works. Warnings are printed twice because this is a
    // colocated repo. That should be fine since "jj git import" wouldn't be
    // used in colocated environment.
    insta::assert_snapshot!(work_dir.run_jj(["git", "import"]), @r"
    ------- stderr -------
    Warning: Failed to import some Git refs:
      refs/remotes/git/git
    Hint: Git remote named 'git' is reserved for local Git repository.
    Use `jj git remote rename` to give a different name.
    Warning: Failed to import some Git refs:
      refs/remotes/git/git
    Hint: Git remote named 'git' is reserved for local Git repository.
    Use `jj git remote rename` to give a different name.
    Nothing changed.
    [EOF]
    ");

    // The remote can be renamed, and the ref can be imported.
    work_dir
        .run_jj(["git", "remote", "rename", "git", "bar"])
        .success();
    let output = work_dir.run_jj(["bookmark", "list", "--all-remotes"]);
    insta::assert_snapshot!(output, @r"
    git: vkponlun 400c483d message
      @bar: vkponlun 400c483d message
      @git: vkponlun 400c483d message
    [EOF]
    ------- stderr -------
    Done importing changes from the underlying Git repo.
    [EOF]
    ");
}

#[test]
fn test_git_fetch_from_remote_with_slashes() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    let work_dir = test_env.work_dir("repo");
    init_git_remote(&test_env, "source");

    git::init(work_dir.root());
    git::add_remote(work_dir.root(), "slash/origin", "../source");

    // Existing remote with slash shouldn't block the repo initialization.
    work_dir.run_jj(["git", "init", "--git-repo=."]).success();

    // Try fetching from the remote named 'git'.
    let output = work_dir.run_jj(["git", "fetch", "--remote=slash/origin"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Error: Git remotes with slashes are incompatible with jj: slash/origin
    Hint: Run `jj git remote rename` to give a different name.
    [EOF]
    [exit status: 1]
    ");
}

#[test]
fn test_git_fetch_prune_before_updating_tips() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    let git_repo = add_git_remote(&test_env, &work_dir, "origin");
    work_dir.run_jj(["git", "fetch"]).success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin: qmyrypzk ab8b299e message
      @origin: qmyrypzk ab8b299e message
    [EOF]
    ");

    // Remove origin bookmark in git repo and create origin/subname
    let mut origin_reference = git_repo.find_reference("refs/heads/origin").unwrap();
    let commit_id = origin_reference.peel_to_commit().unwrap().id().detach();
    origin_reference.delete().unwrap();
    git_repo
        .reference(
            "refs/heads/origin/subname",
            commit_id,
            gix::refs::transaction::PreviousValue::MustNotExist,
            "create new reference",
        )
        .unwrap();

    work_dir.run_jj(["git", "fetch"]).success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin/subname: qmyrypzk ab8b299e message
      @origin: qmyrypzk ab8b299e message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_conflicting_bookmarks() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "rem1");

    // Create a rem1 bookmark locally
    work_dir.run_jj(["new", "root()"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "rem1"])
        .success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1: kkmpptxz 2b17ac71 (empty) (no description set)
    [EOF]
    ");

    work_dir
        .run_jj(["git", "fetch", "--remote", "rem1", "--branch", "glob:*"])
        .success();
    // This should result in a CONFLICTED bookmark
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1 (conflicted):
      + kkmpptxz 2b17ac71 (empty) (no description set)
      + ppspxspk 4acd0343 message
      @rem1 (behind by 1 commits): ppspxspk 4acd0343 message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_conflicting_bookmarks_colocated() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    let work_dir = test_env.work_dir("repo");
    git::init(work_dir.root());
    // create_colocated_repo_and_bookmarks_from_trunk1(&test_env, &repo_path);
    work_dir
        .run_jj(["git", "init", "--git-repo", "."])
        .success();
    add_git_remote(&test_env, &work_dir, "rem1");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @"");

    // Create a rem1 bookmark locally
    work_dir.run_jj(["new", "root()"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "rem1"])
        .success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1: zsuskuln c2934cfb (empty) (no description set)
      @git: zsuskuln c2934cfb (empty) (no description set)
    [EOF]
    ");

    work_dir
        .run_jj(["git", "fetch", "--remote", "rem1", "--branch", "rem1"])
        .success();
    // This should result in a CONFLICTED bookmark
    // See https://github.com/jj-vcs/jj/pull/1146#discussion_r1112372340 for the bug this tests for.
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    rem1 (conflicted):
      + zsuskuln c2934cfb (empty) (no description set)
      + ppspxspk 4acd0343 message
      @git (behind by 1 commits): zsuskuln c2934cfb (empty) (no description set)
      @rem1 (behind by 1 commits): ppspxspk 4acd0343 message
    [EOF]
    ");
}

// Helper functions to test obtaining multiple bookmarks at once and changed
// bookmarks
fn create_colocated_repo_and_bookmarks_from_trunk1(work_dir: &TestWorkDir) -> String {
    // Create a colocated repo in `source` to populate it more easily
    work_dir
        .run_jj(["git", "init", "--git-repo", "."])
        .success();
    create_commit(work_dir, "trunk1", &[]);
    create_commit(work_dir, "a1", &["trunk1"]);
    create_commit(work_dir, "a2", &["trunk1"]);
    create_commit(work_dir, "b", &["trunk1"]);
    format!(
        "   ===== Source git repo contents =====\n{}",
        get_log_output(work_dir)
    )
}

fn create_trunk2_and_rebase_bookmarks(work_dir: &TestWorkDir) -> String {
    create_commit(work_dir, "trunk2", &["trunk1"]);
    for br in ["a1", "a2", "b"] {
        work_dir
            .run_jj(["rebase", "-b", br, "-d", "trunk2"])
            .success();
    }
    format!(
        "   ===== Source git repo contents =====\n{}",
        get_log_output(work_dir)
    )
}

#[test]
fn test_git_fetch_all() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.add_config(r#"revset-aliases."immutable_heads()" = "none()""#);
    let source_dir = test_env.work_dir("source");
    git::init(source_dir.root());

    // Clone an empty repo. The target repo is a normal `jj` repo, *not* colocated
    let output = test_env.run_jj_in(".", ["git", "clone", "source", "target"]);
    insta::assert_snapshot!(output, @r#"
    ------- stderr -------
    Fetching into new repo in "$TEST_ENV/target"
    Nothing changed.
    [EOF]
    "#);
    let target_dir = test_env.work_dir("target");

    let source_log = create_colocated_repo_and_bookmarks_from_trunk1(&source_dir);
    insta::assert_snapshot!(source_log, @r#"
       ===== Source git repo contents =====
    @  bc83465a3090 "b" b
    │ ○  d4d535f1d579 "a2" a2
    ├─╯
    │ ○  c8303692b8e2 "a1" a1
    ├─╯
    ○  382881770501 "trunk1" trunk1
    ◆  000000000000 ""
    [EOF]
    "#);

    // Nothing in our repo before the fetch
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    ◆  000000000000 ""
    [EOF]
    "#);
    insta::assert_snapshot!(get_bookmark_output(&target_dir), @"");
    let output = target_dir.run_jj(["git", "fetch"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: a1@origin     [new] tracked
    bookmark: a2@origin     [new] tracked
    bookmark: b@origin      [new] tracked
    bookmark: trunk1@origin [new] tracked
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&target_dir), @r"
    a1: mzvwutvl c8303692 a1
      @origin: mzvwutvl c8303692 a1
    a2: yqosqzyt d4d535f1 a2
      @origin: yqosqzyt d4d535f1 a2
    b: yostqsxw bc83465a b
      @origin: yostqsxw bc83465a b
    trunk1: kkmpptxz 38288177 trunk1
      @origin: kkmpptxz 38288177 trunk1
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  bc83465a3090 "b" b
    │ │ ○  d4d535f1d579 "a2" a2
    │ ├─╯
    │ │ ○  c8303692b8e2 "a1" a1
    │ ├─╯
    │ ○  382881770501 "trunk1" trunk1
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);

    // ==== Change both repos ====
    // First, change the target repo:
    let source_log = create_trunk2_and_rebase_bookmarks(&source_dir);
    insta::assert_snapshot!(source_log, @r#"
       ===== Source git repo contents =====
    ○  6fc6fe17dbee "b" b
    │ ○  baad96fead6c "a2" a2
    ├─╯
    │ ○  798c5e2435e1 "a1" a1
    ├─╯
    @  e80d998ab04b "trunk2" trunk2
    ○  382881770501 "trunk1" trunk1
    ◆  000000000000 ""
    [EOF]
    "#);
    // Change a bookmark in the source repo as well, so that it becomes conflicted.
    target_dir
        .run_jj(["describe", "b", "-m=new_descr_for_b_to_create_conflict"])
        .success();

    // Our repo before and after fetch
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  0fbbc495357c "new_descr_for_b_to_create_conflict" b*
    │ │ ○  d4d535f1d579 "a2" a2
    │ ├─╯
    │ │ ○  c8303692b8e2 "a1" a1
    │ ├─╯
    │ ○  382881770501 "trunk1" trunk1
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
    insta::assert_snapshot!(get_bookmark_output(&target_dir), @r"
    a1: mzvwutvl c8303692 a1
      @origin: mzvwutvl c8303692 a1
    a2: yqosqzyt d4d535f1 a2
      @origin: yqosqzyt d4d535f1 a2
    b: yostqsxw 0fbbc495 new_descr_for_b_to_create_conflict
      @origin (ahead by 1 commits, behind by 1 commits): yostqsxw hidden bc83465a b
    trunk1: kkmpptxz 38288177 trunk1
      @origin: kkmpptxz 38288177 trunk1
    [EOF]
    ");
    let output = target_dir.run_jj(["git", "fetch"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: a1@origin     [updated] tracked
    bookmark: a2@origin     [updated] tracked
    bookmark: b@origin      [updated] tracked
    bookmark: trunk2@origin [new] tracked
    Abandoned 2 commits that are no longer reachable.
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&target_dir), @r"
    a1: mzvwutvl 798c5e24 a1
      @origin: mzvwutvl 798c5e24 a1
    a2: yqosqzyt baad96fe a2
      @origin: yqosqzyt baad96fe a2
    b (conflicted):
      - yostqsxw hidden bc83465a b
      + yostqsxw?? 0fbbc495 new_descr_for_b_to_create_conflict
      + yostqsxw?? 6fc6fe17 b
      @origin (behind by 1 commits): yostqsxw?? 6fc6fe17 b
    trunk1: kkmpptxz 38288177 trunk1
      @origin: kkmpptxz 38288177 trunk1
    trunk2: uyznsvlq e80d998a trunk2
      @origin: uyznsvlq e80d998a trunk2
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  6fc6fe17dbee "b" b?? b@origin
    │ │ ○  baad96fead6c "a2" a2
    │ ├─╯
    │ │ ○  798c5e2435e1 "a1" a1
    │ ├─╯
    │ ○  e80d998ab04b "trunk2" trunk2
    │ │ ○  0fbbc495357c "new_descr_for_b_to_create_conflict" b??
    │ ├─╯
    │ ○  382881770501 "trunk1" trunk1
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
}

#[test]
fn test_git_fetch_some_of_many_bookmarks() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.add_config(r#"revset-aliases."immutable_heads()" = "none()""#);
    let source_dir = test_env.work_dir("source");
    git::init(source_dir.root());

    // Clone an empty repo. The target repo is a normal `jj` repo, *not* colocated
    let output = test_env.run_jj_in(".", ["git", "clone", "source", "target"]);
    insta::assert_snapshot!(output, @r#"
    ------- stderr -------
    Fetching into new repo in "$TEST_ENV/target"
    Nothing changed.
    [EOF]
    "#);
    let target_dir = test_env.work_dir("target");

    let source_log = create_colocated_repo_and_bookmarks_from_trunk1(&source_dir);
    insta::assert_snapshot!(source_log, @r#"
       ===== Source git repo contents =====
    @  bc83465a3090 "b" b
    │ ○  d4d535f1d579 "a2" a2
    ├─╯
    │ ○  c8303692b8e2 "a1" a1
    ├─╯
    ○  382881770501 "trunk1" trunk1
    ◆  000000000000 ""
    [EOF]
    "#);

    // Test an error message
    let output = target_dir.run_jj(["git", "fetch", "--branch", "glob:^:a*"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Error: Invalid branch pattern provided. When fetching, branch names and globs may not contain the characters `:`, `^`, `?`, `[`, `]`
    [EOF]
    [exit status: 1]
    ");
    let output = target_dir.run_jj(["git", "fetch", "--branch", "a*"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Error: Branch names may not include `*`.
    Hint: Prefix the pattern with `glob:` to expand `*` as a glob
    [EOF]
    [exit status: 1]
    ");

    // Nothing in our repo before the fetch
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    ◆  000000000000 ""
    [EOF]
    "#);
    // Fetch one bookmark...
    let output = target_dir.run_jj(["git", "fetch", "--branch", "b"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: b@origin [new] tracked
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  bc83465a3090 "b" b
    │ ○  382881770501 "trunk1"
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
    // ...check what the intermediate state looks like...
    insta::assert_snapshot!(get_bookmark_output(&target_dir), @r"
    b: yostqsxw bc83465a b
      @origin: yostqsxw bc83465a b
    [EOF]
    ");
    // ...then fetch two others with a glob.
    let output = target_dir.run_jj(["git", "fetch", "--branch", "glob:a*"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: a1@origin [new] tracked
    bookmark: a2@origin [new] tracked
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  d4d535f1d579 "a2" a2
    │ │ ○  c8303692b8e2 "a1" a1
    │ ├─╯
    │ │ ○  bc83465a3090 "b" b
    │ ├─╯
    │ ○  382881770501 "trunk1"
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
    // Fetching the same bookmark again
    let output = target_dir.run_jj(["git", "fetch", "--branch", "a1"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Nothing changed.
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  d4d535f1d579 "a2" a2
    │ │ ○  c8303692b8e2 "a1" a1
    │ ├─╯
    │ │ ○  bc83465a3090 "b" b
    │ ├─╯
    │ ○  382881770501 "trunk1"
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);

    // ==== Change both repos ====
    // First, change the target repo:
    let source_log = create_trunk2_and_rebase_bookmarks(&source_dir);
    insta::assert_snapshot!(source_log, @r#"
       ===== Source git repo contents =====
    ○  2b30dbc93959 "b" b
    │ ○  841140b152fc "a2" a2
    ├─╯
    │ ○  bc7e74c21d43 "a1" a1
    ├─╯
    @  756be1d31c41 "trunk2" trunk2
    ○  382881770501 "trunk1" trunk1
    ◆  000000000000 ""
    [EOF]
    "#);
    // Change a bookmark in the source repo as well, so that it becomes conflicted.
    target_dir
        .run_jj(["describe", "b", "-m=new_descr_for_b_to_create_conflict"])
        .success();

    // Our repo before and after fetch of two bookmarks
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  c62db3119722 "new_descr_for_b_to_create_conflict" b*
    │ │ ○  d4d535f1d579 "a2" a2
    │ ├─╯
    │ │ ○  c8303692b8e2 "a1" a1
    │ ├─╯
    │ ○  382881770501 "trunk1"
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
    let output = target_dir.run_jj(["git", "fetch", "--branch", "b", "--branch", "a1"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: a1@origin [updated] tracked
    bookmark: b@origin  [updated] tracked
    Abandoned 1 commits that are no longer reachable.
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  2b30dbc93959 "b" b?? b@origin
    │ │ ○  bc7e74c21d43 "a1" a1
    │ ├─╯
    │ ○  756be1d31c41 "trunk2"
    │ │ ○  c62db3119722 "new_descr_for_b_to_create_conflict" b??
    │ ├─╯
    │ │ ○  d4d535f1d579 "a2" a2
    │ ├─╯
    │ ○  382881770501 "trunk1"
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);

    // We left a2 where it was before, let's see how `jj bookmark list` sees this.
    insta::assert_snapshot!(get_bookmark_output(&target_dir), @r"
    a1: mzvwutvl bc7e74c2 a1
      @origin: mzvwutvl bc7e74c2 a1
    a2: yqosqzyt d4d535f1 a2
      @origin: yqosqzyt d4d535f1 a2
    b (conflicted):
      - yostqsxw hidden bc83465a b
      + yostqsxw?? c62db311 new_descr_for_b_to_create_conflict
      + yostqsxw?? 2b30dbc9 b
      @origin (behind by 1 commits): yostqsxw?? 2b30dbc9 b
    [EOF]
    ");
    // Now, let's fetch a2 and double-check that fetching a1 and b again doesn't do
    // anything.
    let output = target_dir.run_jj(["git", "fetch", "--branch", "b", "--branch", "glob:a*"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: a2@origin [updated] tracked
    Abandoned 1 commits that are no longer reachable.
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  841140b152fc "a2" a2
    │ │ ○  2b30dbc93959 "b" b?? b@origin
    │ ├─╯
    │ │ ○  bc7e74c21d43 "a1" a1
    │ ├─╯
    │ ○  756be1d31c41 "trunk2"
    │ │ ○  c62db3119722 "new_descr_for_b_to_create_conflict" b??
    │ ├─╯
    │ ○  382881770501 "trunk1"
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
    insta::assert_snapshot!(get_bookmark_output(&target_dir), @r"
    a1: mzvwutvl bc7e74c2 a1
      @origin: mzvwutvl bc7e74c2 a1
    a2: yqosqzyt 841140b1 a2
      @origin: yqosqzyt 841140b1 a2
    b (conflicted):
      - yostqsxw hidden bc83465a b
      + yostqsxw?? c62db311 new_descr_for_b_to_create_conflict
      + yostqsxw?? 2b30dbc9 b
      @origin (behind by 1 commits): yostqsxw?? 2b30dbc9 b
    [EOF]
    ");
}

#[test]
fn test_git_fetch_bookmarks_some_missing() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "origin");
    add_git_remote(&test_env, &work_dir, "rem1");
    add_git_remote(&test_env, &work_dir, "rem2");
    add_git_remote(&test_env, &work_dir, "rem3");

    // single missing bookmark, implicit remotes (@origin)
    let output = work_dir.run_jj(["git", "fetch", "--branch", "noexist"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Warning: No branch matching `noexist` found on any specified/configured remote
    Nothing changed.
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @"");

    // multiple missing bookmarks, implicit remotes (@origin)
    let output = work_dir.run_jj([
        "git", "fetch", "--branch", "noexist1", "--branch", "noexist2",
    ]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Warning: No branch matching `noexist1`, `noexist2` found on any specified/configured remote
    Nothing changed.
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @"");

    // single existing bookmark, implicit remotes (@origin)
    let output = work_dir.run_jj(["git", "fetch", "--branch", "origin"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: origin@origin [new] tracked
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin: qmyrypzk ab8b299e message
      @origin: qmyrypzk ab8b299e message
    [EOF]
    ");

    // multiple existing bookmark, explicit remotes, each bookmark is only in one
    // remote.
    let output = work_dir.run_jj([
        "git", "fetch", "--branch", "rem1", "--branch", "rem2", "--branch", "rem3", "--remote",
        "rem1", "--remote", "rem2", "--remote", "rem3",
    ]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: rem1@rem1 [new] tracked
    bookmark: rem2@rem2 [new] tracked
    bookmark: rem3@rem3 [new] tracked
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin: qmyrypzk ab8b299e message
      @origin: qmyrypzk ab8b299e message
    rem1: ppspxspk 4acd0343 message
      @rem1: ppspxspk 4acd0343 message
    rem2: pzqqpnpo 44c57802 message
      @rem2: pzqqpnpo 44c57802 message
    rem3: wrzwlmys 45a3faef message
      @rem3: wrzwlmys 45a3faef message
    [EOF]
    ");

    // multiple bookmarks, one exists, one doesn't
    let output = work_dir.run_jj([
        "git", "fetch", "--branch", "rem1", "--branch", "notexist", "--remote", "rem1",
    ]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Warning: No branch matching `notexist` found on any specified/configured remote
    Nothing changed.
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin: qmyrypzk ab8b299e message
      @origin: qmyrypzk ab8b299e message
    rem1: ppspxspk 4acd0343 message
      @rem1: ppspxspk 4acd0343 message
    rem2: pzqqpnpo 44c57802 message
      @rem2: pzqqpnpo 44c57802 message
    rem3: wrzwlmys 45a3faef message
      @rem3: wrzwlmys 45a3faef message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_bookmarks_missing_with_subprocess_localized_message() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "origin");

    // "fatal: couldn't find remote ref %s" shouldn't be localized.
    let output = work_dir.run_jj_with(|cmd| {
        cmd.args(["git", "fetch", "--branch=unknown"])
            // Initialize locale as "en_US" which is the most common.
            .env("LC_ALL", "en_US.UTF-8")
            // Set some other locale variables for testing.
            .env("LC_MESSAGES", "en_US.UTF-8")
            .env("LANG", "en_US.UTF-8")
            // GNU gettext prioritizes LANGUAGE if translation is enabled. It works
            // no matter if system locale exists or not.
            .env("LANGUAGE", "zh_TW")
    });
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Warning: No branch matching `unknown` found on any specified/configured remote
    Nothing changed.
    [EOF]
    ");
}

// See `test_undo_restore_commands.rs` for fetch-undo-push and fetch-undo-fetch
// of the same bookmarks for various kinds of undo.
#[test]
fn test_git_fetch_undo() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    let source_dir = test_env.work_dir("source");
    git::init(source_dir.root());

    // Clone an empty repo. The target repo is a normal `jj` repo, *not* colocated
    let output = test_env.run_jj_in(".", ["git", "clone", "source", "target"]);
    insta::assert_snapshot!(output, @r#"
    ------- stderr -------
    Fetching into new repo in "$TEST_ENV/target"
    Nothing changed.
    [EOF]
    "#);
    let target_dir = test_env.work_dir("target");

    let source_log = create_colocated_repo_and_bookmarks_from_trunk1(&source_dir);
    insta::assert_snapshot!(source_log, @r#"
       ===== Source git repo contents =====
    @  bc83465a3090 "b" b
    │ ○  d4d535f1d579 "a2" a2
    ├─╯
    │ ○  c8303692b8e2 "a1" a1
    ├─╯
    ○  382881770501 "trunk1" trunk1
    ◆  000000000000 ""
    [EOF]
    "#);

    // Fetch 2 bookmarks
    let output = target_dir.run_jj(["git", "fetch", "--branch", "b", "--branch", "a1"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: a1@origin [new] tracked
    bookmark: b@origin  [new] tracked
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  bc83465a3090 "b" b
    │ │ ○  c8303692b8e2 "a1" a1
    │ ├─╯
    │ ○  382881770501 "trunk1"
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
    let output = target_dir.run_jj(["undo"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Undid operation: 158b589e0e15 (2001-02-03 08:05:18) fetch from git remote(s) origin
    [EOF]
    ");
    // The undo works as expected
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    ◆  000000000000 ""
    [EOF]
    "#);
    // Now try to fetch just one bookmark
    let output = target_dir.run_jj(["git", "fetch", "--branch", "b"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: b@origin [new] tracked
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  bc83465a3090 "b" b
    │ ○  382881770501 "trunk1"
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
}

// Compare to `test_git_import_undo` in test_git_import_export
// TODO: Explain why these behaviors are useful
#[test]
fn test_fetch_undo_what() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    let source_dir = test_env.work_dir("source");
    git::init(source_dir.root());

    // Clone an empty repo. The target repo is a normal `jj` repo, *not* colocated
    let output = test_env.run_jj_in(".", ["git", "clone", "source", "target"]);
    insta::assert_snapshot!(output, @r#"
    ------- stderr -------
    Fetching into new repo in "$TEST_ENV/target"
    Nothing changed.
    [EOF]
    "#);
    let work_dir = test_env.work_dir("target");

    let source_log = create_colocated_repo_and_bookmarks_from_trunk1(&source_dir);
    insta::assert_snapshot!(source_log, @r#"
       ===== Source git repo contents =====
    @  bc83465a3090 "b" b
    │ ○  d4d535f1d579 "a2" a2
    ├─╯
    │ ○  c8303692b8e2 "a1" a1
    ├─╯
    ○  382881770501 "trunk1" trunk1
    ◆  000000000000 ""
    [EOF]
    "#);

    // Initial state we will try to return to after `op restore`. There are no
    // bookmarks.
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @"");
    let base_operation_id = work_dir.current_operation_id();

    // Fetch a bookmark
    let output = work_dir.run_jj(["git", "fetch", "--branch", "b"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: b@origin [new] tracked
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  bc83465a3090 "b" b
    │ ○  382881770501 "trunk1"
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    b: yostqsxw bc83465a b
      @origin: yostqsxw bc83465a b
    [EOF]
    ");

    // We can undo the change in the repo without moving the remote-tracking
    // bookmark
    let output = work_dir.run_jj(["op", "restore", "--what", "repo", &base_operation_id]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Restored to operation: 8f47435a3990 (2001-02-03 08:05:07) add workspace 'default'
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    b (deleted)
      @origin: yostqsxw hidden bc83465a b
    [EOF]
    ");

    // Now, let's demo restoring just the remote-tracking bookmark. First, let's
    // change our local repo state...
    work_dir
        .run_jj(["bookmark", "c", "-r@", "newbookmark"])
        .success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    b (deleted)
      @origin: yostqsxw hidden bc83465a b
    newbookmark: qpvuntsm e8849ae1 (empty) (no description set)
    [EOF]
    ");
    // Restoring just the remote-tracking state will not affect `newbookmark`, but
    // will eliminate `b@origin`.
    let output = work_dir.run_jj([
        "op",
        "restore",
        "--what",
        "remote-tracking",
        &base_operation_id,
    ]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Restored to operation: 8f47435a3990 (2001-02-03 08:05:07) add workspace 'default'
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    newbookmark: qpvuntsm e8849ae1 (empty) (no description set)
    [EOF]
    ");
}

#[test]
fn test_git_fetch_remove_fetch() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "origin");

    work_dir
        .run_jj(["bookmark", "create", "-r@", "origin"])
        .success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin: qpvuntsm e8849ae1 (empty) (no description set)
    [EOF]
    ");

    work_dir.run_jj(["git", "fetch"]).success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin (conflicted):
      + qpvuntsm e8849ae1 (empty) (no description set)
      + qmyrypzk ab8b299e message
      @origin (behind by 1 commits): qmyrypzk ab8b299e message
    [EOF]
    ");

    work_dir
        .run_jj(["git", "remote", "remove", "origin"])
        .success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin (conflicted):
      + qpvuntsm e8849ae1 (empty) (no description set)
      + qmyrypzk ab8b299e message
    [EOF]
    ");

    work_dir
        .run_jj(["git", "remote", "add", "origin", "../origin"])
        .success();

    // Check that origin@origin is properly recreated
    let output = work_dir.run_jj(["git", "fetch"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: origin@origin [new] tracked
    [EOF]
    ");
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin (conflicted):
      + qpvuntsm e8849ae1 (empty) (no description set)
      + qmyrypzk ab8b299e message
      @origin (behind by 1 commits): qmyrypzk ab8b299e message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_rename_fetch() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    add_git_remote(&test_env, &work_dir, "origin");

    work_dir
        .run_jj(["bookmark", "create", "-r@", "origin"])
        .success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin: qpvuntsm e8849ae1 (empty) (no description set)
    [EOF]
    ");

    work_dir.run_jj(["git", "fetch"]).success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin (conflicted):
      + qpvuntsm e8849ae1 (empty) (no description set)
      + qmyrypzk ab8b299e message
      @origin (behind by 1 commits): qmyrypzk ab8b299e message
    [EOF]
    ");

    work_dir
        .run_jj(["git", "remote", "rename", "origin", "upstream"])
        .success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    origin (conflicted):
      + qpvuntsm e8849ae1 (empty) (no description set)
      + qmyrypzk ab8b299e message
      @upstream (behind by 1 commits): qmyrypzk ab8b299e message
    [EOF]
    ");

    // Check that jj indicates that nothing has changed
    let output = work_dir.run_jj(["git", "fetch", "--remote", "upstream"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Nothing changed.
    [EOF]
    ");
}

#[test]
fn test_git_fetch_removed_bookmark() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    let source_dir = test_env.work_dir("source");
    git::init(source_dir.root());

    // Clone an empty repo. The target repo is a normal `jj` repo, *not* colocated
    let output = test_env.run_jj_in(".", ["git", "clone", "source", "target"]);
    insta::assert_snapshot!(output, @r#"
    ------- stderr -------
    Fetching into new repo in "$TEST_ENV/target"
    Nothing changed.
    [EOF]
    "#);
    let target_dir = test_env.work_dir("target");

    let source_log = create_colocated_repo_and_bookmarks_from_trunk1(&source_dir);
    insta::assert_snapshot!(source_log, @r#"
       ===== Source git repo contents =====
    @  bc83465a3090 "b" b
    │ ○  d4d535f1d579 "a2" a2
    ├─╯
    │ ○  c8303692b8e2 "a1" a1
    ├─╯
    ○  382881770501 "trunk1" trunk1
    ◆  000000000000 ""
    [EOF]
    "#);

    // Fetch all bookmarks
    let output = target_dir.run_jj(["git", "fetch"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: a1@origin     [new] tracked
    bookmark: a2@origin     [new] tracked
    bookmark: b@origin      [new] tracked
    bookmark: trunk1@origin [new] tracked
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  bc83465a3090 "b" b
    │ │ ○  d4d535f1d579 "a2" a2
    │ ├─╯
    │ │ ○  c8303692b8e2 "a1" a1
    │ ├─╯
    │ ○  382881770501 "trunk1" trunk1
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);

    // Remove a2 bookmark in origin
    source_dir
        .run_jj(["bookmark", "forget", "--include-remotes", "a2"])
        .success();

    // Fetch bookmark a1 from origin and check that a2 is still there
    let output = target_dir.run_jj(["git", "fetch", "--branch", "a1"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Nothing changed.
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  bc83465a3090 "b" b
    │ │ ○  d4d535f1d579 "a2" a2
    │ ├─╯
    │ │ ○  c8303692b8e2 "a1" a1
    │ ├─╯
    │ ○  382881770501 "trunk1" trunk1
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);

    // Fetch bookmarks a2 from origin, and check that it has been removed locally
    let output = target_dir.run_jj(["git", "fetch", "--branch", "a2"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: a2@origin [deleted] untracked
    Abandoned 1 commits that are no longer reachable.
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  bc83465a3090 "b" b
    │ │ ○  c8303692b8e2 "a1" a1
    │ ├─╯
    │ ○  382881770501 "trunk1" trunk1
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
}

#[test]
fn test_git_fetch_removed_parent_bookmark() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    let source_dir = test_env.work_dir("source");
    git::init(source_dir.root());

    // Clone an empty repo. The target repo is a normal `jj` repo, *not* colocated
    let output = test_env.run_jj_in(".", ["git", "clone", "source", "target"]);
    insta::assert_snapshot!(output, @r#"
    ------- stderr -------
    Fetching into new repo in "$TEST_ENV/target"
    Nothing changed.
    [EOF]
    "#);
    let target_dir = test_env.work_dir("target");

    let source_log = create_colocated_repo_and_bookmarks_from_trunk1(&source_dir);
    insta::assert_snapshot!(source_log, @r#"
       ===== Source git repo contents =====
    @  bc83465a3090 "b" b
    │ ○  d4d535f1d579 "a2" a2
    ├─╯
    │ ○  c8303692b8e2 "a1" a1
    ├─╯
    ○  382881770501 "trunk1" trunk1
    ◆  000000000000 ""
    [EOF]
    "#);

    // Fetch all bookmarks
    let output = target_dir.run_jj(["git", "fetch"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: a1@origin     [new] tracked
    bookmark: a2@origin     [new] tracked
    bookmark: b@origin      [new] tracked
    bookmark: trunk1@origin [new] tracked
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  bc83465a3090 "b" b
    │ │ ○  d4d535f1d579 "a2" a2
    │ ├─╯
    │ │ ○  c8303692b8e2 "a1" a1
    │ ├─╯
    │ ○  382881770501 "trunk1" trunk1
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);

    // Remove all bookmarks in origin.
    source_dir
        .run_jj(["bookmark", "forget", "--include-remotes", "glob:*"])
        .success();

    // Fetch bookmarks master, trunk1 and a1 from origin and check that only those
    // bookmarks have been removed and that others were not rebased because of
    // abandoned commits.
    let output = target_dir.run_jj([
        "git", "fetch", "--branch", "master", "--branch", "trunk1", "--branch", "a1",
    ]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    bookmark: a1@origin     [deleted] untracked
    bookmark: trunk1@origin [deleted] untracked
    Abandoned 1 commits that are no longer reachable.
    Warning: No branch matching `master` found on any specified/configured remote
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&target_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  bc83465a3090 "b" b
    │ │ ○  d4d535f1d579 "a2" a2
    │ ├─╯
    │ ○  382881770501 "trunk1"
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
}

#[test]
fn test_git_fetch_remote_only_bookmark() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    // Create non-empty git repo to add as a remote
    let git_repo_path = test_env.env_root().join("git-repo");
    let git_repo = git::init(git_repo_path);
    work_dir
        .run_jj(["git", "remote", "add", "origin", "../git-repo"])
        .success();

    // Create a commit and a bookmark in the git repo
    let commit_result = git::add_commit(
        &git_repo,
        "refs/heads/feature1",
        "file",
        b"content",
        "message",
        &[],
    );

    // Fetch using git.auto_local_bookmark = true
    test_env.add_config("git.auto-local-bookmark = true");
    work_dir
        .run_jj(["git", "fetch", "--remote=origin"])
        .success();
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    feature1: qomsplrm ebeb70d8 message
      @origin: qomsplrm ebeb70d8 message
    [EOF]
    ");

    git::write_commit(
        &git_repo,
        "refs/heads/feature2",
        commit_result.tree_id,
        "message",
        &[],
    );

    // Fetch using git.auto_local_bookmark = false
    test_env.add_config("git.auto-local-bookmark = false");
    work_dir
        .run_jj(["git", "fetch", "--remote=origin"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r#"
    @  e8849ae12c70 ""
    │ ◆  ebeb70d8c5f9 "message" feature1 feature2@origin
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    feature1: qomsplrm ebeb70d8 message
      @origin: qomsplrm ebeb70d8 message
    feature2@origin: qomsplrm ebeb70d8 message
    [EOF]
    ");
}

#[test]
fn test_git_fetch_preserve_commits_across_repos() {
    let test_env = TestEnvironment::default();
    test_env.add_config("git.auto-local-bookmark = true");
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    let upstream_repo = add_git_remote(&test_env, &work_dir, "upstream");

    let fork_path = test_env.env_root().join("fork");
    let fork_repo = clone_git_remote_into(&test_env, "upstream", "fork");
    work_dir
        .run_jj(["git", "remote", "add", "fork", "../fork"])
        .success();

    // add commit to fork remote in another branch
    add_commit_to_branch(&fork_repo, "feature");

    // fetch remote bookmarks
    work_dir
        .run_jj(["git", "fetch", "--remote=fork", "--remote=upstream"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r#"
    @  e8849ae12c70 ""
    │ ○  bcd7cd779791 "message" upstream
    ├─╯
    │ ○  16ec9ef2877a "message" feature
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    feature: srwrtuky 16ec9ef2 message
      @fork: srwrtuky 16ec9ef2 message
    upstream: zkvzklqn bcd7cd77 message
      @fork: zkvzklqn bcd7cd77 message
      @upstream: zkvzklqn bcd7cd77 message
    [EOF]
    ");

    // merge fork/feature into the upstream/upstream
    git::add_remote(upstream_repo.git_dir(), "fork", fork_path.to_str().unwrap());
    git::fetch(upstream_repo.git_dir(), "fork");

    let base_id = upstream_repo
        .find_reference("refs/heads/upstream")
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .id()
        .detach();

    let fork_id = upstream_repo
        .find_reference("refs/remotes/fork/feature")
        .unwrap()
        .peel_to_commit()
        .unwrap()
        .id()
        .detach();

    git::write_commit(
        &upstream_repo,
        "refs/heads/upstream",
        upstream_repo.empty_tree().id().detach(),
        "merge",
        &[base_id, fork_id],
    );

    // remove branch on the fork
    fork_repo
        .find_reference("refs/heads/feature")
        .unwrap()
        .delete()
        .unwrap();

    // fetch again on the jj repo, first looking at fork and then at upstream
    work_dir
        .run_jj(["git", "fetch", "--remote=fork", "--remote=upstream"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r#"
    @  e8849ae12c70 ""
    │ ○    f3e9250bd003 "merge" upstream*
    │ ├─╮
    │ │ ○  16ec9ef2877a "message"
    ├───╯
    │ ○  bcd7cd779791 "message" upstream@fork
    ├─╯
    ◆  000000000000 ""
    [EOF]
    "#);
    insta::assert_snapshot!(get_bookmark_output(&work_dir), @r"
    upstream: trrkvuqr f3e9250b merge
      @fork (behind by 2 commits): zkvzklqn bcd7cd77 message
      @upstream: trrkvuqr f3e9250b merge
    [EOF]
    ");
}

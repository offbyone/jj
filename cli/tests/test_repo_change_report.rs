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

use crate::common::TestEnvironment;

#[test]
fn test_report_conflicts() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    work_dir.write_file("file", "A\n");
    work_dir.run_jj(["commit", "-m=A"]).success();
    work_dir.write_file("file", "B\n");
    work_dir.run_jj(["commit", "-m=B"]).success();
    work_dir.write_file("file", "C\n");
    work_dir.run_jj(["commit", "-m=C"]).success();

    let output = work_dir.run_jj(["rebase", "-s=description(B)", "-d=root()"]);
    insta::assert_snapshot!(output, @r###"
    ------- stderr -------
    Rebased 3 commits to destination
    Working copy  (@) now at: zsuskuln dd37d4a5 (conflict) (empty) (no description set)
    Parent commit (@-)      : kkmpptxz c7f5d6e5 (conflict) C
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file    2-sided conflict including 1 deletion
    New conflicts appeared in 2 commits:
      kkmpptxz c7f5d6e5 (conflict) C
      rlvkpnrz 032a8668 (conflict) B
    Hint: To resolve the conflicts, start by creating a commit on top of
    the first conflicted commit:
      jj new rlvkpnrz
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you can inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    "###);

    let output = work_dir.run_jj(["rebase", "-d=description(A)"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 3 commits to destination
    Working copy  (@) now at: zsuskuln bad741db (empty) (no description set)
    Parent commit (@-)      : kkmpptxz cec3d034 C
    Added 0 files, modified 1 files, removed 0 files
    Existing conflicts were resolved or abandoned from 2 commits.
    [EOF]
    ");

    // Can get hint about multiple root commits
    let output = work_dir.run_jj(["rebase", "-r=description(B)", "-d=root()"]);
    insta::assert_snapshot!(output, @r###"
    ------- stderr -------
    Rebased 1 commits to destination
    Rebased 2 descendant commits
    Working copy  (@) now at: zsuskuln 5b511768 (conflict) (empty) (no description set)
    Parent commit (@-)      : kkmpptxz 0252a7f0 (conflict) C
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file    2-sided conflict
    New conflicts appeared in 2 commits:
      kkmpptxz 0252a7f0 (conflict) C
      rlvkpnrz fcfd7304 (conflict) B
    Hint: To resolve the conflicts, start by creating a commit on top of
    one of the first conflicted commits:
      jj new kkmpptxz
      jj new rlvkpnrz
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you can inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    "###);

    // Resolve one of the conflicts by (mostly) following the instructions
    let output = work_dir.run_jj(["new", "rlvkpnrzqnoo"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: vruxwmqv 55514f4e (conflict) (empty) (no description set)
    Parent commit (@-)      : rlvkpnrz fcfd7304 (conflict) B
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file    2-sided conflict including 1 deletion
    [EOF]
    ");
    work_dir.write_file("file", "resolved\n");
    let output = work_dir.run_jj(["squash"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: yostqsxw 350a6e50 (empty) (no description set)
    Parent commit (@-)      : rlvkpnrz 1aa1004b B
    Existing conflicts were resolved or abandoned from 1 commits.
    [EOF]
    ");
}

#[test]
fn test_report_conflicts_with_divergent_commits() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    work_dir.run_jj(["describe", "-m=A"]).success();
    work_dir.write_file("file", "A\n");
    work_dir.run_jj(["new", "-m=B"]).success();
    work_dir.write_file("file", "B\n");
    work_dir.run_jj(["new", "-m=C"]).success();
    work_dir.write_file("file", "C\n");
    work_dir.run_jj(["describe", "-m=C2"]).success();
    work_dir
        .run_jj(["describe", "-m=C3", "--at-op=@-"])
        .success();

    let output = work_dir.run_jj(["rebase", "-s=description(B)", "-d=root()"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Concurrent modification detected, resolving automatically.
    Rebased 3 commits to destination
    Working copy  (@) now at: zsuskuln?1 08a31f4f (conflict) C2
    Parent commit (@-)      : kkmpptxz 099d6624 (conflict) B
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file    2-sided conflict including 1 deletion
    New conflicts appeared in 3 commits:
      zsuskuln?0 df34134a (conflict) C3
      zsuskuln?1 08a31f4f (conflict) C2
      kkmpptxz 099d6624 (conflict) B
    Hint: To resolve the conflicts, start by creating a commit on top of
    the first conflicted commit:
      jj new kkmpptxz
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you can inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");

    let output = work_dir.run_jj(["rebase", "-d=description(A)"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 3 commits to destination
    Working copy  (@) now at: zsuskuln?1 27ef05d9 C2
    Parent commit (@-)      : kkmpptxz 9039ed49 B
    Added 0 files, modified 1 files, removed 0 files
    Existing conflicts were resolved or abandoned from 3 commits.
    [EOF]
    ");

    // Same thing when rebasing the divergent commits one at a time
    let output = work_dir.run_jj(["rebase", "-s=description(C2)", "-d=root()"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 1 commits to destination
    Working copy  (@) now at: zsuskuln?0 dfe73891 (conflict) C2
    Parent commit (@-)      : zzzzzzzz 00000000 (empty) (no description set)
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file    2-sided conflict including 1 deletion
    New conflicts appeared in 1 commits:
      zsuskuln?0 dfe73891 (conflict) C2
    Hint: To resolve the conflicts, start by creating a commit on top of
    the conflicted commit:
      jj new zsuskuln
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you can inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");

    let output = work_dir.run_jj(["rebase", "-s=description(C3)", "-d=root()"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 1 commits to destination
    New conflicts appeared in 1 commits:
      zsuskuln?0 02834578 (conflict) C3
    Hint: To resolve the conflicts, start by creating a commit on top of
    the conflicted commit:
      jj new zsuskuln
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you can inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    ");

    let output = work_dir.run_jj(["rebase", "-s=description(C2)", "-d=description(B)"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 1 commits to destination
    Working copy  (@) now at: zsuskuln?0 3fcf2fd2 C2
    Parent commit (@-)      : kkmpptxz 9039ed49 B
    Added 0 files, modified 1 files, removed 0 files
    Existing conflicts were resolved or abandoned from 1 commits.
    [EOF]
    ");

    let output = work_dir.run_jj(["rebase", "-s=description(C3)", "-d=description(B)"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 1 commits to destination
    Existing conflicts were resolved or abandoned from 1 commits.
    [EOF]
    ");
}

#[test]
fn test_report_conflicts_with_resolving_conflicts_hint_disabled() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    work_dir.write_file("file", "A\n");
    work_dir.run_jj(["commit", "-m=A"]).success();
    work_dir.write_file("file", "B\n");
    work_dir.run_jj(["commit", "-m=B"]).success();
    work_dir.write_file("file", "C\n");
    work_dir.run_jj(["commit", "-m=C"]).success();

    let output = work_dir.run_jj([
        "rebase",
        "-s=description(B)",
        "-d=root()",
        "--config=hints.resolving-conflicts=false",
    ]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 3 commits to destination
    Working copy  (@) now at: zsuskuln dd37d4a5 (conflict) (empty) (no description set)
    Parent commit (@-)      : kkmpptxz c7f5d6e5 (conflict) C
    Added 0 files, modified 1 files, removed 0 files
    Warning: There are unresolved conflicts at these paths:
    file    2-sided conflict including 1 deletion
    New conflicts appeared in 2 commits:
      kkmpptxz c7f5d6e5 (conflict) C
      rlvkpnrz 032a8668 (conflict) B
    [EOF]
    ");
}

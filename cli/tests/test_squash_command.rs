// Copyright 2022 The Jujutsu Authors
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

use std::path::PathBuf;

use crate::common::CommandOutput;
use crate::common::TestEnvironment;
use crate::common::TestWorkDir;

#[test]
fn test_squash() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    work_dir
        .run_jj(["bookmark", "create", "-r@", "a"])
        .success();
    work_dir.write_file("file1", "a\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "b"])
        .success();
    work_dir.write_file("file1", "b\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "c"])
        .success();
    work_dir.write_file("file1", "c\n");
    // Test the setup
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  22be6c4e01da c
    ○  75591b1896b4 b
    ○  e6086990958c a
    ◆  000000000000 (empty)
    [EOF]
    ");

    // Squashes the working copy into the parent by default
    let output = work_dir.run_jj(["squash"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: vruxwmqv 2cf02eb8 (empty) (no description set)
    Parent commit (@-)      : kkmpptxz 9422c8d6 b c | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  2cf02eb82d82 (empty)
    ○  9422c8d6f294 b c
    ○  e6086990958c a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file1"]);
    insta::assert_snapshot!(output, @r"
    c
    [EOF]
    ");

    // Can squash a given commit into its parent
    work_dir.run_jj(["undo"]).success();
    let output = work_dir.run_jj(["squash", "-r", "b"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 1 descendant commits
    Working copy  (@) now at: mzvwutvl 441a7a3a c | (no description set)
    Parent commit (@-)      : qpvuntsm 105931bf a b | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  441a7a3a17b0 c
    ○  105931bfedad a b
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file1", "-r", "b"]);
    insta::assert_snapshot!(output, @r"
    b
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file1"]);
    insta::assert_snapshot!(output, @r"
    c
    [EOF]
    ");

    // Cannot squash a merge commit (because it's unclear which parent it should go
    // into)
    work_dir.run_jj(["undo"]).success();
    work_dir.run_jj(["edit", "b"]).success();
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "d"])
        .success();
    work_dir.write_file("file2", "d\n");
    work_dir.run_jj(["new", "c", "d"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "e"])
        .success();
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @    e05d4caaf6ce e (empty)
    ├─╮
    │ ○  9bb7863cfc78 d
    ○ │  22be6c4e01da c
    ├─╯
    ○  75591b1896b4 b
    ○  e6086990958c a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["squash"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Error: Cannot squash merge commits without a specified destination
    Hint: Use `--into` to specify which parent to squash into
    [EOF]
    [exit status: 1]
    ");

    // Can squash into a merge commit
    work_dir.run_jj(["new", "e"]).success();
    work_dir.write_file("file1", "e\n");
    let output = work_dir.run_jj(["squash"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: xlzxqlsl 91a81249 (empty) (no description set)
    Parent commit (@-)      : nmzmmopx 9155baf5 e | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  91a81249954f (empty)
    ○    9155baf5ced1 e
    ├─╮
    │ ○  9bb7863cfc78 d
    ○ │  22be6c4e01da c
    ├─╯
    ○  75591b1896b4 b
    ○  e6086990958c a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file1", "-r", "e"]);
    insta::assert_snapshot!(output, @r"
    e
    [EOF]
    ");
}

#[test]
fn test_squash_partial() {
    let mut test_env = TestEnvironment::default();
    let edit_script = test_env.set_up_fake_diff_editor();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    work_dir
        .run_jj(["bookmark", "create", "-r@", "a"])
        .success();
    work_dir.write_file("file1", "a\n");
    work_dir.write_file("file2", "a\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "b"])
        .success();
    work_dir.write_file("file1", "b\n");
    work_dir.write_file("file2", "b\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "c"])
        .success();
    work_dir.write_file("file1", "c\n");
    work_dir.write_file("file2", "c\n");
    // Test the setup
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  87059ac9657b c
    ○  f2c9709f39e9 b
    ○  64ea60be8d77 a
    ◆  000000000000 (empty)
    [EOF]
    ");

    // If we don't make any changes in the diff-editor, the whole change is moved
    // into the parent
    std::fs::write(&edit_script, "dump JJ-INSTRUCTIONS instrs").unwrap();
    let output = work_dir.run_jj(["squash", "-r", "b", "-i"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 1 descendant commits
    Working copy  (@) now at: mzvwutvl 34484d82 c | (no description set)
    Parent commit (@-)      : qpvuntsm 3141e675 a b | (no description set)
    [EOF]
    ");

    insta::assert_snapshot!(
        std::fs::read_to_string(test_env.env_root().join("instrs")).unwrap(), @r"
    You are moving changes from: kkmpptxz f2c9709f b | (no description set)
    into commit: qpvuntsm 64ea60be a | (no description set)

    The left side of the diff shows the contents of the parent commit. The
    right side initially shows the contents of the commit you're moving
    changes from.

    Adjust the right side until the diff shows the changes you want to move
    to the destination. If you don't make any changes, then all the changes
    from the source will be moved into the destination.
    ");

    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  34484d825f47 c
    ○  3141e67514f6 a b
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file1", "-r", "a"]);
    insta::assert_snapshot!(output, @r"
    b
    [EOF]
    ");

    // Can squash only some changes in interactive mode
    work_dir.run_jj(["undo"]).success();
    std::fs::write(&edit_script, "reset file1").unwrap();
    let output = work_dir.run_jj(["squash", "-r", "b", "-i"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 2 descendant commits
    Working copy  (@) now at: mzvwutvl 37e1a0ef c | (no description set)
    Parent commit (@-)      : kkmpptxz b41e789d b | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  37e1a0ef57ff c
    ○  b41e789df71c b
    ○  3af17565155e a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file1", "-r", "a"]);
    insta::assert_snapshot!(output, @r"
    a
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file2", "-r", "a"]);
    insta::assert_snapshot!(output, @r"
    b
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file1", "-r", "b"]);
    insta::assert_snapshot!(output, @r"
    b
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file2", "-r", "b"]);
    insta::assert_snapshot!(output, @r"
    b
    [EOF]
    ");

    // Can squash only some changes in non-interactive mode
    work_dir.run_jj(["undo"]).success();
    // Clear the script so we know it won't be used even without -i
    std::fs::write(&edit_script, "").unwrap();
    let output = work_dir.run_jj(["squash", "-r", "b", "file2"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 2 descendant commits
    Working copy  (@) now at: mzvwutvl 72ff256c c | (no description set)
    Parent commit (@-)      : kkmpptxz dd056a92 b | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  72ff256cd290 c
    ○  dd056a925eb3 b
    ○  cf083f1d9ccf a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file1", "-r", "a"]);
    insta::assert_snapshot!(output, @r"
    a
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file2", "-r", "a"]);
    insta::assert_snapshot!(output, @r"
    b
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file1", "-r", "b"]);
    insta::assert_snapshot!(output, @r"
    b
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file2", "-r", "b"]);
    insta::assert_snapshot!(output, @r"
    b
    [EOF]
    ");

    // If we specify only a non-existent file, then nothing changes.
    work_dir.run_jj(["undo"]).success();
    let output = work_dir.run_jj(["squash", "-r", "b", "nonexistent"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Nothing changed.
    [EOF]
    ");

    // We get a warning if we pass a positional argument that looks like a revset
    work_dir.run_jj(["undo"]).success();
    let output = work_dir.run_jj(["squash", "b"]);
    insta::assert_snapshot!(output, @r#"
    ------- stderr -------
    Warning: The argument "b" is being interpreted as a fileset expression. To specify a revset, pass -r "b" instead.
    Nothing changed.
    [EOF]
    "#);

    // we can use --interactive and fileset together
    work_dir.run_jj(["undo"]).success();
    work_dir.write_file("file3", "foo\n");
    std::fs::write(&edit_script, "reset file1").unwrap();
    let output = work_dir.run_jj(["squash", "-i", "file1", "file3"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 1 descendant commits
    Working copy  (@) now at: mzvwutvl 69c58f86 c | (no description set)
    Parent commit (@-)      : kkmpptxz 0f38c564 b | (no description set)
    [EOF]
    ");
    let output = work_dir.run_jj(["log", "-s"]);
    insta::assert_snapshot!(output, @r"
    @  mzvwutvl test.user@example.com 2001-02-03 08:05:36 c 69c58f86
    │  (no description set)
    │  M file1
    │  M file2
    ○  kkmpptxz test.user@example.com 2001-02-03 08:05:36 b 0f38c564
    │  (no description set)
    │  M file1
    │  M file2
    │  A file3
    ○  qpvuntsm test.user@example.com 2001-02-03 08:05:09 a 64ea60be
    │  (no description set)
    │  A file1
    │  A file2
    ◆  zzzzzzzz root() 00000000
    [EOF]
    ");
}

#[test]
fn test_squash_keep_emptied() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    work_dir
        .run_jj(["bookmark", "create", "-r@", "a"])
        .success();
    work_dir.write_file("file1", "a\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "b"])
        .success();
    work_dir.write_file("file1", "b\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "c"])
        .success();
    work_dir.write_file("file1", "c\n");
    // Test the setup

    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  22be6c4e01da c
    ○  75591b1896b4 b
    ○  e6086990958c a
    ◆  000000000000 (empty)
    [EOF]
    ");

    let output = work_dir.run_jj(["squash", "-r", "b", "--keep-emptied"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 2 descendant commits
    Working copy  (@) now at: mzvwutvl 093590e0 c | (no description set)
    Parent commit (@-)      : kkmpptxz 357946cf b | (empty) (no description set)
    [EOF]
    ");
    // With --keep-emptied, b remains even though it is now empty.
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  093590e044bd c
    ○  357946cf85df b (empty)
    ○  2269fb3b12f5 a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file1", "-r", "a"]);
    insta::assert_snapshot!(output, @r"
    b
    [EOF]
    ");
}

#[test]
fn test_squash_from_to() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    // Create history like this:
    // F
    // |
    // E C
    // | |
    // D B
    // |/
    // A
    //
    // When moving changes between e.g. C and F, we should not get unrelated changes
    // from B and D.
    work_dir
        .run_jj(["bookmark", "create", "-r@", "a"])
        .success();
    work_dir.write_file("file1", "a\n");
    work_dir.write_file("file2", "a\n");
    work_dir.write_file("file3", "a\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "b"])
        .success();
    work_dir.write_file("file3", "b\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "c"])
        .success();
    work_dir.write_file("file1", "c\n");
    work_dir.run_jj(["edit", "a"]).success();
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "d"])
        .success();
    work_dir.write_file("file3", "d\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "e"])
        .success();
    work_dir.write_file("file2", "e\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "f"])
        .success();
    work_dir.write_file("file2", "f\n");
    // Test the setup
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  0fac1124d1ad f
    ○  4ebe104a0e4e e
    ○  dc71a460d5d6 d
    │ ○  ee0b260ffc44 c
    │ ○  e31bf988d7c9 b
    ├─╯
    ○  e3e04beaf7d3 a
    ◆  000000000000 (empty)
    [EOF]
    ");

    // No-op if source and destination are the same
    let output = work_dir.run_jj(["squash", "--into", "@"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Nothing changed.
    [EOF]
    ");

    // Can squash from sibling, which results in the source being abandoned
    let output = work_dir.run_jj(["squash", "--from", "c"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: kmkuslsw 941ab024 f | (no description set)
    Parent commit (@-)      : znkkpsqq 4ebe104a e | (no description set)
    Added 0 files, modified 1 files, removed 0 files
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  941ab024b3f8 f
    ○  4ebe104a0e4e e
    ○  dc71a460d5d6 d
    │ ○  e31bf988d7c9 b c
    ├─╯
    ○  e3e04beaf7d3 a
    ◆  000000000000 (empty)
    [EOF]
    ");
    // The change from the source has been applied
    let output = work_dir.run_jj(["file", "show", "file1"]);
    insta::assert_snapshot!(output, @r"
    c
    [EOF]
    ");
    // File `file2`, which was not changed in source, is unchanged
    let output = work_dir.run_jj(["file", "show", "file2"]);
    insta::assert_snapshot!(output, @r"
    f
    [EOF]
    ");

    // Can squash from ancestor
    work_dir.run_jj(["undo"]).success();
    let output = work_dir.run_jj(["squash", "--from", "@--"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: kmkuslsw c102d2c4 f | (no description set)
    Parent commit (@-)      : znkkpsqq beb7c033 e | (no description set)
    [EOF]
    ");
    // The change has been removed from the source (the change pointed to by 'd'
    // became empty and was abandoned)
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  c102d2c4e165 f
    ○  beb7c0338f7c e
    │ ○  ee0b260ffc44 c
    │ ○  e31bf988d7c9 b
    ├─╯
    ○  e3e04beaf7d3 a d
    ◆  000000000000 (empty)
    [EOF]
    ");
    // The change from the source has been applied (the file contents were already
    // "f", as is typically the case when moving changes from an ancestor)
    let output = work_dir.run_jj(["file", "show", "file2"]);
    insta::assert_snapshot!(output, @r"
    f
    [EOF]
    ");

    // Can squash from descendant
    work_dir.run_jj(["undo"]).success();
    let output = work_dir.run_jj(["squash", "--from", "e", "--into", "d"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 1 descendant commits
    Working copy  (@) now at: kmkuslsw 1bc21d4e f | (no description set)
    Parent commit (@-)      : vruxwmqv 8b6b080a d e | (no description set)
    [EOF]
    ");
    // The change has been removed from the source (the change pointed to by 'e'
    // became empty and was abandoned)
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  1bc21d4e92d6 f
    ○  8b6b080ab587 d e
    │ ○  ee0b260ffc44 c
    │ ○  e31bf988d7c9 b
    ├─╯
    ○  e3e04beaf7d3 a
    ◆  000000000000 (empty)
    [EOF]
    ");
    // The change from the source has been applied
    let output = work_dir.run_jj(["file", "show", "file2", "-r", "d"]);
    insta::assert_snapshot!(output, @r"
    e
    [EOF]
    ");

    // Can squash into the sources
    work_dir.run_jj(["undo"]).success();
    let output = work_dir.run_jj(["squash", "--from", "e::f", "--into", "d"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: pkstwlsy 76baa567 (empty) (no description set)
    Parent commit (@-)      : vruxwmqv 415e4069 d e f | (no description set)
    [EOF]
    ");
    // The change has been removed from the source (the change pointed to by 'e'
    // became empty and was abandoned)
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  76baa567ed0a (empty)
    ○  415e40694e88 d e f
    │ ○  ee0b260ffc44 c
    │ ○  e31bf988d7c9 b
    ├─╯
    ○  e3e04beaf7d3 a
    ◆  000000000000 (empty)
    [EOF]
    ");
    // The change from the source has been applied
    let output = work_dir.run_jj(["file", "show", "file2", "-r", "d"]);
    insta::assert_snapshot!(output, @r"
    f
    [EOF]
    ");
}

#[test]
fn test_squash_from_to_partial() {
    let mut test_env = TestEnvironment::default();
    let edit_script = test_env.set_up_fake_diff_editor();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    // Create history like this:
    //   C
    //   |
    // D B
    // |/
    // A
    work_dir
        .run_jj(["bookmark", "create", "-r@", "a"])
        .success();
    work_dir.write_file("file1", "a\n");
    work_dir.write_file("file2", "a\n");
    work_dir.write_file("file3", "a\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "b"])
        .success();
    work_dir.write_file("file3", "b\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "c"])
        .success();
    work_dir.write_file("file1", "c\n");
    work_dir.write_file("file2", "c\n");
    work_dir.run_jj(["edit", "a"]).success();
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "d"])
        .success();
    work_dir.write_file("file3", "d\n");
    // Test the setup
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  dc71a460d5d6 d
    │ ○  499d601f6046 c
    │ ○  e31bf988d7c9 b
    ├─╯
    ○  e3e04beaf7d3 a
    ◆  000000000000 (empty)
    [EOF]
    ");

    // If we don't make any changes in the diff-editor, the whole change is moved
    let output = work_dir.run_jj(["squash", "-i", "--from", "c"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: vruxwmqv 85589465 d | (no description set)
    Parent commit (@-)      : qpvuntsm e3e04bea a | (no description set)
    Added 0 files, modified 2 files, removed 0 files
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  85589465a5f7 d
    │ ○  e31bf988d7c9 b c
    ├─╯
    ○  e3e04beaf7d3 a
    ◆  000000000000 (empty)
    [EOF]
    ");
    // The changes from the source has been applied
    let output = work_dir.run_jj(["file", "show", "file1"]);
    insta::assert_snapshot!(output, @r"
    c
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "file2"]);
    insta::assert_snapshot!(output, @r"
    c
    [EOF]
    ");
    // File `file3`, which was not changed in source, is unchanged
    let output = work_dir.run_jj(["file", "show", "file3"]);
    insta::assert_snapshot!(output, @r"
    d
    [EOF]
    ");

    // Can squash only part of the change in interactive mode
    work_dir.run_jj(["undo"]).success();
    std::fs::write(&edit_script, "reset file2").unwrap();
    let output = work_dir.run_jj(["squash", "-i", "--from", "c"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: vruxwmqv 62bd5cd9 d | (no description set)
    Parent commit (@-)      : qpvuntsm e3e04bea a | (no description set)
    Added 0 files, modified 1 files, removed 0 files
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  62bd5cd9f413 d
    │ ○  2748f30463ed c
    │ ○  e31bf988d7c9 b
    ├─╯
    ○  e3e04beaf7d3 a
    ◆  000000000000 (empty)
    [EOF]
    ");
    // The selected change from the source has been applied
    let output = work_dir.run_jj(["file", "show", "file1"]);
    insta::assert_snapshot!(output, @r"
    c
    [EOF]
    ");
    // The unselected change from the source has not been applied
    let output = work_dir.run_jj(["file", "show", "file2"]);
    insta::assert_snapshot!(output, @r"
    a
    [EOF]
    ");
    // File `file3`, which was changed in source's parent, is unchanged
    let output = work_dir.run_jj(["file", "show", "file3"]);
    insta::assert_snapshot!(output, @r"
    d
    [EOF]
    ");

    // Can squash only part of the change from a sibling in non-interactive mode
    work_dir.run_jj(["undo"]).success();
    // Clear the script so we know it won't be used
    std::fs::write(&edit_script, "").unwrap();
    let output = work_dir.run_jj(["squash", "--from", "c", "file1"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: vruxwmqv 76bf6139 d | (no description set)
    Parent commit (@-)      : qpvuntsm e3e04bea a | (no description set)
    Added 0 files, modified 1 files, removed 0 files
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  76bf613928cf d
    │ ○  9d4418d4828e c
    │ ○  e31bf988d7c9 b
    ├─╯
    ○  e3e04beaf7d3 a
    ◆  000000000000 (empty)
    [EOF]
    ");
    // The selected change from the source has been applied
    let output = work_dir.run_jj(["file", "show", "file1"]);
    insta::assert_snapshot!(output, @r"
    c
    [EOF]
    ");
    // The unselected change from the source has not been applied
    let output = work_dir.run_jj(["file", "show", "file2"]);
    insta::assert_snapshot!(output, @r"
    a
    [EOF]
    ");
    // File `file3`, which was changed in source's parent, is unchanged
    let output = work_dir.run_jj(["file", "show", "file3"]);
    insta::assert_snapshot!(output, @r"
    d
    [EOF]
    ");

    // Can squash only part of the change from a descendant in non-interactive mode
    work_dir.run_jj(["undo"]).success();
    // Clear the script so we know it won't be used
    std::fs::write(&edit_script, "").unwrap();
    let output = work_dir.run_jj(["squash", "--from", "c", "--into", "b", "file1"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 1 descendant commits
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  dc71a460d5d6 d
    │ ○  f964ce4bca71 c
    │ ○  e12c895adba6 b
    ├─╯
    ○  e3e04beaf7d3 a
    ◆  000000000000 (empty)
    [EOF]
    ");
    // The selected change from the source has been applied
    let output = work_dir.run_jj(["file", "show", "file1", "-r", "b"]);
    insta::assert_snapshot!(output, @r"
    c
    [EOF]
    ");
    // The unselected change from the source has not been applied
    let output = work_dir.run_jj(["file", "show", "file2", "-r", "b"]);
    insta::assert_snapshot!(output, @r"
    a
    [EOF]
    ");

    // If we specify only a non-existent file, then nothing changes.
    work_dir.run_jj(["undo"]).success();
    let output = work_dir.run_jj(["squash", "--from", "c", "nonexistent"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Nothing changed.
    [EOF]
    ");
}

#[test]
fn test_squash_working_copy_restore_descendants() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    // Create history like this:
    //   Y
    //   |
    // B X@
    // |/
    // A
    //
    // Each commit adds a file named the same as the commit
    let create_commit = |name: &str| {
        work_dir
            .run_jj(["bookmark", "create", "-r@", name])
            .success();
        work_dir.write_file(name, format!("test {name}\n"));
    };

    create_commit("a");
    work_dir.run_jj(["new"]).success();
    create_commit("b");
    work_dir.run_jj(["new", "a"]).success();
    create_commit("x");
    work_dir.run_jj(["new"]).success();
    create_commit("y");
    work_dir.run_jj(["edit", "x"]).success();

    let template = r#"separate(
        " ",
        commit_id.short(),
        bookmarks,
        description,
        if(empty, "(empty)")
    )"#;
    let run_log = || work_dir.run_jj(["log", "-r=::", "--summary", "-T", template]);

    // Verify the setup
    insta::assert_snapshot!(run_log(), @r"
    ○  3f45d7a3ae69 y
    │  A y
    @  5b4046443e64 x
    │  A x
    │ ○  b1e1eea2f666 b
    ├─╯  A b
    ○  7468364c89fc a
    │  A a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=a"]);
    insta::assert_snapshot!(output, @r"
    a
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=b"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list"]);
    insta::assert_snapshot!(output, @r"
    a
    x
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=y"]);
    insta::assert_snapshot!(output, @r"
    a
    x
    y
    [EOF]
    ");

    let output = work_dir.run_jj(["squash", "--restore-descendants"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 2 descendant commits (while preserving their content)
    Working copy  (@) now at: kxryzmor 7ec5499d (empty) (no description set)
    Parent commit (@-)      : qpvuntsm 1c6a069e a x | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(run_log(), @r"
    @  7ec5499d9141 (empty)
    │ ○  ddfef0b279f8 y
    ├─╯  A y
    │ ○  640ba5e85507 b
    ├─╯  A b
    │    D x
    ○  1c6a069ec7e3 a x
    │  A a
    │  A x
    ◆  000000000000 (empty)
    [EOF]
    ");

    let output = work_dir.run_jj(["diff", "--summary"]);
    //  The current commit becomes empty.
    insta::assert_snapshot!(output, @"");
    // Should coincide with the working copy commit before
    let output = work_dir.run_jj(["file", "list", "-r=a"]);
    insta::assert_snapshot!(output, @r"
    a
    x
    [EOF]
    ");
    // Commit b should be the same as before
    let output = work_dir.run_jj(["file", "list", "-r=b"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=y"]);
    insta::assert_snapshot!(output, @r"
    a
    x
    y
    [EOF]
    ");
}

#[test]
fn test_squash_from_to_restore_descendants() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    // Create history like this:
    // F
    // |\
    // E C
    // | |
    // D B
    // |/
    // A
    //
    // Each commit adds a file named the same as the commit
    let create_commit = |name: &str| {
        work_dir
            .run_jj(["bookmark", "create", "-r@", name])
            .success();
        work_dir.write_file(name, format!("test {name}\n"));
    };

    create_commit("a");
    work_dir.run_jj(["new"]).success();
    create_commit("b");
    work_dir.run_jj(["new"]).success();
    create_commit("c");
    work_dir.run_jj(["new", "a"]).success();
    create_commit("d");
    work_dir.run_jj(["new"]).success();
    create_commit("e");
    work_dir.run_jj(["new", "e", "c"]).success();
    create_commit("f");

    let template = r#"separate(
        " ",
        commit_id.short(),
        bookmarks,
        description,
        if(empty, "(empty)")
    )"#;
    let run_log = || work_dir.run_jj(["log", "-r=::", "--summary", "-T", template]);

    // ========== Part 1 =========
    // Verify the setup
    insta::assert_snapshot!(run_log(), @r"
    @    42acd0537c88 f
    ├─╮  A f
    │ ○  4fb9706b0f47 c
    │ │  A c
    │ ○  b1e1eea2f666 b
    │ │  A b
    ○ │  b4e3197108ba e
    │ │  A e
    ○ │  d707102f499f d
    ├─╯  A d
    ○  7468364c89fc a
    │  A a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let beginning = work_dir.current_operation_id();
    test_env.advance_test_rng_seed_to_multiple_of(200_000);

    // Squash without --restore-descendants for comparison
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    let output = work_dir.run_jj(["squash", "--from=b", "--into=d"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 3 descendant commits
    Working copy  (@) now at: kpqxywon e462100a f | (no description set)
    Parent commit (@-)      : yostqsxw 6944fd03 e | (no description set)
    Parent commit (@-)      : mzvwutvl 6cd5d5c1 c | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(run_log(), @r"
    @    e462100ae7c3 f
    ├─╮  A f
    │ ○  6cd5d5c1daf7 c
    │ │  A c
    ○ │  6944fd03dc5d e
    │ │  A e
    ○ │  1befcf027d1b d
    ├─╯  A b
    │    A d
    ○  7468364c89fc a b
    │  A a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=d"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    d
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=c"]);
    insta::assert_snapshot!(output, @r"
    a
    c
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=e"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    d
    e
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=f"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    d
    e
    f
    [EOF]
    ");

    // --restore-descendants
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    let output = work_dir.run_jj(["squash", "--from=b", "--into=d", "--restore-descendants"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 3 descendant commits (while preserving their content)
    Working copy  (@) now at: kpqxywon 1d64ccbf f | (no description set)
    Parent commit (@-)      : yostqsxw cb90d752 e | (no description set)
    Parent commit (@-)      : mzvwutvl 4e6702ae c | (no description set)
    [EOF]
    ");
    //  `d`` becomes the same as in the above example,
    // but `c` does not lose file `b` and `e` still does not contain file `b`
    // regardless of what happened to their parents.
    insta::assert_snapshot!(run_log(), @r"
    @    1d64ccbf4608 f
    ├─╮  A f
    │ ○  4e6702ae494c c
    │ │  A b
    │ │  A c
    ○ │  cb90d75271b4 e
    │ │  D b
    │ │  A e
    ○ │  853ea07451aa d
    ├─╯  A b
    │    A d
    ○  7468364c89fc a b
    │  A a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=d"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    d
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=c"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=e"]);
    insta::assert_snapshot!(output, @r"
    a
    d
    e
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=f"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    d
    e
    f
    [EOF]
    ");

    // --restore-descendants works with --keep-emptied, same result except for
    // leaving an empty commit
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    let output = work_dir.run_jj([
        "squash",
        "--from=b",
        "--into=d",
        "--restore-descendants",
        "--keep-emptied",
    ]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 3 descendant commits (while preserving their content)
    Working copy  (@) now at: kpqxywon 3c13920f f | (no description set)
    Parent commit (@-)      : yostqsxw aa73012d e | (no description set)
    Parent commit (@-)      : mzvwutvl d323deaa c | (no description set)
    [EOF]
    ");
    //  `d`` becomes the same as in the above example,
    // but `c` does not lose file `b` and `e` still does not contain file `b`
    // regardless of what happened to their parents.
    insta::assert_snapshot!(run_log(), @r"
    @    3c13920f1e9a f
    ├─╮  A f
    │ ○  d323deaa04c2 c
    │ │  A b
    │ │  A c
    │ ○  a55451e8808f b (empty)
    ○ │  aa73012df9cd e
    │ │  D b
    │ │  A e
    ○ │  d00e73142243 d
    ├─╯  A b
    │    A d
    ○  7468364c89fc a
    │  A a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=d"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    d
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=c"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=e"]);
    insta::assert_snapshot!(output, @r"
    a
    d
    e
    [EOF]
    ");

    // ========== Part 2: Children and parents =========
    // Reminder of the setup
    test_env.advance_test_rng_seed_to_multiple_of(200_000);
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    insta::assert_snapshot!(run_log(), @r"
    @    42acd0537c88 f
    ├─╮  A f
    │ ○  4fb9706b0f47 c
    │ │  A c
    │ ○  b1e1eea2f666 b
    │ │  A b
    ○ │  b4e3197108ba e
    │ │  A e
    ○ │  d707102f499f d
    ├─╯  A d
    ○  7468364c89fc a
    │  A a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=c"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=d"]);
    insta::assert_snapshot!(output, @r"
    a
    d
    [EOF]
    ");

    // --restore-descendants works when squashing from parent to child
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    let output = work_dir.run_jj(["squash", "--from=a", "--into=b", "--restore-descendants"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 2 descendant commits (while preserving their content)
    Working copy  (@) now at: kpqxywon 7fa445c9 f | (no description set)
    Parent commit (@-)      : yostqsxw 102e6106 e | (no description set)
    Parent commit (@-)      : mzvwutvl a2ff7c27 c | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(run_log(), @r"
    @    7fa445c9e606 f
    ├─╮  A f
    │ ○  a2ff7c27dbba c
    │ │  A c
    │ ○  2bf81678391c b
    │ │  A a
    │ │  A b
    ○ │  102e61065eb2 e
    │ │  A e
    ○ │  7b1493a2027e d
    ├─╯  A a
    │    A d
    ◆  000000000000 a (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=b"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=c"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=d"]);
    insta::assert_snapshot!(output, @r"
    a
    d
    [EOF]
    ");

    // --restore-descendants works when squashing from child to parent
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    let output = work_dir.run_jj(["squash", "--from=b", "--into=a", "--restore-descendants"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 4 descendant commits (while preserving their content)
    Working copy  (@) now at: kpqxywon 84ae48a9 f | (no description set)
    Parent commit (@-)      : yostqsxw 1694571e e | (no description set)
    Parent commit (@-)      : mzvwutvl 3c5e64c8 c | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(run_log(), @r"
    @    84ae48a9284a f
    ├─╮  A b
    │ │  A f
    │ ○  3c5e64c855d6 c
    │ │  A c
    ○ │  1694571e2c54 e
    │ │  A e
    ○ │  3d8fac8b412a d
    ├─╯  D b
    │    A d
    ○  a2fbb6bd71cf a b
    │  A a
    │  A b
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=b"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=c"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=d"]);
    insta::assert_snapshot!(output, @r"
    a
    d
    [EOF]
    ");

    // ========== Part 3: Grandchildren and Grandparents =========
    // Reminder of the setup
    test_env.advance_test_rng_seed_to_multiple_of(200_000);
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    insta::assert_snapshot!(run_log(), @r"
    @    42acd0537c88 f
    ├─╮  A f
    │ ○  4fb9706b0f47 c
    │ │  A c
    │ ○  b1e1eea2f666 b
    │ │  A b
    ○ │  b4e3197108ba e
    │ │  A e
    ○ │  d707102f499f d
    ├─╯  A d
    ○  7468364c89fc a
    │  A a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=d"]);
    insta::assert_snapshot!(output, @r"
    a
    d
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=f"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    d
    e
    f
    [EOF]
    ");

    // --restore-descendants works when squashing from grandchild to grandparent
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    let output = work_dir.run_jj(["squash", "--from=c", "--into=a", "--restore-descendants"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 4 descendant commits (while preserving their content)
    Working copy  (@) now at: kpqxywon 0f4f8d7e f | (no description set)
    Parent commit (@-)      : yostqsxw 73b74c93 e | (no description set)
    Parent commit (@-)      : kkmpptxz ee689833 b c | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(run_log(), @r"
    @    0f4f8d7e07c3 f
    ├─╮  A c
    │ │  A f
    │ ○  ee689833aabe b c
    │ │  A b
    │ │  D c
    ○ │  73b74c936c84 e
    │ │  A e
    ○ │  c7eae606fc21 d
    ├─╯  D c
    │    A d
    ○  10f2f3e67384 a
    │  A a
    │  A c
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=b"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=d"]);
    insta::assert_snapshot!(output, @r"
    a
    d
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=f"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    d
    e
    f
    [EOF]
    ");

    // --restore-descendants works when squashing from grandparent to grandchild
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    let output = work_dir.run_jj(["squash", "--from=a", "--into=c", "--restore-descendants"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 1 descendant commits (while preserving their content)
    Working copy  (@) now at: kpqxywon c7279080 f | (no description set)
    Parent commit (@-)      : yostqsxw 78651b37 e | (no description set)
    Parent commit (@-)      : mzvwutvl b3d5c5e2 c | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(run_log(), @r"
    @    c7279080cf54 f
    ├─╮  A f
    │ ○  b3d5c5e22519 c
    │ │  A c
    │ ○  a469c893f362 b
    │ │  A a
    │ │  A b
    ○ │  78651b37e114 e
    │ │  A e
    ○ │  93671eb30330 d
    ├─╯  A a
    │    A d
    ◆  000000000000 a (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=b"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=d"]);
    insta::assert_snapshot!(output, @r"
    a
    d
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=f"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    d
    e
    f
    [EOF]
    ");

    // ========== Part 4: Partial Squashes to parents with --keep-emptied =========
    // Even if they don't use `--keep-emptied` explicitly, the user is likely to
    // encounter this behavior for `squash -i` and `squash` of some but not all
    // paths in a commit.
    //
    // Reminder of the setup
    test_env.advance_test_rng_seed_to_multiple_of(200_000);
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    insta::assert_snapshot!(run_log(), @r"
    @    42acd0537c88 f
    ├─╮  A f
    │ ○  4fb9706b0f47 c
    │ │  A c
    │ ○  b1e1eea2f666 b
    │ │  A b
    ○ │  b4e3197108ba e
    │ │  A e
    ○ │  d707102f499f d
    ├─╯  A d
    ○  7468364c89fc a
    │  A a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=c"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=d"]);
    insta::assert_snapshot!(output, @r"
    a
    d
    [EOF]
    ");

    // We allow squashing from a child to a direct parent with `--keep-emptied`,
    // even though there is already some uncertainty about which of two
    // reasonable behaviors (see comment in [`jj_lib::rewrite::squash_commits`])
    // is "correct". We choose the one where the source commit is left with an
    // empty diff, which is by far the less surprising one.
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    let output = work_dir.run_jj([
        "squash",
        "--from=b",
        "--into=a",
        "--keep-emptied",
        "--restore-descendants",
    ]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 5 descendant commits (while preserving their content)
    Working copy  (@) now at: kpqxywon f25876e1 f | (no description set)
    Parent commit (@-)      : yostqsxw bf3abc1c e | (no description set)
    Parent commit (@-)      : mzvwutvl 4e3db108 c | (no description set)
    [EOF]
    ");
    // WILL CHANGE NEXT COMMIT: b should now be empty, but isn't.
    insta::assert_snapshot!(run_log(), @r"
    @    f25876e1c27b f
    ├─╮  A b
    │ │  A f
    │ ○  4e3db108b4ab c
    │ │  A b
    │ │  A c
    │ ○  c8a5db5d033d b
    │ │  D b
    ○ │  bf3abc1c878e e
    │ │  A e
    ○ │  f4921ae685ee d
    ├─╯  D b
    │    A d
    ○  44bbbbbb064c a
    │  A a
    │  A b
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=b"]);
    insta::assert_snapshot!(output, @r"
    a
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=c"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=d"]);
    insta::assert_snapshot!(output, @r"
    a
    d
    [EOF]
    ");

    // Squashing from grandchild to grandparent with `--keep-emptied` also has
    // several different behaviors we could choose from. Instead of choosing,
    // we forbid this behavior entirely.
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    let output = work_dir.run_jj([
        "squash",
        "--from=c",
        "--into=a",
        "--restore-descendants",
        "--keep-emptied",
    ]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 5 descendant commits (while preserving their content)
    Working copy  (@) now at: kpqxywon e68a1aeb f | (no description set)
    Parent commit (@-)      : yostqsxw d748f816 e | (no description set)
    Parent commit (@-)      : mzvwutvl 4cc7a349 c | (empty) (no description set)
    [EOF]
    ");
    // WILL CHANGE NEXT COMMIT: This is one of several possible behaviors here,
    // perhaps a more reasonable one, but it is not consistent with what we consider
    // the "reasonable" in the cases above and below. So, we will forbid
    // squashing in this case entirely in the next commit, since any choice
    // would have confusing aspects.
    insta::assert_snapshot!(run_log(), @r"
    @    e68a1aeb8dc5 f
    ├─╮  A c
    │ │  A f
    │ ○  4cc7a3491d5f c (empty)
    │ ○  5e52be79326a b
    │ │  A b
    │ │  D c
    ○ │  d748f816e719 e
    │ │  A e
    ○ │  182d71be8f76 d
    ├─╯  D c
    │    A d
    ○  14d719ec8da2 a
    │  A a
    │  A c
    ◆  000000000000 (empty)
    [EOF]
    ");

    // squash into parent with `--keep-emptied` and two sources also has several
    // potential behaviors, and is also forbidden.
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    let output = work_dir.run_jj([
        "squash",
        "--from=b",
        "--from=d",
        "--into=a",
        "--keep-emptied",
        "--restore-descendants",
    ]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 5 descendant commits (while preserving their content)
    Working copy  (@) now at: kpqxywon b7a0c1f5 f | (no description set)
    Parent commit (@-)      : yostqsxw b26366b4 e | (no description set)
    Parent commit (@-)      : mzvwutvl ec212e80 c | (no description set)
    [EOF]
    ");
    // WILL CHANGE NEXT COMMIT: This is one of several possible behaviors here.
    // The user might expect `b` and `d` to have empty diffs, which is not the
    // case. We will forbid squashing in this case in the next commit..
    insta::assert_snapshot!(run_log(), @r"
    @    b7a0c1f57f33 f
    ├─╮  A b
    │ │  A d
    │ │  A f
    │ ○  ec212e80dcd7 c
    │ │  A b
    │ │  A c
    │ ○  6b7ecfcb7195 b
    │ │  D b
    │ │  D d
    ○ │  b26366b44ef5 e
    │ │  A d
    │ │  A e
    ○ │  11ce896ba6fa d
    ├─╯  D b
    │    D d
    ○  a92a7b2bca0b a
    │  A a
    │  A b
    │  A d
    ◆  000000000000 (empty)
    [EOF]
    ");

    // Let's take a break from the confusing cases. `squash --restore-descendants
    // --keep-emptied`` works straighforwardly when squashing from parent to child
    work_dir
        .run_jj(["operation", "restore", &beginning])
        .success();
    let output = work_dir.run_jj([
        "squash",
        "--from=a",
        "--into=b",
        "--restore-descendants",
        "--keep-emptied",
    ]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 2 descendant commits (while preserving their content)
    Working copy  (@) now at: kpqxywon 0b2d5c8b f | (no description set)
    Parent commit (@-)      : yostqsxw de0638e5 e | (no description set)
    Parent commit (@-)      : mzvwutvl 77303a4f c | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(run_log(), @r"
    @    0b2d5c8b0a2f f
    ├─╮  A f
    │ ○  77303a4f5f80 c
    │ │  A c
    │ ○  b51e7a9d2e11 b
    │ │  A a
    │ │  A b
    ○ │  de0638e59243 e
    │ │  A e
    ○ │  4f58f74d8f6b d
    ├─╯  A a
    │    A d
    ○  705b75c84a2f a (empty)
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=b"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=c"]);
    insta::assert_snapshot!(output, @r"
    a
    b
    c
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "list", "-r=d"]);
    insta::assert_snapshot!(output, @r"
    a
    d
    [EOF]
    ");
}

#[test]
fn test_squash_from_multiple() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    // Create history like this:
    //   F
    //   |
    //   E
    //  /|\
    // B C D
    //  \|/
    //   A
    work_dir
        .run_jj(["bookmark", "create", "-r@", "a"])
        .success();
    work_dir.write_file("file", "a\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "b"])
        .success();
    work_dir.write_file("file", "b\n");
    work_dir.run_jj(["new", "@-"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "c"])
        .success();
    work_dir.write_file("file", "c\n");
    work_dir.run_jj(["new", "@-"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "d"])
        .success();
    work_dir.write_file("file", "d\n");
    work_dir.run_jj(["new", "visible_heads()"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "e"])
        .success();
    work_dir.write_file("file", "e\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "f"])
        .success();
    work_dir.write_file("file", "f\n");
    // Test the setup
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  65e53f39b4d6 f
    ○      7dc592781647 e
    ├─┬─╮
    │ │ ○  fed4d1a2e491 b
    │ ○ │  d7e94ec7e73e c
    │ ├─╯
    ○ │  8acbb71558d5 d
    ├─╯
    ○  e88768e65e67 a
    ◆  000000000000 (empty)
    [EOF]
    ");

    // Squash a few commits sideways
    let output = work_dir.run_jj(["squash", "--from=b", "--from=c", "--into=d"]);
    insta::assert_snapshot!(output, @r###"
    ------- stderr -------
    Rebased 2 descendant commits
    Working copy  (@) now at: kpqxywon 703c6f0c f | (no description set)
    Parent commit (@-)      : yostqsxw 3d6a1899 e | (no description set)
    New conflicts appeared in 1 commits:
      yqosqzyt a3221d7a d | (conflict) (no description set)
    Hint: To resolve the conflicts, start by creating a commit on top of
    the conflicted commit:
      jj new yqosqzyt
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you can inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    "###);
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  703c6f0cae6f f
    ○    3d6a18995cae e
    ├─╮
    × │  a3221d7ae02a d
    ├─╯
    ○  e88768e65e67 a b c
    ◆  000000000000 (empty)
    [EOF]
    ");
    // The changes from the sources have been applied
    let output = work_dir.run_jj(["file", "show", "-r=d", "file"]);
    insta::assert_snapshot!(output, @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base #1 to side #1
    -a
    +d
    %%%%%%% Changes from base #2 to side #2
    -a
    +b
    +++++++ Contents of side #3
    c
    >>>>>>> Conflict 1 of 1 ends
    [EOF]
    ");

    // Squash a few commits up an down
    work_dir.run_jj(["undo"]).success();
    let output = work_dir.run_jj(["squash", "--from=b|c|f", "--into=e"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 1 descendant commits
    Working copy  (@) now at: xznxytkn ec32238b (empty) (no description set)
    Parent commit (@-)      : yostqsxw 5298eef6 e f | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  ec32238b2be5 (empty)
    ○    5298eef6bca5 e f
    ├─╮
    ○ │  8acbb71558d5 d
    ├─╯
    ○  e88768e65e67 a b c
    ◆  000000000000 (empty)
    [EOF]
    ");
    // The changes from the sources have been applied to the destination
    let output = work_dir.run_jj(["file", "show", "-r=e", "file"]);
    insta::assert_snapshot!(output, @r"
    f
    [EOF]
    ");

    // Empty squash shouldn't crash
    let output = work_dir.run_jj(["squash", "--from=none()"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Nothing changed.
    [EOF]
    ");
}

#[test]
fn test_squash_from_multiple_partial() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    // Create history like this:
    //   F
    //   |
    //   E
    //  /|\
    // B C D
    //  \|/
    //   A
    work_dir
        .run_jj(["bookmark", "create", "-r@", "a"])
        .success();
    work_dir.write_file("file1", "a\n");
    work_dir.write_file("file2", "a\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "b"])
        .success();
    work_dir.write_file("file1", "b\n");
    work_dir.write_file("file2", "b\n");
    work_dir.run_jj(["new", "@-"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "c"])
        .success();
    work_dir.write_file("file1", "c\n");
    work_dir.write_file("file2", "c\n");
    work_dir.run_jj(["new", "@-"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "d"])
        .success();
    work_dir.write_file("file1", "d\n");
    work_dir.write_file("file2", "d\n");
    work_dir.run_jj(["new", "visible_heads()"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "e"])
        .success();
    work_dir.write_file("file1", "e\n");
    work_dir.write_file("file2", "e\n");
    work_dir.run_jj(["new"]).success();
    work_dir
        .run_jj(["bookmark", "create", "-r@", "f"])
        .success();
    work_dir.write_file("file1", "f\n");
    work_dir.write_file("file2", "f\n");
    // Test the setup
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  4558bd852475 f
    ○      e2db96b2e57a e
    ├─┬─╮
    │ │ ○  f2c9709f39e9 b
    │ ○ │  aa908686a197 c
    │ ├─╯
    ○ │  f6812ff8db35 d
    ├─╯
    ○  64ea60be8d77 a
    ◆  000000000000 (empty)
    [EOF]
    ");

    // Partially squash a few commits sideways
    let output = work_dir.run_jj(["squash", "--from=b|c", "--into=d", "file1"]);
    insta::assert_snapshot!(output, @r###"
    ------- stderr -------
    Rebased 2 descendant commits
    Working copy  (@) now at: kpqxywon f3ae0274 f | (no description set)
    Parent commit (@-)      : yostqsxw 45ad30bd e | (no description set)
    New conflicts appeared in 1 commits:
      yqosqzyt 15efa8c0 d | (conflict) (no description set)
    Hint: To resolve the conflicts, start by creating a commit on top of
    the conflicted commit:
      jj new yqosqzyt
    Then use `jj resolve`, or edit the conflict markers in the file directly.
    Once the conflicts are resolved, you can inspect the result with `jj diff`.
    Then run `jj squash` to move the resolution into the conflicted commit.
    [EOF]
    "###);
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  f3ae0274fb6c f
    ○      45ad30bdccc6 e
    ├─┬─╮
    │ │ ○  e9db15b956c4 b
    │ ○ │  83cbe51db94d c
    │ ├─╯
    × │  15efa8c069e0 d
    ├─╯
    ○  64ea60be8d77 a
    ◆  000000000000 (empty)
    [EOF]
    ");
    // The selected changes have been removed from the sources
    let output = work_dir.run_jj(["file", "show", "-r=b", "file1"]);
    insta::assert_snapshot!(output, @r"
    a
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "-r=c", "file1"]);
    insta::assert_snapshot!(output, @r"
    a
    [EOF]
    ");
    // The selected changes from the sources have been applied
    let output = work_dir.run_jj(["file", "show", "-r=d", "file1"]);
    insta::assert_snapshot!(output, @r"
    <<<<<<< Conflict 1 of 1
    %%%%%%% Changes from base #1 to side #1
    -a
    +d
    %%%%%%% Changes from base #2 to side #2
    -a
    +b
    +++++++ Contents of side #3
    c
    >>>>>>> Conflict 1 of 1 ends
    [EOF]
    ");
    // The unselected change from the sources have not been applied to the
    // destination
    let output = work_dir.run_jj(["file", "show", "-r=d", "file2"]);
    insta::assert_snapshot!(output, @r"
    d
    [EOF]
    ");

    // Partially squash a few commits up an down
    work_dir.run_jj(["undo"]).success();
    let output = work_dir.run_jj(["squash", "--from=b|c|f", "--into=e", "file1"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Rebased 1 descendant commits
    Working copy  (@) now at: kpqxywon b5a40c15 f | (no description set)
    Parent commit (@-)      : yostqsxw 5dea187c e | (no description set)
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  b5a40c154611 f
    ○      5dea187c414d e
    ├─┬─╮
    │ │ ○  8b9afc05ca07 b
    │ ○ │  5630471a8fd5 c
    │ ├─╯
    ○ │  f6812ff8db35 d
    ├─╯
    ○  64ea60be8d77 a
    ◆  000000000000 (empty)
    [EOF]
    ");
    // The selected changes have been removed from the sources
    let output = work_dir.run_jj(["file", "show", "-r=b", "file1"]);
    insta::assert_snapshot!(output, @r"
    a
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "-r=c", "file1"]);
    insta::assert_snapshot!(output, @r"
    a
    [EOF]
    ");
    let output = work_dir.run_jj(["file", "show", "-r=f", "file1"]);
    insta::assert_snapshot!(output, @r"
    f
    [EOF]
    ");
    // The selected changes from the sources have been applied to the destination
    let output = work_dir.run_jj(["file", "show", "-r=e", "file1"]);
    insta::assert_snapshot!(output, @r"
    f
    [EOF]
    ");
    // The unselected changes from the sources have not been applied
    let output = work_dir.run_jj(["file", "show", "-r=d", "file2"]);
    insta::assert_snapshot!(output, @r"
    d
    [EOF]
    ");
}

#[test]
fn test_squash_from_multiple_partial_no_op() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    // Create history like this:
    // B C D
    //  \|/
    //   A
    work_dir.run_jj(["describe", "-m=a"]).success();
    work_dir.write_file("a", "a\n");
    work_dir.run_jj(["new", "-m=b"]).success();
    work_dir.write_file("b", "b\n");
    work_dir.run_jj(["new", "@-", "-m=c"]).success();
    work_dir.write_file("c", "c\n");
    work_dir.run_jj(["new", "@-", "-m=d"]).success();
    work_dir.write_file("d", "d\n");
    // Test the setup
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  fdb92bc249a0 d
    │ ○  0dc8cb72859d c
    ├─╯
    │ ○  b1a17f79a1a5 b
    ├─╯
    ○  93d495c46d89 a
    ◆  000000000000 (empty)
    [EOF]
    ");

    // Source commits that didn't match the paths are not rewritten
    let output = work_dir.run_jj(["squash", "--from=@-+ ~ @", "--into=@", "-m=d", "b"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: mzvwutvl 6dfc239e d
    Parent commit (@-)      : qpvuntsm 93d495c4 a
    Added 1 files, modified 0 files, removed 0 files
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  6dfc239e2ba3 d
    │ ○  0dc8cb72859d c
    ├─╯
    ○  93d495c46d89 a
    ◆  000000000000 (empty)
    [EOF]
    ");
    let output = work_dir.run_jj([
        "evolog",
        "-T",
        r#"separate(" ", commit_id.short(), description)"#,
    ]);
    insta::assert_snapshot!(output, @r"
    @    6dfc239e2ba3 d
    ├─╮  -- operation b7394e553191 (2001-02-03 08:05:13) squash commits into fdb92bc249a019337e7fa3f6c6fa74a762dd20b5
    │ ○  b1a17f79a1a5 b
    │ │  -- operation 853cf887ea1b (2001-02-03 08:05:10) snapshot working copy
    │ ○  d8b7d57239ca b
    │    -- operation a7f388a190d3 (2001-02-03 08:05:09) new empty commit
    ○  fdb92bc249a0 d
    │  -- operation 2a8d2002ac46 (2001-02-03 08:05:12) snapshot working copy
    ○  af709ccc1ca9 d
       -- operation 5aefb1c40e7d (2001-02-03 08:05:11) new empty commit
    [EOF]
    ");

    // If no source commits match the paths, then the whole operation is a no-op
    work_dir.run_jj(["undo"]).success();
    let output = work_dir.run_jj(["squash", "--from=@-+ ~ @", "--into=@", "-m=d", "a"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Nothing changed.
    [EOF]
    ");
    insta::assert_snapshot!(get_log_output(&work_dir), @r"
    @  fdb92bc249a0 d
    │ ○  0dc8cb72859d c
    ├─╯
    │ ○  b1a17f79a1a5 b
    ├─╯
    ○  93d495c46d89 a
    ◆  000000000000 (empty)
    [EOF]
    ");
}

#[must_use]
fn get_log_output(work_dir: &TestWorkDir) -> CommandOutput {
    let template = r#"separate(
        " ",
        commit_id.short(),
        bookmarks,
        description,
        if(empty, "(empty)")
    )"#;
    work_dir.run_jj(["log", "-T", template])
}

#[test]
fn test_squash_description() {
    let mut test_env = TestEnvironment::default();
    let edit_script = test_env.set_up_fake_editor();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    std::fs::write(&edit_script, r#"fail"#).unwrap();

    // If both descriptions are empty, the resulting description is empty
    work_dir.write_file("file1", "a\n");
    work_dir.write_file("file2", "a\n");
    work_dir.run_jj(["new"]).success();
    work_dir.write_file("file1", "b\n");
    work_dir.write_file("file2", "b\n");
    work_dir.run_jj(["squash"]).success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @"");

    // If the destination's description is empty and the source's description is
    // non-empty, the resulting description is from the source
    work_dir.run_jj(["undo"]).success();
    work_dir.run_jj(["describe", "-m", "source"]).success();
    work_dir.run_jj(["squash"]).success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @r"
    source
    [EOF]
    ");

    // If the destination description is non-empty and the source's description is
    // empty, the resulting description is from the destination
    work_dir.run_jj(["op", "restore", "@--"]).success();
    work_dir
        .run_jj(["describe", "@-", "-m", "destination"])
        .success();
    work_dir.run_jj(["squash"]).success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @r"
    destination
    [EOF]
    ");

    // An explicit description on the command-line overrides this
    work_dir.run_jj(["undo"]).success();
    work_dir.run_jj(["squash", "-m", "custom"]).success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @r"
    custom
    [EOF]
    ");

    // If both descriptions were non-empty, we get asked for a combined description
    work_dir.run_jj(["undo"]).success();
    work_dir.run_jj(["describe", "-m", "source"]).success();
    std::fs::write(&edit_script, "dump editor0").unwrap();
    work_dir.run_jj(["squash"]).success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @r"
    destination

    source
    [EOF]
    ");
    insta::assert_snapshot!(
        std::fs::read_to_string(test_env.env_root().join("editor0")).unwrap(), @r#"
    JJ: Enter a description for the combined commit.
    JJ: Description from the destination commit:
    destination

    JJ: Description from source commit:
    source

    JJ: This commit contains the following changes:
    JJ:     A file1
    JJ:     A file2
    JJ:
    JJ: Lines starting with "JJ:" (like this one) will be removed.
    "#);

    // An explicit description on the command-line overrides prevents launching an
    // editor
    work_dir.run_jj(["undo"]).success();
    work_dir.run_jj(["squash", "-m", "custom"]).success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @r"
    custom
    [EOF]
    ");

    // An explicit description on the command-line includes the trailers when
    // templates.commit_trailers is configured
    work_dir.run_jj(["undo"]).success();
    work_dir
        .run_jj([
            "squash",
            "--config",
            r#"templates.commit_trailers='"CC: " ++ committer.email()'"#,
            "-m",
            "custom",
        ])
        .success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @r"
    custom

    CC: test.user@example.com
    [EOF]
    ");

    // If the source's *content* doesn't become empty, then the source remains and
    // both descriptions are unchanged
    work_dir.run_jj(["undo"]).success();
    work_dir.run_jj(["squash", "file1"]).success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @r"
    destination
    [EOF]
    ");
    insta::assert_snapshot!(get_description(&work_dir, "@"), @r"
    source
    [EOF]
    ");

    // A combined description should only contain the trailers from the
    // commit_trailers template that were not in the squashed commits
    work_dir.run_jj(["undo"]).success();
    work_dir
        .run_jj(["describe", "-m", "source\n\nfoo: bar"])
        .success();
    std::fs::write(&edit_script, "dump editor0").unwrap();
    work_dir
        .run_jj([
            "squash",
            "--config",
            r#"templates.commit_trailers='"CC: alice@example.com\nfoo: bar"'"#,
        ])
        .success();
    insta::assert_snapshot!(
        std::fs::read_to_string(test_env.env_root().join("editor0")).unwrap(), @r#"
    JJ: Enter a description for the combined commit.
    JJ: Description from the destination commit:
    destination

    JJ: Description from source commit:
    source

    foo: bar

    JJ: Trailers not found in the squashed commits:
    CC: alice@example.com

    JJ: This commit contains the following changes:
    JJ:     A file1
    JJ:     A file2
    JJ:
    JJ: Lines starting with "JJ:" (like this one) will be removed.
    "#);

    // If the destination description is non-empty and the source's description is
    // empty, the resulting description is from the destination, with additional
    // trailers if defined in the commit_trailers template
    work_dir.run_jj(["op", "restore", "@--"]).success();
    work_dir.run_jj(["describe", "-m", ""]).success();
    insta::assert_snapshot!(get_log_output_with_description(&work_dir), @r"
    @  b086e6e1d02c
    ○  aeace309a1bd destination
    ◆  000000000000
    [EOF]
    ");
    work_dir
        .run_jj([
            "squash",
            "--config",
            r#"templates.commit_trailers='"CC: alice@example.com"'"#,
        ])
        .success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @r"
    destination

    CC: alice@example.com
    [EOF]
    ");

    // If a single description is non-empty, the resulting description is
    // from the destination, with additional trailers if defined in the
    // commit_trailers template
    work_dir.run_jj(["op", "restore", "@--"]).success();
    work_dir
        .run_jj(["describe", "-r", "@-", "-m", ""])
        .success();
    insta::assert_snapshot!(get_log_output_with_description(&work_dir), @r"
    @  2664d61781df source
    ○  c7a218b8d32e
    ◆  000000000000
    [EOF]
    ");
    work_dir
        .run_jj([
            "squash",
            "--config",
            r#"templates.commit_trailers='"CC: alice@example.com"'"#,
        ])
        .success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @r"
    source

    CC: alice@example.com
    [EOF]
    ");

    // squashing messages with empty descriptions shouldn't add any trailer
    work_dir.run_jj(["op", "restore", "@--"]).success();
    work_dir
        .run_jj(["describe", "-r", "..", "-m", ""])
        .success();
    insta::assert_snapshot!(get_log_output_with_description(&work_dir), @r"
    @  e024f101aae7
    ○  c6812e220e36
    ◆  000000000000
    [EOF]
    ");
    work_dir
        .run_jj([
            "squash",
            "--config",
            r#"templates.commit_trailers='"CC: bob@example.com"'"#,
        ])
        .success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @"");

    // squashing messages with --use-destination-message on a commit with an
    // empty description shouldn't add any trailer
    work_dir.run_jj(["op", "restore", "@--"]).success();
    work_dir
        .run_jj(["describe", "-r", "@-", "-m", ""])
        .success();
    insta::assert_snapshot!(get_log_output_with_description(&work_dir), @r"
    @  95925ceb516a source
    ○  db0ba0f18c8f
    ◆  000000000000
    [EOF]
    ");
    work_dir
        .run_jj([
            "squash",
            "--use-destination-message",
            "--config",
            r#"templates.commit_trailers='"CC: bob@example.com"'"#,
        ])
        .success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @"");

    // squashing with an empty message on the command line shouldn't add
    // any trailer
    work_dir.run_jj(["op", "restore", "@--"]).success();
    insta::assert_snapshot!(get_log_output_with_description(&work_dir), @r"
    @  aeaaeb3703e0 source
    ○  aeace309a1bd destination
    ◆  000000000000
    [EOF]
    ");
    work_dir
        .run_jj([
            "squash",
            "--message",
            "",
            "--config",
            r#"templates.commit_trailers='"CC: bob@example.com"'"#,
        ])
        .success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @"");
}

#[test]
fn test_squash_description_editor_avoids_unc() {
    let mut test_env = TestEnvironment::default();
    let edit_script = test_env.set_up_fake_editor();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    work_dir.write_file("file1", "a\n");
    work_dir.write_file("file2", "a\n");
    work_dir.run_jj(["new"]).success();
    work_dir.write_file("file1", "b\n");
    work_dir.write_file("file2", "b\n");
    work_dir
        .run_jj(["describe", "@-", "-m", "destination"])
        .success();
    work_dir.run_jj(["describe", "-m", "source"]).success();

    std::fs::write(edit_script, "dump-path path").unwrap();
    work_dir.run_jj(["squash"]).success();

    let edited_path =
        PathBuf::from(std::fs::read_to_string(test_env.env_root().join("path")).unwrap());
    // While `assert!(!edited_path.starts_with("//?/"))` could work here in most
    // cases, it fails when it is not safe to strip the prefix, such as paths
    // over 260 chars.
    assert_eq!(edited_path, dunce::simplified(&edited_path));
}

#[test]
fn test_squash_empty() {
    let mut test_env = TestEnvironment::default();
    test_env.set_up_fake_editor();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    work_dir.run_jj(["commit", "-m", "parent"]).success();

    let output = work_dir.run_jj(["squash"]);
    insta::assert_snapshot!(output, @r"
    ------- stderr -------
    Working copy  (@) now at: kkmpptxz db7ad962 (empty) (no description set)
    Parent commit (@-)      : qpvuntsm 771da191 (empty) parent
    [EOF]
    ");
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @r"
    parent
    [EOF]
    ");

    work_dir.run_jj(["describe", "-m", "child"]).success();
    work_dir.run_jj(["squash"]).success();
    insta::assert_snapshot!(get_description(&work_dir, "@-"), @r"
    parent

    child
    [EOF]
    ");
}

#[test]
fn test_squash_use_destination_message() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");

    work_dir.run_jj(["commit", "-m=a"]).success();
    work_dir.run_jj(["commit", "-m=b"]).success();
    work_dir.run_jj(["describe", "-m=c"]).success();
    // Test the setup
    insta::assert_snapshot!(get_log_output_with_description(&work_dir), @r"
    @  cf388db088f7 c
    ○  e412ddda5587 b
    ○  b86e28cd6862 a
    ◆  000000000000
    [EOF]
    ");

    // Squash the current revision using the short name for the option.
    work_dir.run_jj(["squash", "-u"]).success();
    insta::assert_snapshot!(get_log_output_with_description(&work_dir), @r"
    @  70c0f74e4486
    ○  44c1701e4ef8 b
    ○  b86e28cd6862 a
    ◆  000000000000
    [EOF]
    ");

    // Undo and squash again, but this time squash both "b" and "c" into "a".
    work_dir.run_jj(["undo"]).success();
    work_dir
        .run_jj([
            "squash",
            "--use-destination-message",
            "--from",
            "description(b)::",
            "--into",
            "description(a)",
        ])
        .success();
    insta::assert_snapshot!(get_log_output_with_description(&work_dir), @r"
    @  e5a16e0e6a46
    ○  6e47254e0803 a
    ◆  000000000000
    [EOF]
    ");
}

// The --use-destination-message and --message options are incompatible.
#[test]
fn test_squash_use_destination_message_and_message_mutual_exclusion() {
    let test_env = TestEnvironment::default();
    test_env.run_jj_in(".", ["git", "init", "repo"]).success();
    let work_dir = test_env.work_dir("repo");
    work_dir.run_jj(["commit", "-m=a"]).success();
    work_dir.run_jj(["describe", "-m=b"]).success();
    insta::assert_snapshot!(work_dir.run_jj([
        "squash",
        "--message=123",
        "--use-destination-message",
    ]), @r"
    ------- stderr -------
    error: the argument '--message <MESSAGE>' cannot be used with '--use-destination-message'

    Usage: jj squash --message <MESSAGE> [FILESETS]...

    For more information, try '--help'.
    [EOF]
    [exit status: 2]
    ");
}

#[must_use]
fn get_description(work_dir: &TestWorkDir, rev: &str) -> CommandOutput {
    work_dir.run_jj(["log", "--no-graph", "-T", "description", "-r", rev])
}

#[must_use]
fn get_log_output_with_description(work_dir: &TestWorkDir) -> CommandOutput {
    let template = r#"separate(" ", commit_id.short(), description)"#;
    work_dir.run_jj(["log", "-T", template])
}

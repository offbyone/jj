use jj_lib::fileset::FilesetExpression;
use jj_lib::repo::Repo;
use pollster::FutureExt as _;
use test_case::test_case;
use testutils::{TestRepo, create_tree, repo_path};

#[test_case(false; "empty tree")]
#[test_case(true; "with files")]
fn test_none(with_files: bool) {
    let test_repo = TestRepo::init();
    let expression = FilesetExpression::none();
    let fileset_matcher = expression.to_matcher();

    let tree = if with_files {
        testutils::create_tree(
            &test_repo.repo,
            &[
                (&testutils::repo_path("file1.txt"), "content1"),
                (&testutils::repo_path("dir/file2.txt"), "content2"),
            ],
        )
    } else {
        test_repo.repo.store().root_commit().tree().unwrap()
    };

    let entries: Vec<_> = tree.entries_matching(fileset_matcher.as_ref()).collect();
    assert!(entries.is_empty());
}

#[test_case(false; "empty tree")]
#[test_case(true; "with files")]
fn test_all(with_files: bool) {
    let test_repo = TestRepo::init();
    let expression = FilesetExpression::all();
    let fileset_matcher = expression.to_matcher();

    let tree = if with_files {
        testutils::create_tree(
            &test_repo.repo,
            &[
                (&testutils::repo_path("file1.txt"), "content1"),
                (&testutils::repo_path("dir/file2.txt"), "content2"),
            ],
        )
    } else {
        test_repo.repo.store().root_commit().tree().unwrap()
    };

    let entries: Vec<_> = tree.entries_matching(fileset_matcher.as_ref()).collect();

    if with_files {
        // "all" should match all files when files are present
        assert_eq!(entries.len(), 2);
        assert!(
            entries
                .iter()
                .any(|(path, _)| path.as_ref() == testutils::repo_path("file1.txt"))
        );
        assert!(
            entries
                .iter()
                .any(|(path, _)| path.as_ref() == testutils::repo_path("dir/file2.txt"))
        );
    } else {
        // root commit has an empty tree, so even "all" should match nothing
        assert!(entries.is_empty());
    }
}

#[test_case(false; "conflict-free")]
#[test_case(true; "with conflicts")]
fn test_conflicted(with_conflicts: bool) {
    let test_repo = TestRepo::init();
    let repo = &test_repo.repo;

    let expression = FilesetExpression::all();
    let fileset_matcher = expression.to_matcher();

    let mut tx = repo.start_transaction();
    let mut_repo = tx.repo_mut();
    let mut create_commit =
        |parent_ids, tree_id| mut_repo.new_commit(parent_ids, tree_id).write().unwrap();

    let file_path1 = repo_path("file1");
    let file_path2 = repo_path("file2");
    let tree1 = create_tree(repo, &[(file_path1, "1"), (file_path2, "1")]);
    let commit1 = create_commit(vec![repo.store().root_commit_id().clone()], tree1.id());
    let tree2 = create_tree(repo, &[(file_path1, "2"), (file_path2, "2")]);
    let commit2 = create_commit(vec![commit1.id().clone()], tree2.id());
    let tree3 = create_tree(repo, &[(file_path1, "3"), (file_path2, "1")]);
    let commit3 = create_commit(vec![commit2.id().clone()], tree3.id());
    let tree4 = tree2
        .clone()
        .merge(tree1.clone(), tree3.clone())
        .block_on()
        .unwrap();
    let _commit4 = create_commit(vec![commit3.id().clone()], tree4.id());

    let tree = if with_conflicts { tree4 } else { tree2 };

    let entries: Vec<_> = tree.entries_matching(fileset_matcher.as_ref()).collect();

    if with_conflicts {
        // "conflicts" should match the conflicted file
        assert_eq!(entries.len(), 1);
        assert!(
            entries
                .iter()
                .any(|(path, _)| path.as_ref() == testutils::repo_path("file1"))
        );
    } else {
        // tree2 has no conflicted files, so "conflicts" should match nothing
        assert!(entries.is_empty());
    }
}

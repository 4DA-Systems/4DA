use super::*;

#[test]
fn test_parse_porcelain_v2_branch() {
    let input = "# branch.oid abc123\n# branch.head main\n# branch.ab +2 -1\n";
    let status = parse_porcelain_v2(input);
    assert_eq!(status.branch, "main");
    assert_eq!(status.ahead, 2);
    assert_eq!(status.behind, 1);
}

#[test]
fn test_parse_porcelain_v2_files() {
    let input =
        "# branch.head main\n1 M. N... 100644 100644 100644 abc def src/main.rs\n? new_file.txt\n";
    let status = parse_porcelain_v2(input);
    assert_eq!(status.staged.len(), 1);
    assert_eq!(status.staged[0].path, "src/main.rs");
    assert_eq!(status.staged[0].status, "M");
    assert_eq!(status.untracked.len(), 1);
    assert_eq!(status.untracked[0], "new_file.txt");
}

#[test]
fn test_parse_porcelain_v2_unstaged() {
    let input = "# branch.head dev\n1 .M N... 100644 100644 100644 abc def src/lib.rs\n";
    let status = parse_porcelain_v2(input);
    assert!(status.staged.is_empty());
    assert_eq!(status.unstaged.len(), 1);
    assert_eq!(status.unstaged[0].status, "M");
}

#[test]
fn test_validate_repo_path_traversal() {
    let result = validate_repo_path("../../etc/passwd");
    assert!(result.is_err());
}

#[test]
fn test_heuristic_commit_message() {
    let stat = " src/main.rs | 10 +++++-----\n 1 file changed, 5 insertions(+), 5 deletions(-)\n";
    let msg = heuristic_commit_message(stat);
    assert!(msg.message.starts_with("chore:"));
    assert!(msg.model.is_none());
}

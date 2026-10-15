use super::*;
use tempfile::TempDir;

struct Fixture {
    dir: TempDir,
    repo: Repo,
}
fn git(dir: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir).args(args);
    String::from_utf8(output(cmd, None).unwrap()).unwrap()
}
impl Fixture {
    fn new(content: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        git(dir.path(), &["config", "user.name", "Test"]);
        git(
            dir.path(),
            &["config", "user.email", "test@example.invalid"],
        );
        git(dir.path(), &["config", "core.autocrlf", "false"]);
        fs::write(dir.path().join("file"), content).unwrap();
        fs::write(dir.path().join("other"), "other\n").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-qm", "initial"]);
        let repo = Repo::open(dir.path(), Path::new("file")).unwrap();
        Self { dir, repo }
    }
    fn work(&self, content: &str) {
        fs::write(&self.repo.path, content).unwrap();
    }
    fn state(&self) -> Snapshot {
        self.repo.snapshot().unwrap()
    }
    fn apply(&self, op: Operation, rows: &[usize]) -> Snapshot {
        self.repo.mutate(&self.state(), op, rows).unwrap()
    }
}
#[test]
fn stage_and_unstage_independent_replacement_rows() {
    let f = Fixture::new("old\n");
    f.work("new\n");
    let s = f.apply(Operation::Stage, &[1]);
    assert_eq!(s.index, "old\nnew\n");
    assert_eq!(s.work, "new\n");
    let s = f.apply(Operation::Unstage, &[1]);
    assert_eq!(s.index, "old\n");
    assert_eq!(s.work, "new\n");
    let s = f.apply(Operation::Stage, &[0]);
    assert_eq!(s.index, "");
    assert_eq!(s.work, "new\n");
    assert!(!git(f.dir.path(), &["ls-files", "--stage", "file"]).is_empty());
    let s = f.apply(Operation::Unstage, &[0]);
    assert_eq!(s.index, "old\n");
    assert_eq!(s.work, "new\n");
    let s = f.apply(Operation::Stage, &[0, 1]);
    assert_eq!(s.index, "new\n");
    let s = f.apply(Operation::Unstage, &[0, 1]);
    assert_eq!(s.index, "old\n");
}
#[test]
fn overlapping_staged_and_unstaged_are_not_combined() {
    let f = Fixture::new("old\n");
    f.work("new\n");
    f.apply(Operation::Stage, &[0, 1]);
    f.work("old\n");
    let s = f.state();
    assert_eq!(Diff::between(&s.index, &s.work).rows[0].text, "new\n");
    assert_eq!(Diff::between(&s.head, &s.index).rows[0].text, "old\n");
    let s = f.apply(Operation::Stage, &[1]);
    assert_eq!(s.index, "new\nold\n");
    assert_eq!(s.work, "old\n");
}
#[test]
fn discard_rows_independently_preserves_index_and_path() {
    let f = Fixture::new("old\n");
    f.work("new\n");
    let s = f.apply(Operation::Discard, &[0]);
    assert_eq!(s.work, "old\nnew\n");
    assert_eq!(s.index, "old\n");
    f.work("new\n");
    let s = f.apply(Operation::Discard, &[1]);
    assert_eq!(s.work, "");
    assert_eq!(s.index, "old\n");
    assert!(f.repo.path.is_file());
    f.work("new\n");
    let s = f.apply(Operation::Discard, &[0, 1]);
    assert_eq!(s.work, "old\n");
}
#[test]
fn partial_selection_and_hunk_preserve_unrelated_changes() {
    let original = (0..30).map(|n| format!("line {n}\n")).collect::<String>();
    let f = Fixture::new(&original);
    f.work(
        &original
            .replace("line 1\n", "one\n")
            .replace("line 20\n", "twenty\n"),
    );
    let s = f.state();
    let d = Diff::between(&s.index, &s.work);
    assert_eq!(d.hunks.len(), 2);
    let selected: Vec<_> = d.hunks[0].clone().filter(|i| d.actionable(*i)).collect();
    let s = f.apply(Operation::Stage, &selected);
    assert_eq!(s.index, original.replace("line 1\n", "one\n"));
    assert!(s.work.contains("twenty\n"));
    let d = Diff::between(&s.index, &s.work);
    let selected: Vec<_> = (0..d.rows.len()).filter(|i| d.actionable(*i)).collect();
    let s = f.apply(Operation::Discard, &selected);
    assert_eq!(s.work, s.index);
    assert!(s.index.contains("one\n"));
}
#[test]
fn stale_selections_and_confirmation_are_rejected() {
    let f = Fixture::new("old\n");
    f.work("new\n");
    let captured_before_confirmation = f.state();
    f.work("different\n");
    for op in [Operation::Stage, Operation::Unstage, Operation::Discard] {
        assert!(f
            .repo
            .mutate(&captured_before_confirmation, op, &[0])
            .is_err());
    }
    assert_eq!(f.state().index, "old\n");
    assert_eq!(f.state().work, "different\n");
    let captured = f.state();
    git(f.dir.path(), &["add", "file"]);
    assert!(f.repo.mutate(&captured, Operation::Stage, &[0]).is_err());
    assert_eq!(f.state().index, "different\n");
}
#[test]
fn unrelated_index_updates_are_preserved_and_lock_is_respected() {
    let f = Fixture::new("old\n");
    f.work("new\n");
    let captured = f.state();
    fs::write(f.dir.path().join("other"), "staged elsewhere\n").unwrap();
    git(f.dir.path(), &["add", "other"]);
    f.repo.mutate(&captured, Operation::Stage, &[1]).unwrap();
    assert_eq!(git(f.dir.path(), &["show", ":other"]), "staged elsewhere\n");
    let lock_path = f.dir.path().join(".git/index.lock");
    fs::write(&lock_path, "owned elsewhere").unwrap();
    assert!(f.repo.mutate(&f.state(), Operation::Stage, &[0]).is_err());
    assert_eq!(fs::read_to_string(&lock_path).unwrap(), "owned elsewhere");
    assert_eq!(f.state().index, "old\nnew\n");
}
#[test]
fn line_endings_and_final_newline_are_byte_exact() {
    let f = Fixture::new("a\r\nb\r\nlast");
    f.work("a\r\nc\r\nlast");
    let s = f.apply(Operation::Stage, &[1, 2]);
    assert_eq!(s.index, s.work);
    let s = f.apply(Operation::Unstage, &[1, 2]);
    assert_eq!(s.index, "a\r\nb\r\nlast");
    let f = Fixture::new("old");
    f.work("new");
    assert_eq!(f.apply(Operation::Stage, &[0, 1]).index, "new");
}
#[test]
fn unsupported_inputs_are_rejected_without_writes() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let f = Fixture::new("old\n");
    fs::write(f.dir.path().join("untracked"), "x").unwrap();
    let err = Repo::open(f.dir.path(), Path::new("untracked"))
        .err()
        .unwrap()
        .to_string();
    assert!(err.contains("track the file"));
    git(f.dir.path(), &["add", "untracked"]);
    assert!(Repo::open(f.dir.path(), Path::new("untracked")).is_err());
    f.work("nul\0binary");
    assert!(f.repo.snapshot().is_err());
    fs::write(&f.repo.path, [0xff]).unwrap();
    assert!(f.repo.snapshot().is_err());
    fs::remove_file(&f.repo.path).unwrap();
    assert!(f.repo.snapshot().is_err());
    symlink("other", &f.repo.path).unwrap();
    assert!(f.repo.snapshot().is_err());
    fs::remove_file(&f.repo.path).unwrap();
    f.work("old\n");
    fs::set_permissions(&f.repo.path, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(f.repo.snapshot().is_err());
    fs::set_permissions(&f.repo.path, fs::Permissions::from_mode(0o644)).unwrap();
    git(f.dir.path(), &["rm", "-q", "file"]);
    assert!(f.repo.snapshot().is_err());
}
#[test]
fn conflicts_and_transformation_attributes_are_rejected() {
    let f = Fixture::new("old\n");
    let oid = f.state().index_oid;
    let record = format!(
        "0 {}\tfile\n100644 {oid} 1\tfile\n100644 {oid} 2\tfile\n100644 {oid} 3\tfile\n",
        "0".repeat(40)
    );
    output(
        f.repo.command(&["update-index", "--index-info"]),
        Some(record.as_bytes()),
    )
    .unwrap();
    assert!(f.repo.snapshot().is_err());
    let f = Fixture::new("old\n");
    fs::write(f.dir.path().join(".gitattributes"), "file text eol=crlf\n").unwrap();
    assert!(f.repo.snapshot().is_err());
}
#[test]
fn repository_context_is_invocation_not_target() {
    let f = Fixture::new("old\n");
    let outside = tempfile::tempdir().unwrap();
    assert!(Repo::open(outside.path(), &f.repo.path).is_err());
    fs::create_dir(f.dir.path().join("sub")).unwrap();
    let repo = Repo::open(&f.dir.path().join("sub"), Path::new("../file")).unwrap();
    f.work("new\n");
    let s = repo
        .mutate(&repo.snapshot().unwrap(), Operation::Stage, &[1])
        .unwrap();
    assert_eq!(s.index, "old\nnew\n");
    assert!(git(f.dir.path(), &["ls-files", "sub/file"]).is_empty());
}
#[test]
fn unborn_head_reports_required_committed_baseline_without_mutation() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    let file = dir.path().join("hi");
    fs::write(&file, "").unwrap();
    let error = Repo::open(dir.path(), Path::new("hi")).err().unwrap();
    assert!(format!("{error:#}").contains("track the file"));
    assert!(!dir.path().join(".git/index").exists());
    git(dir.path(), &["add", "hi"]);
    let index_before = fs::read(dir.path().join(".git/index")).unwrap();
    let error = Repo::open(dir.path(), Path::new("hi")).err().unwrap();
    let message = format!("{error:#}");
    assert!(message.contains("repositories without commits"), "{message}");
    assert!(message.contains("commit the file before opening it"));
    assert_eq!(fs::read(dir.path().join(".git/index")).unwrap(), index_before);
    assert_eq!(fs::read(file).unwrap(), b"");
    assert!(!dir.path().join(".git/index.lock").exists());
}
#[test]
fn literal_special_paths_and_empty_head_content() {
    let f = Fixture::new("");
    f.work("line\n");
    let s = f.apply(Operation::Stage, &[0]);
    assert_eq!(s.index, "line\n");
    let s = f.apply(Operation::Unstage, &[0]);
    assert_eq!(s.index, "");
    git(f.dir.path(), &["mv", "file", ":odd [x]\tname"]);
    git(f.dir.path(), &["commit", "-qm", "rename"]);
    let repo = Repo::open(f.dir.path(), Path::new(":odd [x]\tname")).unwrap();
    let s = repo.snapshot().unwrap();
    let d = Diff::between(&s.index, &s.work);
    let selected: Vec<_> = (0..d.rows.len()).filter(|i| d.actionable(*i)).collect();
    assert_eq!(
        repo.mutate(&s, Operation::Stage, &selected).unwrap().index,
        "line\n"
    );
}

use crate::fuse::read_write::{mount_rw, RwFilesystem};
use crate::path_translation::PathTranslator;
use suture_core::repository::Repository;
use std::path::Path;

fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

fn setup_repo(repo_path: &Path) -> Repository {
    let mut repo = Repository::init(repo_path, "test").unwrap();
    std::fs::create_dir_all(repo_path.join("src")).unwrap();
    std::fs::write(repo_path.join("README.md"), b"# Test Repo\n").unwrap();
    std::fs::write(repo_path.join("src/main.rs"), b"fn main() {}\n").unwrap();
    std::fs::write(repo_path.join("src/lib.rs"), b"pub fn hello() -> &'static str { \"hi\" }\n")
        .unwrap();
    std::fs::create_dir_all(repo_path.join("docs")).unwrap();
    std::fs::write(repo_path.join("docs/guide.md"), b"# Guide\n\nWelcome.\n").unwrap();
    repo.add("README.md").unwrap();
    repo.add("src/main.rs").unwrap();
    repo.add("src/lib.rs").unwrap();
    repo.add("docs/guide.md").unwrap();
    repo.commit("initial commit").unwrap();
    repo
}

fn try_unmount(mountpoint: &Path) {
    let _ = std::process::Command::new("fusermount")
        .args(["-u", "-z", &mountpoint.to_string_lossy()])
        .output();
    let _ = std::process::Command::new("umount")
        .args(["-l", &mountpoint.to_string_lossy()])
        .output();
}

async fn mount_and_wait(
    repo_path: &Path,
    mountpoint: &Path,
) -> tokio::task::JoinHandle<()> {
    let repo_path = repo_path.to_path_buf();
    let mountpoint = mountpoint.to_path_buf();
    let handle = tokio::spawn(async move {
        let _ = mount_rw(
            repo_path.to_str().unwrap(),
            &mountpoint,
            None,
        )
        .await;
    });
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    handle
}

#[tokio::test]
#[ignore]
async fn test_fuse_mount_read_files() {
    if !is_root() {
        eprintln!("skipping: requires root");
        return;
    }

    let repo_dir = tempfile::tempdir().unwrap();
    let repo_path = repo_dir.path();
    setup_repo(repo_path);

    let mount_dir = tempfile::tempdir().unwrap();
    let mountpoint = mount_dir.path().join("mnt");
    std::fs::create_dir_all(&mountpoint).unwrap();

    let handle = mount_and_wait(repo_path, &mountpoint).await;

    let result = (|| {
        let readme = std::fs::read(mountpoint.join("README.md"))?;
        assert_eq!(readme.as_slice(), b"# Test Repo\n");

        let main_rs = std::fs::read(mountpoint.join("src/main.rs"))?;
        assert_eq!(main_rs.as_slice(), b"fn main() {}\n");

        let lib_rs = std::fs::read(mountpoint.join("src/lib.rs"))?;
        assert_eq!(lib_rs.as_slice(), b"pub fn hello() -> &'static str { \"hi\" }\n");

        let guide = std::fs::read(mountpoint.join("docs/guide.md"))?;
        assert_eq!(guide.as_slice(), b"# Guide\n\nWelcome.\n");

        Ok::<(), std::io::Error>(())
    })();

    try_unmount(&mountpoint);
    handle.abort();

    result.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_fuse_mount_write_creates_patch() {
    if !is_root() {
        eprintln!("skipping: requires root");
        return;
    }

    let repo_dir = tempfile::tempdir().unwrap();
    let repo_path = repo_dir.path();
    let mut repo = Repository::init(repo_path, "test").unwrap();
    std::fs::write(repo_path.join("existing.txt"), b"already here").unwrap();
    repo.add("existing.txt").unwrap();
    repo.commit("initial").unwrap();

    let mount_dir = tempfile::tempdir().unwrap();
    let mountpoint = mount_dir.path().join("mnt");
    std::fs::create_dir_all(&mountpoint).unwrap();

    let handle = mount_and_wait(repo_path, &mountpoint).await;

    let result = (|| {
        std::fs::write(mountpoint.join("brand_new.txt"), b"created via FUSE")?;
        Ok::<(), std::io::Error>(())
    })();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    try_unmount(&mountpoint);
    handle.abort();

    result.unwrap();

    let repo2 = Repository::open(repo_path).unwrap();
    let tree = repo2.snapshot_head().unwrap();
    assert!(tree.contains("brand_new.txt"));
    let blob = repo2
        .cas()
        .get_blob(tree.get("brand_new.txt").unwrap())
        .unwrap();
    assert_eq!(blob.as_slice(), b"created via FUSE");
    assert!(tree.contains("existing.txt"));

    let log = repo2.log(None).unwrap();
    let vfs_commits: Vec<_> = log.iter().filter(|c| c.message.starts_with("vfs:")).collect();
    assert!(
        vfs_commits.iter().any(|c| c.message.contains("create brand_new.txt")),
        "expected a create commit, got: {:?}",
        vfs_commits.iter().map(|c| &c.message).collect::<Vec<_>>()
    );
}

#[tokio::test]
#[ignore]
async fn test_fuse_mount_modify_file() {
    if !is_root() {
        eprintln!("skipping: requires root");
        return;
    }

    let repo_dir = tempfile::tempdir().unwrap();
    let repo_path = repo_dir.path();
    let mut repo = Repository::init(repo_path, "test").unwrap();
    std::fs::write(repo_path.join("hello.txt"), b"original content").unwrap();
    repo.add("hello.txt").unwrap();
    repo.commit("initial").unwrap();

    let mount_dir = tempfile::tempdir().unwrap();
    let mountpoint = mount_dir.path().join("mnt");
    std::fs::create_dir_all(&mountpoint).unwrap();

    let handle = mount_and_wait(repo_path, &mountpoint).await;

    let result = (|| {
        std::fs::write(mountpoint.join("hello.txt"), b"modified content")?;
        Ok::<(), std::io::Error>(())
    })();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    try_unmount(&mountpoint);
    handle.abort();

    result.unwrap();

    let repo2 = Repository::open(repo_path).unwrap();
    let tree = repo2.snapshot_head().unwrap();
    let blob = repo2
        .cas()
        .get_blob(tree.get("hello.txt").unwrap())
        .unwrap();
    assert_eq!(blob.as_slice(), b"modified content");

    let log = repo2.log(None).unwrap();
    let modify_commits: Vec<_> = log
        .iter()
        .filter(|c| c.message.starts_with("vfs:") && c.message.contains("modify"))
        .collect();
    assert!(
        !modify_commits.is_empty(),
        "expected a modify commit in log"
    );
}

#[tokio::test]
#[ignore]
async fn test_fuse_mount_directory_listing() {
    if !is_root() {
        eprintln!("skipping: requires root");
        return;
    }

    let repo_dir = tempfile::tempdir().unwrap();
    let repo_path = repo_dir.path();
    setup_repo(repo_path);

    let mount_dir = tempfile::tempdir().unwrap();
    let mountpoint = mount_dir.path().join("mnt");
    std::fs::create_dir_all(&mountpoint).unwrap();

    let handle = mount_and_wait(repo_path, &mountpoint).await;

    let result = (|| {
        let root_entries: Vec<String> = std::fs::read_dir(&mountpoint)?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(root_entries.contains(&"README.md".to_string()));
        assert!(root_entries.contains(&"src".to_string()));
        assert!(root_entries.contains(&"docs".to_string()));

        let src_entries: Vec<String> = std::fs::read_dir(mountpoint.join("src"))?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(src_entries.contains(&"main.rs".to_string()));
        assert!(src_entries.contains(&"lib.rs".to_string()));

        let docs_entries: Vec<String> = std::fs::read_dir(mountpoint.join("docs"))?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(docs_entries.contains(&"guide.md".to_string()));

        Ok::<(), std::io::Error>(())
    })();

    try_unmount(&mountpoint);
    handle.abort();

    result.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_fuse_mount_delete_file() {
    if !is_root() {
        eprintln!("skipping: requires root");
        return;
    }

    let repo_dir = tempfile::tempdir().unwrap();
    let repo_path = repo_dir.path();
    let mut repo = Repository::init(repo_path, "test").unwrap();
    std::fs::write(repo_path.join("doomed.txt"), b"goodbye").unwrap();
    std::fs::write(repo_path.join("survivor.txt"), b"stays").unwrap();
    repo.add("doomed.txt").unwrap();
    repo.add("survivor.txt").unwrap();
    repo.commit("initial").unwrap();

    let mount_dir = tempfile::tempdir().unwrap();
    let mountpoint = mount_dir.path().join("mnt");
    std::fs::create_dir_all(&mountpoint).unwrap();

    let handle = mount_and_wait(repo_path, &mountpoint).await;

    let result = (|| {
        std::fs::remove_file(mountpoint.join("doomed.txt"))?;
        Ok::<(), std::io::Error>(())
    })();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    try_unmount(&mountpoint);
    handle.abort();

    result.unwrap();

    let repo2 = Repository::open(repo_path).unwrap();
    let tree = repo2.snapshot_head().unwrap();
    assert!(!tree.contains("doomed.txt"));
    assert!(tree.contains("survivor.txt"));

    let log = repo2.log(None).unwrap();
    let delete_commits: Vec<_> = log
        .iter()
        .filter(|c| c.message.starts_with("vfs:") && c.message.contains("delete"))
        .collect();
    assert!(
        !delete_commits.is_empty(),
        "expected a delete commit in log"
    );
}

#[tokio::test]
#[ignore]
async fn test_fuse_mount_stat_file() {
    if !is_root() {
        eprintln!("skipping: requires root");
        return;
    }

    let repo_dir = tempfile::tempdir().unwrap();
    let repo_path = repo_dir.path();
    let mut repo = Repository::init(repo_path, "test").unwrap();
    let content = b"hello world, this is some content for stat testing";
    std::fs::write(repo_path.join("statme.txt"), content).unwrap();
    repo.add("statme.txt").unwrap();
    repo.commit("initial").unwrap();

    let mount_dir = tempfile::tempdir().unwrap();
    let mountpoint = mount_dir.path().join("mnt");
    std::fs::create_dir_all(&mountpoint).unwrap();

    let handle = mount_and_wait(repo_path, &mountpoint).await;

    let result = (|| {
        let meta = std::fs::metadata(mountpoint.join("statme.txt"))?;
        assert!(meta.is_file());
        assert!(!meta.is_dir());
        assert_eq!(meta.len(), content.len() as u64);

        let dir_meta = std::fs::metadata(&mountpoint)?;
        assert!(dir_meta.is_dir());
        assert!(!dir_meta.is_file());

        let src_meta = std::fs::metadata(mountpoint.join("nonexistent.txt"));
        assert!(src_meta.is_err());

        Ok::<(), std::io::Error>(())
    })();

    try_unmount(&mountpoint);
    handle.abort();

    result.unwrap();
}

#[tokio::test]
async fn test_webdav_serves_files() {
    let repo_dir = tempfile::tempdir().unwrap();
    let repo_path = repo_dir.path();
    let mut repo = Repository::init(repo_path, "test").unwrap();
    std::fs::write(repo_path.join("hello.txt"), b"hello from webdav").unwrap();
    std::fs::create_dir_all(repo_path.join("src")).unwrap();
    std::fs::write(repo_path.join("src/main.rs"), b"fn main() {}").unwrap();
    repo.add("hello.txt").unwrap();
    repo.add("src/main.rs").unwrap();
    repo.commit("initial").unwrap();

    let port = portpicker::pick_unused_port().expect("no free port");
    let addr = format!("127.0.0.1:{port}");
    let base_url = format!("http://{addr}");

    let repo_path_str = repo_path.to_str().unwrap().to_string();
    let server_handle = tokio::spawn(async move {
        let _ = crate::webdav::serve_webdav(&repo_path_str, &addr).await;
    });

    for _ in 0..20 {
        if reqwest::Client::new()
            .get(&format!("{base_url}/"))
            .send()
            .await
            .is_ok()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let root_res = reqwest::get(&format!("{base_url}/"))
        .await
        .expect("GET / failed");
    assert!(root_res.status().is_success());
    let body = root_res.text().await.unwrap();
    assert!(body.contains("hello.txt"));
    assert!(body.contains("Suture"));

    let file_res = reqwest::get(&format!("{base_url}/hello.txt"))
        .await
        .expect("GET /hello.txt failed");
    assert!(file_res.status().is_success());
    let file_body = file_res.text().await.unwrap();
    assert_eq!(file_body, "hello from webdav");

    let nested_res = reqwest::get(&format!("{base_url}/src/main.rs"))
        .await
        .expect("GET /src/main.rs failed");
    assert!(nested_res.status().is_success());
    let nested_body = nested_res.text().await.unwrap();
    assert_eq!(nested_body, "fn main() {}");

    let missing_res = reqwest::get(&format!("{base_url}/nope.txt"))
        .await
        .expect("GET /nope.txt failed");
    assert_eq!(missing_res.status(), reqwest::StatusCode::NOT_FOUND);

    server_handle.abort();
}

#[test]
fn test_path_translation_roundtrip() {
    let paths = [
        "src/main.rs",
        "src/lib.rs",
        "README.md",
        "a/b/c/deep.txt",
        "top-level.txt",
    ];
    let t = PathTranslator::build(&paths);

    for path in &paths {
        assert!(t.is_file(path), "expected file: {path}");
    }

    assert!(t.is_dir(""));
    assert!(t.is_dir("src"));
    assert!(t.is_dir("a"));
    assert!(t.is_dir("a/b"));
    assert!(t.is_dir("a/b/c"));
    assert!(!t.is_dir("src/main.rs"));
    assert!(!t.is_dir("README.md"));

    let root_entries = t.list_dir("");
    let root_names: Vec<&str> = root_entries.iter().map(|e| e.name.as_str()).collect();
    assert!(root_names.contains(&"src"));
    assert!(root_names.contains(&"README.md"));
    assert!(root_names.contains(&"top-level.txt"));
    assert!(root_names.contains(&"a"));

    let src_entries = t.list_dir("src");
    assert_eq!(src_entries.len(), 2);
    assert_eq!(src_entries[0].name, "lib.rs");
    assert!(!src_entries[0].is_dir);
    assert_eq!(src_entries[1].name, "main.rs");
    assert!(!src_entries[1].is_dir);

    let deep_entries = t.list_dir("a/b/c");
    assert_eq!(deep_entries.len(), 1);
    assert_eq!(deep_entries[0].name, "deep.txt");
}

#[test]
fn test_path_translation_spaces_and_special() {
    let paths = [
        "my file.txt",
        "dir with spaces/nested file.rs",
        "unicode_\u{00e9}\u{00f1}.txt",
        "d1/d2/d3/final.md",
        "file.with.dots.txt",
        "UPPER_CASE.TXT",
    ];
    let t = PathTranslator::build(&paths);

    assert!(t.is_file("my file.txt"));
    assert!(t.is_file("dir with spaces/nested file.rs"));
    assert!(t.is_file("unicode_\u{00e9}\u{00f1}.txt"));
    assert!(t.is_file("d1/d2/d3/final.md"));
    assert!(t.is_file("file.with.dots.txt"));
    assert!(t.is_file("UPPER_CASE.TXT"));

    assert!(t.is_dir("dir with spaces"));
    assert!(t.is_dir("d1"));
    assert!(t.is_dir("d1/d2"));
    assert!(t.is_dir("d1/d2/d3"));

    let root = t.list_dir("");
    let root_names: Vec<&str> = root.iter().map(|e| e.name.as_str()).collect();
    assert!(root_names.contains(&"my file.txt"));
    assert!(root_names.contains(&"dir with spaces"));
    assert!(root_names.contains(&"unicode_\u{00e9}\u{00f1}.txt"));
    assert!(root_names.contains(&"d1"));
    assert!(root_names.contains(&"file.with.dots.txt"));
    assert!(root_names.contains(&"UPPER_CASE.TXT"));

    let nested = t.list_dir("dir with spaces");
    assert_eq!(nested.len(), 1);
    assert_eq!(nested[0].name, "nested file.rs");

    let deep = t.list_dir("d1/d2/d3");
    assert_eq!(deep.len(), 1);
    assert_eq!(deep[0].name, "final.md");
}

#[test]
fn test_path_translation_empty_and_single() {
    let t_empty = PathTranslator::build(&[]);
    assert!(t_empty.is_dir(""));
    assert!(t_empty.list_dir("").is_empty());

    let t_single = PathTranslator::build(&["solo.txt"]);
    assert!(t_single.is_file("solo.txt"));
    assert!(!t_single.is_dir("solo.txt"));
    assert!(t_single.is_dir(""));

    let entries = t_single.list_dir("");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "solo.txt");
    assert!(!entries[0].is_dir);
}

#[test]
fn test_path_translation_deeply_nested() {
    let paths = ["a/b/c/d/e/f/g.txt"];
    let t = PathTranslator::build(&paths);

    for dir in ["a", "a/b", "a/b/c", "a/b/c/d", "a/b/c/d/e", "a/b/c/d/e/f"] {
        assert!(t.is_dir(dir), "expected dir: {dir}");
    }

    assert!(t.is_file("a/b/c/d/e/f/g.txt"));
    assert_eq!(t.list_dir("a/b/c/d/e/f").len(), 1);
    assert_eq!(t.list_dir("a/b/c/d/e/f")[0].name, "g.txt");
}

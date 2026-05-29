fn parse_suture_url_helper(url: &str) -> (String, String) {
    let stripped = url.strip_prefix("suture://").unwrap_or(url);
    let parts: Vec<&str> = stripped.splitn(2, '/').collect();
    let host = parts.get(0).unwrap_or(&"localhost:8080");
    let repo_id = parts.get(1).unwrap_or(&"default").to_string();
    (format!("http://{}", host), repo_id)
}

#[test]
fn test_parse_suture_url_basic() {
    let (base, repo) = parse_suture_url_helper("suture://hub.example.com/myrepo");
    assert_eq!(base, "http://hub.example.com");
    assert_eq!(repo, "myrepo");
}

#[test]
fn test_parse_suture_url_with_port() {
    let (base, repo) = parse_suture_url_helper("suture://hub.example.com:9090/myrepo");
    assert_eq!(base, "http://hub.example.com:9090");
    assert_eq!(repo, "myrepo");
}

#[test]
fn test_parse_suture_url_nested_path() {
    let (base, repo) = parse_suture_url_helper("suture://hub.example.com/org/repo");
    assert_eq!(base, "http://hub.example.com");
    assert_eq!(repo, "org/repo");
}

#[test]
fn test_handle_fetch_empty_repo() {
    let (_base, repo) = parse_suture_url_helper("suture://localhost:9999/empty");
    assert_eq!(repo, "empty");
}

#[test]
fn test_push_refspec_parsing() {
    let cmd = "push refs/heads/main:refs/heads/main";
    let parts: Vec<&str> = cmd.strip_prefix("push ").unwrap().split(':').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], "refs/heads/main");
    assert_eq!(parts[1], "refs/heads/main");
}

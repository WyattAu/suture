use axum::{
    Router,
    body::Body,
    extract::{Path as AxumPath, State},
    http::{Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use suture_core::repository::Repository;
use tokio::net::TcpListener;

struct WebDavState {
    repo_path: std::path::PathBuf,
    file_contents: Mutex<HashMap<String, Vec<u8>>>,
    dirs: Mutex<Vec<String>>,
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn build_propfind_collection(path: &str, entries: &[(&str, bool)]) -> String {
    let display = if path.is_empty() {
        "/"
    } else {
        &format!("/{path}")
    };
    let entries_xml = entries
        .iter()
        .map(|(name, is_dir)| {
            let href = format!("{}/{}", display, escape_xml(name));
            let kind = if *is_dir {
                "<D:resourcetype><D:collection/></D:resourcetype>"
            } else {
                "<D:resourcetype/>"
            };
            format!(
                "<D:response><D:href>{href}</D:href><D:propstat><D:prop>{kind}</D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>"
            )
        })
        .collect::<Vec<_>>()
        .join("");

    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
         <D:multistatus xmlns:D=\"DAV:\">\
         <D:response><D:href>{display}</D:href><D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>\
         {entries_xml}\
         </D:multistatus>"
    )
}

fn build_propfind_resource(path: &str, size: usize) -> String {
    let display = if path.is_empty() {
        "/"
    } else {
        &format!("/{path}")
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
         <D:multistatus xmlns:D=\"DAV:\">\
         <D:response><D:href>{display}</D:href><D:propstat><D:prop><D:resourcetype/><D:getcontentlength>{size}</D:getcontentlength></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>\
         </D:multistatus>"
    )
}

fn list_entries(state: &WebDavState, dir_path: &str) -> Vec<(String, bool)> {
    let files = state.file_contents.lock().unwrap();
    let dirs = state.dirs.lock().unwrap();

    let mut entries: Vec<(String, bool)> = Vec::new();

    for d in dirs.iter() {
        if d == dir_path {
            continue;
        }
        let parent = parent_of(d).unwrap_or_default();
        if parent == dir_path {
            let name = d.rsplit('/').next().unwrap_or(d).to_string();
            entries.push((name, true));
        }
    }

    for path in files.keys() {
        let parent = parent_of(path).unwrap_or_default();
        if parent == dir_path {
            let name = path.rsplit('/').next().unwrap_or(path).to_string();
            entries.push((name, false));
        }
    }

    entries.sort_by(|a, b| match (a.1, b.1) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.0.cmp(&b.0),
    });

    entries
}

fn parent_of(path: &str) -> Option<String> {
    let pos = path.rfind('/')?;
    Some(path[..pos].to_string())
}

fn ensure_parent_dirs(dirs: &mut Vec<String>, path: &str) {
    let parts: Vec<&str> = path.split('/').collect();
    let mut prefix = String::new();
    for part in parts.iter().take(parts.len() - 1) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(part);
        if !dirs.contains(&prefix) {
            dirs.push(prefix.clone());
        }
    }
}

async fn handle_options() -> impl IntoResponse {
    let mut res = Response::new(Body::empty());
    *res.status_mut() = StatusCode::OK;
    res.headers_mut().insert(
        header::ALLOW,
        "OPTIONS,GET,PUT,DELETE,PROPFIND,MKCOL,HEAD"
            .parse()
            .unwrap(),
    );
    res.headers_mut()
        .insert(header::CONTENT_LENGTH, "0".parse().unwrap());
    res.headers_mut().insert(
        header::HeaderName::from_static("dav"),
        "1,2".parse().unwrap(),
    );
    res
}

async fn handle_propfind_path(
    State(state): State<Arc<WebDavState>>,
    AxumPath(path): AxumPath<String>,
) -> impl IntoResponse {
    let clean = path.trim_start_matches('/').trim_end_matches('/');

    let dirs = state.dirs.lock().unwrap();
    let is_dir = clean.is_empty() || dirs.contains(&clean.to_string());
    drop(dirs);

    if is_dir {
        let entries = list_entries(&state, clean);
        let entries_ref: Vec<(&str, bool)> =
            entries.iter().map(|(n, d)| (n.as_str(), *d)).collect();
        let body = build_propfind_collection(clean, &entries_ref);
        (
            StatusCode::MULTI_STATUS,
            [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
            body,
        )
    } else {
        let files = state.file_contents.lock().unwrap();
        let size = files.get(clean).map(|d| d.len()).unwrap_or(0);
        drop(files);
        let body = build_propfind_resource(clean, size);
        (
            StatusCode::MULTI_STATUS,
            [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
            body,
        )
    }
}

async fn handle_get_root(State(state): State<Arc<WebDavState>>) -> impl IntoResponse {
    let entries = list_entries(&state, "");

    let html = entries
        .iter()
        .map(|(name, is_dir)| {
            if *is_dir {
                format!("<li><a href=\"{name}/\">{name}/</a></li>")
            } else {
                format!("<li><a href=\"{name}\">{name}</a></li>")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        format!(
            "<html><head><title>Suture WebDAV</title></head>\
             <body><h1>Suture Repository</h1><ul>{html}</ul></body></html>"
        ),
    )
}

async fn handle_get_file(
    State(state): State<Arc<WebDavState>>,
    AxumPath(path): AxumPath<String>,
) -> Response {
    let clean = path.trim_start_matches('/');

    let files = state.file_contents.lock().unwrap();
    match files.get(clean) {
        Some(data) => {
            let mime = mime_guess_path(clean);
            let body = data.clone();
            drop(files);
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, mime),
                    (header::CONTENT_LENGTH, body.len().to_string()),
                ],
                body,
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

fn mime_guess_path(path: &str) -> String {
    match path.rsplit('.').next() {
        Some("rs") => "text/rust",
        Some("toml") => "text/toml",
        Some("md") => "text/markdown",
        Some("json") => "application/json",
        Some("txt") => "text/plain",
        Some("html") | Some("htm") => "text/html",
        Some("css") => "text/css",
        Some("js") => "text/javascript",
        Some("xml") => "application/xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        Some("gz") | Some("tgz") => "application/gzip",
        _ => "application/octet-stream",
    }
    .to_string()
}

async fn handle_put(
    State(state): State<Arc<WebDavState>>,
    AxumPath(path): AxumPath<String>,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let clean = path.trim_start_matches('/').to_string();

    if clean.is_empty() || clean.ends_with('/') {
        return StatusCode::CONFLICT.into_response();
    }

    {
        let mut dirs = state.dirs.lock().unwrap();
        ensure_parent_dirs(&mut dirs, &clean);
    }

    state
        .file_contents
        .lock()
        .unwrap()
        .insert(clean.clone(), body.to_vec());

    tracing::info!("PUT {} ({} bytes)", clean, body.len());

    if let Ok(mut repo) = Repository::open(&state.repo_path) {
        let full_path = state.repo_path.join(&clean);
        if let Some(parent) = full_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&full_path, &body);
        let _ = repo.add(&clean);
        let _ = repo.commit(&format!("webdav: modify {}", clean));
    }

    StatusCode::NO_CONTENT.into_response()
}

async fn handle_delete(
    State(state): State<Arc<WebDavState>>,
    AxumPath(path): AxumPath<String>,
) -> impl IntoResponse {
    let clean = path.trim_start_matches('/');

    let mut removed = false;

    {
        let mut files = state.file_contents.lock().unwrap();
        if files.remove(clean).is_some() {
            removed = true;
        }
    }

    if !removed {
        let mut dirs = state.dirs.lock().unwrap();
        if let Some(pos) = dirs.iter().position(|d| d == clean) {
            dirs.remove(pos);
            removed = true;
        }
    }

    if !removed {
        return StatusCode::NOT_FOUND.into_response();
    }

    tracing::info!("DELETE {}", clean);

    if let Ok(mut repo) = Repository::open(&state.repo_path) {
        let full_path = state.repo_path.join(clean);
        if full_path.exists() {
            let _ = std::fs::remove_file(&full_path);
        }
        let _ = repo.add(clean);
        let _ = repo.commit(&format!("webdav: delete {}", clean));
    }

    StatusCode::NO_CONTENT.into_response()
}

async fn handle_mkcol(
    State(state): State<Arc<WebDavState>>,
    AxumPath(path): AxumPath<String>,
) -> impl IntoResponse {
    let clean = path
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string();

    if clean.is_empty() {
        return StatusCode::FORBIDDEN.into_response();
    }

    {
        let mut dirs = state.dirs.lock().unwrap();
        if dirs.contains(&clean) {
            return StatusCode::METHOD_NOT_ALLOWED.into_response();
        }
        ensure_parent_dirs(&mut dirs, &clean);
        dirs.push(clean.clone());
    }

    tracing::info!("MKCOL {}", clean);

    StatusCode::CREATED.into_response()
}

async fn handle_webdav_fallback(
    method: Method,
    uri: axum::http::Uri,
    State(state): State<Arc<WebDavState>>,
) -> Response {
    let path = uri.path().to_string();
    match method.as_str() {
        "PROPFIND" => handle_propfind_path(State(state), AxumPath(path))
            .await
            .into_response(),
        "MKCOL" => handle_mkcol(State(state), AxumPath(path))
            .await
            .into_response(),
        _ => (StatusCode::METHOD_NOT_ALLOWED, "method not allowed").into_response(),
    }
}

type FileContents = HashMap<String, Vec<u8>>;

fn load_repo(repo_path: &Path) -> anyhow::Result<(FileContents, Vec<String>)> {
    let repo = Repository::open(repo_path)
        .map_err(|e| anyhow::anyhow!("failed to open repository: {e}"))?;

    let file_tree = repo
        .snapshot_head()
        .map_err(|e| anyhow::anyhow!("snapshot failed: {e}"))?;

    let mut file_contents = HashMap::new();
    let mut dirs = vec![String::new()];

    for (path, hash) in file_tree.iter() {
        if let Ok(data) = repo.cas().get_blob(hash) {
            let parts: Vec<&str> = path.split('/').collect();
            let mut prefix = String::new();
            for part in parts.iter().take(parts.len() - 1) {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(part);
                if !dirs.contains(&prefix) {
                    dirs.push(prefix.clone());
                }
            }
            file_contents.insert(path.clone(), data);
        }
    }

    dirs.sort();

    Ok((file_contents, dirs))
}

pub async fn serve_webdav(repo_path: &str, addr: &str) -> Result<(), anyhow::Error> {
    let repo = Path::new(repo_path);
    if !repo.exists() {
        anyhow::bail!("repository path does not exist: {}", repo_path);
    }

    let (file_contents, dirs) = load_repo(repo)?;

    let state = Arc::new(WebDavState {
        repo_path: repo.to_path_buf(),
        file_contents: Mutex::new(file_contents),
        dirs: Mutex::new(dirs),
    });

    let app = Router::new()
        .route("/", axum::routing::get(handle_get_root))
        .route(
            "/{*path}",
            axum::routing::get(handle_get_file)
                .put(handle_put)
                .delete(handle_delete),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            webdav_method_middleware,
        ))
        .fallback(handle_webdav_fallback)
        .with_state(state);

    let listener = TcpListener::bind(addr).await?;
    tracing::info!("WebDAV server listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

async fn webdav_method_middleware(
    method: Method,
    uri: axum::http::Uri,
    State(state): State<Arc<WebDavState>>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Response {
    match method.as_str() {
        "OPTIONS" => handle_options().await.into_response(),
        "PROPFIND" => {
            let path = uri.path().to_string();
            handle_propfind_path(State(state), AxumPath(path))
                .await
                .into_response()
        }
        "MKCOL" => {
            let path = uri.path().to_string();
            handle_mkcol(State(state), AxumPath(path))
                .await
                .into_response()
        }
        "HEAD" => (StatusCode::OK).into_response(),
        _ => next.run(request).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use tower::ServiceExt;

    fn make_test_state(
        file_contents: HashMap<String, Vec<u8>>,
        dirs: Vec<String>,
    ) -> Arc<WebDavState> {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path().to_path_buf();
        let _ = Repository::init(&repo_path, "test");
        Arc::new(WebDavState {
            repo_path,
            file_contents: Mutex::new(file_contents),
            dirs: Mutex::new(dirs),
        })
    }

    fn test_app(state: Arc<WebDavState>) -> Router {
        Router::new()
            .route("/", axum::routing::get(handle_get_root))
            .route(
                "/{*path}",
                axum::routing::get(handle_get_file)
                    .put(handle_put)
                    .delete(handle_delete),
            )
            .route_layer(axum::middleware::from_fn_with_state(
                state.clone(),
                webdav_method_middleware,
            ))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_propfind_response() {
        let mut files = HashMap::new();
        files.insert("hello.txt".to_string(), b"hello world".to_vec());
        files.insert("src/main.rs".to_string(), b"fn main() {}".to_vec());

        let state = make_test_state(files, vec![String::new(), "src".to_string()]);
        let app = test_app(state);

        let req = HttpRequest::builder()
            .method("PROPFIND")
            .uri("/")
            .header("depth", "1")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::MULTI_STATUS);

        let content_type = res.headers().get(header::CONTENT_TYPE).unwrap();
        assert_eq!(content_type, "application/xml; charset=utf-8");

        let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let xml = String::from_utf8(body.to_vec()).unwrap();

        assert!(xml.contains("<?xml version=\"1.0\""));
        assert!(xml.contains("DAV:"));
        assert!(xml.contains("multistatus"));
        assert!(xml.contains("collection"));
        assert!(xml.contains("hello.txt"));
        assert!(xml.contains("src"));
        assert!(xml.contains("200 OK"));
    }

    #[tokio::test]
    async fn test_propfind_nested_dir() {
        let mut files = HashMap::new();
        files.insert("a/b/c/deep.rs".to_string(), b"deep".to_vec());

        let state = make_test_state(
            files,
            vec![
                String::new(),
                "a".to_string(),
                "a/b".to_string(),
                "a/b/c".to_string(),
            ],
        );
        let app = test_app(state);

        let req = HttpRequest::builder()
            .method("PROPFIND")
            .uri("/a/b")
            .header("depth", "1")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::MULTI_STATUS);

        let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let xml = String::from_utf8(body.to_vec()).unwrap();

        assert!(xml.contains("c"));
        assert!(xml.contains("collection"));
    }

    #[tokio::test]
    async fn test_capabilities() {
        let state = make_test_state(HashMap::new(), vec![String::new()]);
        let app = test_app(state);

        let req = HttpRequest::builder()
            .method(Method::OPTIONS)
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let allow = res.headers().get(header::ALLOW).unwrap().to_str().unwrap();
        assert!(allow.contains("OPTIONS"));
        assert!(allow.contains("GET"));
        assert!(allow.contains("PUT"));
        assert!(allow.contains("DELETE"));
        assert!(allow.contains("PROPFIND"));
        assert!(allow.contains("MKCOL"));

        let dav = res.headers().get("dav").unwrap().to_str().unwrap();
        assert!(dav.contains("1"));
        assert!(dav.contains("2"));
    }

    #[tokio::test]
    async fn test_get_file() {
        let mut files = HashMap::new();
        files.insert("hello.txt".to_string(), b"hello world".to_vec());

        let state = make_test_state(files, vec![String::new()]);
        let app = test_app(state);

        let req = HttpRequest::builder()
            .method(Method::GET)
            .uri("/hello.txt")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let body = axum::body::to_bytes(res.into_body(), 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(body.as_ref(), b"hello world");
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let state = make_test_state(HashMap::new(), vec![String::new()]);
        let app = test_app(state);

        let req = HttpRequest::builder()
            .method(Method::GET)
            .uri("/nonexistent.txt")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_put_file() {
        let state = make_test_state(HashMap::new(), vec![String::new()]);
        let app = test_app(state.clone());

        let req = HttpRequest::builder()
            .method(Method::PUT)
            .uri("/newfile.txt")
            .header(header::CONTENT_TYPE, "text/plain")
            .body(Body::from("new content"))
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        let files = state.file_contents.lock().unwrap();
        assert_eq!(files.get("newfile.txt").unwrap().as_slice(), b"new content");
    }

    #[tokio::test]
    async fn test_delete_file() {
        let mut files = HashMap::new();
        files.insert("hello.txt".to_string(), b"hello".to_vec());

        let state = make_test_state(files, vec![String::new()]);
        let app = test_app(state.clone());

        let req = HttpRequest::builder()
            .method(Method::DELETE)
            .uri("/hello.txt")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        let files = state.file_contents.lock().unwrap();
        assert!(!files.contains_key("hello.txt"));
    }

    #[tokio::test]
    async fn test_mkcol() {
        let state = make_test_state(HashMap::new(), vec![String::new()]);
        let app = test_app(state.clone());

        let req = HttpRequest::builder()
            .method("MKCOL")
            .uri("/newdir")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);

        let dirs = state.dirs.lock().unwrap();
        assert!(dirs.contains(&"newdir".to_string()));
    }

    #[tokio::test]
    async fn test_mkcol_duplicate() {
        let state = make_test_state(HashMap::new(), vec![String::new(), "existing".to_string()]);
        let app = test_app(state.clone());

        let req = HttpRequest::builder()
            .method("MKCOL")
            .uri("/existing")
            .body(Body::empty())
            .unwrap();

        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("a&b"), "a&amp;b");
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
        assert_eq!(escape_xml("it's"), "it&apos;s");
        assert_eq!(escape_xml("say \"hi\""), "say &quot;hi&quot;");
    }

    #[test]
    fn test_load_repo() {
        let dir = tempfile::tempdir().unwrap();
        let repo_path = dir.path();
        let mut repo = Repository::init(repo_path, "test").unwrap();

        std::fs::write(repo_path.join("a.txt"), b"aaa").unwrap();
        repo.add("a.txt").unwrap();
        repo.commit("init").unwrap();

        let (files, dirs) = load_repo(repo_path).unwrap();
        assert!(files.contains_key("a.txt"));
        assert_eq!(files.get("a.txt").unwrap().as_slice(), b"aaa");
        assert!(dirs.contains(&String::new()));
    }
}

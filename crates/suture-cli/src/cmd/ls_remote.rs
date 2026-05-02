use std::path::Path as StdPath;

pub async fn cmd_ls_remote(remote_or_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = if remote_or_url.starts_with("http://") || remote_or_url.starts_with("https://") {
        remote_or_url.to_owned()
    } else {
        let repo = suture_core::repository::Repository::open(StdPath::new("."))?;
        repo.get_remote_url(remote_or_url)?
    };

    crate::remote_proto::check_handshake(&url).await?;

    let repo_id = crate::remote_proto::derive_repo_id(&url, remote_or_url);

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{url}/repos/{repo_id}/branches"))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("failed to list remote branches: {status} - {text}").into());
    }

    #[derive(serde::Deserialize)]
    struct BranchInfo {
        name: String,
        target_id: crate::remote_proto::HashProto,
    }

    let branches: Vec<BranchInfo> = resp.json().await?;

    if branches.is_empty() {
        println!("(no branches)");
        return Ok(());
    }

    for branch in &branches {
        println!("{}\trefs/heads/{}", branch.target_id.value, branch.name);
    }

    Ok(())
}

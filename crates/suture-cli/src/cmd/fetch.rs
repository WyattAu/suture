use crate::remote_proto::do_fetch;

pub(crate) async fn cmd_fetch(
    remote: &str,
    depth: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut repo = suture_core::repository::Repository::open(std::path::Path::new("."))?;
    eprintln!("Fetching from {}...", remote);
    let new_patches = do_fetch(&mut repo, remote, depth).await?;
    println!("Fetch successful: {} new patch(es)", new_patches);
    Ok(())
}

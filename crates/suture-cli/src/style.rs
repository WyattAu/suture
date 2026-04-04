pub(crate) const ANSI_RED: &str = "\x1b[31m";
pub(crate) const ANSI_GREEN: &str = "\x1b[32m";
pub(crate) const ANSI_BOLD_CYAN: &str = "\x1b[1;36m";
pub(crate) const ANSI_RESET: &str = "\x1b[0m";

pub(crate) fn run_hook_if_exists(
    repo_root: &std::path::Path,
    hook_name: &str,
    extra_env: std::collections::HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let author = extra_env.get("SUTURE_AUTHOR").cloned();
    let branch = extra_env.get("SUTURE_BRANCH").cloned();
    let head = extra_env.get("SUTURE_HEAD").cloned();

    let env = suture_core::hooks::build_env(
        repo_root,
        hook_name,
        author.as_deref(),
        branch.as_deref(),
        head.as_deref(),
        extra_env,
    );

    match suture_core::hooks::run_hooks(repo_root, hook_name, &env) {
        Ok(results) => {
            for result in &results {
                if !result.stdout.is_empty() {
                    print!("{}", result.stdout);
                }
                if !result.success() {
                    let msg = format!(
                        "{}{} {}",
                        ANSI_RED,
                        suture_core::hooks::format_hook_result(result),
                        ANSI_RESET
                    );
                    eprintln!("{}", msg);
                    if !result.stderr.is_empty() {
                        eprintln!("{}", result.stderr);
                    }
                    return Err(format!(
                        "Hook '{}' failed (exit code {:?}). Aborting.",
                        hook_name, result.exit_code
                    )
                    .into());
                }
            }
            Ok(())
        }
        Err(suture_core::hooks::HookError::NotFound(_)) => Ok(()),
        Err(e) => Err(format!("Hook '{}' error: {}", hook_name, e).into()),
    }
}

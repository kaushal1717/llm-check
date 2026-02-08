// src/updater.rs
use owo_colors::OwoColorize;
use self_update::cargo_crate_version;

pub async fn check_for_update() {
    println!("{}", "Checking for updates...".cyan());

    let result = tokio::task::spawn_blocking(get_latest_version).await;

    match result {
        Ok(Ok(latest)) => {
            let current = cargo_crate_version!();
            if latest != current {
                println!(
                    "{} New version available: {} -> {}",
                    "[update]".green(),
                    current.dimmed(),
                    latest.green()
                );
                println!("  Run `llm-check update` to install");
            } else {
                println!("{} Already on the latest version ({})", "[ok]".green(), current);
            }
        }
        Ok(Err(e)) => {
            eprintln!("{} Failed to check for updates: {}", "[error]".red(), e);
        }
        Err(e) => {
            eprintln!("{} Failed to check for updates: {}", "[error]".red(), e);
        }
    }
}

pub async fn perform_update() {
    println!("{}", "Checking for updates...".cyan());

    let result = tokio::task::spawn_blocking(|| {
        self_update::backends::github::Update::configure()
            .repo_owner("kaushal1717")
            .repo_name("llm-check")
            .bin_name("llm-check")
            .show_download_progress(true)
            .current_version(cargo_crate_version!())
            .build()
            .and_then(|updater| updater.update())
    })
    .await;

    match result {
        Ok(Ok(status)) => {
            let current = cargo_crate_version!();
            if status.version() != current {
                println!(
                    "{} Updated to version {}",
                    "[ok]".green(),
                    status.version().green()
                );
            } else {
                println!(
                    "{} Already on the latest version ({})",
                    "[ok]".green(),
                    current
                );
            }
        }
        Ok(Err(e)) => {
            eprintln!("{} Update failed: {}", "[error]".red(), e);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("{} Update failed: {}", "[error]".red(), e);
            std::process::exit(1);
        }
    }
}

fn get_latest_version() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner("kaushal1717")
        .repo_name("llm-check")
        .build()?
        .fetch()?;

    releases
        .first()
        .map(|r| r.version.clone())
        .ok_or_else(|| "No releases found".into())
}

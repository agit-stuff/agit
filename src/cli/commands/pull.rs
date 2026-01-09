//! Implementation of the `agit pull` command.
//!
//! Pulls agit refs (refs/agit/*) from a remote repository.

use git2::{Cred, FetchOptions, RemoteCallbacks, Repository};

use crate::cli::args::PullArgs;
use crate::error::{AgitError, Result};

/// Execute the `pull` command.
pub fn execute(args: PullArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(".agit");

    // Check if initialized
    if !agit_dir.exists() {
        return Err(AgitError::NotInitialized);
    }

    // Open the git repository
    let repo = Repository::discover(&cwd)?;

    // Find the remote
    let mut remote = repo.find_remote(&args.remote).map_err(|e| {
        AgitError::InvalidArgument(format!("Remote '{}' not found: {}", args.remote, e))
    })?;

    // Set up callbacks for authentication
    let mut callbacks = RemoteCallbacks::new();

    // Try SSH agent first, then credential helper
    callbacks.credentials(|_url, username_from_url, allowed_types| {
        if allowed_types.contains(git2::CredentialType::SSH_KEY) {
            // Try SSH agent
            if let Some(username) = username_from_url {
                return Cred::ssh_key_from_agent(username);
            }
        }

        if allowed_types.contains(git2::CredentialType::USER_PASS_PLAINTEXT) {
            // Try credential helper
            return Cred::credential_helper(
                &Repository::discover(".").unwrap().config().unwrap(),
                _url,
                username_from_url,
            );
        }

        if allowed_types.contains(git2::CredentialType::DEFAULT) {
            return Cred::default();
        }

        Err(git2::Error::from_str("No suitable credentials found"))
    });

    // Track transfer progress
    callbacks.transfer_progress(|progress| {
        if progress.total_objects() > 0 {
            print!(
                "\rFetching: {}/{} objects ({} bytes)",
                progress.received_objects(),
                progress.total_objects(),
                progress.received_bytes()
            );
        }
        true
    });

    let mut fetch_options = FetchOptions::new();
    fetch_options.remote_callbacks(callbacks);

    // The refspec to fetch refs/agit/* from remote to local
    let refspec = "refs/agit/*:refs/agit/*";

    println!("Fetching agit refs from '{}'...", args.remote);

    // Fetch the refs
    remote.fetch(&[refspec], Some(&mut fetch_options), None)?;

    // Count fetched refs
    let refs: Vec<_> = repo
        .references_glob("refs/agit/*")?
        .filter_map(|r| r.ok())
        .collect();

    println!("\nFetched {} agit ref(s) from '{}'", refs.len(), args.remote);

    // Show the refs
    if !refs.is_empty() {
        println!("\nAvailable refs:");
        for reference in refs {
            if let Some(name) = reference.name() {
                if let Some(branch) = name.strip_prefix("refs/agit/heads/") {
                    let short_hash = reference
                        .target()
                        .map(|oid| oid.to_string()[..7].to_string())
                        .unwrap_or_else(|| "???????".to_string());
                    println!("  {} -> {}", branch, short_hash);
                }
            }
        }
    }

    Ok(())
}

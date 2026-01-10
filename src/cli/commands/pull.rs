//! Implementation of the `agit pull` command.
//!
//! Pulls agit refs (refs/agit/*) from a remote repository.
//! After fetching, automatically updates the search index with new commits.

use std::collections::HashMap;
use std::path::Path;

use git2::{Cred, FetchOptions, RemoteCallbacks, Repository};

use crate::cli::args::PullArgs;
use crate::error::{AgitError, Result};
use crate::search::incremental;

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

    // Capture refs BEFORE fetch (for incremental indexing)
    let refs_before = capture_agit_refs(&repo)?;

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

    println!(
        "\nFetched {} agit ref(s) from '{}'",
        refs.len(),
        args.remote
    );

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

    // Capture refs AFTER fetch
    let refs_after = capture_agit_refs(&repo)?;

    // Auto-update search index (non-fatal)
    index_after_pull(&cwd, &agit_dir, &refs_before, &refs_after);

    Ok(())
}

/// Capture all agit refs as a map of branch name to commit hash.
fn capture_agit_refs(repo: &Repository) -> Result<HashMap<String, String>> {
    let mut refs = HashMap::new();

    if let Ok(references) = repo.references_glob("refs/agit/heads/*") {
        for reference in references.filter_map(|r| r.ok()) {
            if let (Some(name), Some(target)) = (reference.name(), reference.target()) {
                if let Some(branch) = name.strip_prefix("refs/agit/heads/") {
                    refs.insert(branch.to_string(), target.to_string());
                }
            }
        }
    }

    Ok(refs)
}

/// Update the search index after pulling new commits.
fn index_after_pull(
    repo_path: &Path,
    agit_dir: &Path,
    refs_before: &HashMap<String, String>,
    refs_after: &HashMap<String, String>,
) {
    // Check if any refs changed
    if refs_before == refs_after {
        return;
    }

    match incremental::index_new_commits(repo_path, agit_dir, refs_before, refs_after) {
        Ok((count, was_full_rebuild)) => {
            if count > 0 {
                if was_full_rebuild {
                    println!("\nRebuilt search index ({} entries)", count);
                } else {
                    println!("\nIndexed {} new search entries", count);
                }
            }
        },
        Err(e) => {
            eprintln!("\nWarning: Failed to update search index: {}", e);
            eprintln!("Run 'agit search rebuild' to rebuild manually.");
        },
    }
}

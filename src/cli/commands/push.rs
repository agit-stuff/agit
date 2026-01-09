//! Implementation of the `agit push` command.
//!
//! Pushes agit refs (refs/agit/*) to a remote repository.

use git2::{Cred, PushOptions, RemoteCallbacks, Repository};

use crate::cli::args::PushArgs;
use crate::error::{AgitError, Result};

/// Execute the `push` command.
pub fn execute(args: PushArgs) -> Result<()> {
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

    // Check if we have any refs to push
    let refs: Vec<_> = repo
        .references_glob("refs/agit/*")?
        .filter_map(|r| r.ok())
        .collect();

    if refs.is_empty() {
        println!("No agit refs to push.");
        println!("Use 'agit commit' with V2 storage to create refs.");
        return Ok(());
    }

    // Build the refspec
    let refspec = if args.force {
        "+refs/agit/*:refs/agit/*"
    } else {
        "refs/agit/*:refs/agit/*"
    };

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
    callbacks.push_transfer_progress(|current, total, bytes| {
        if total > 0 {
            print!("\rPushing: {}/{} objects ({} bytes)", current, total, bytes);
        }
    });

    let mut push_options = PushOptions::new();
    push_options.remote_callbacks(callbacks);

    // Push the refs
    println!("Pushing agit refs to '{}'...", args.remote);

    match remote.push(&[refspec], Some(&mut push_options)) {
        Ok(()) => {
            println!("\nPushed {} ref(s) to '{}'", refs.len(), args.remote);
            Ok(())
        },
        Err(e) => {
            if e.message().contains("non-fast-forward") && !args.force {
                println!("\nFailed to push: remote refs have diverged.");
                println!("Use 'agit push --force' to overwrite, or 'agit pull' first.");
            }
            Err(AgitError::Git(e))
        },
    }
}

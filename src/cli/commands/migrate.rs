//! Implementation of the `agit migrate` command.
//!
//! Migrates storage from V1 (file-based) to V2 (Git-native).

use git2::Repository;

use crate::cli::args::MigrateArgs;
use crate::core::{cleanup_v1_storage, detect_version, migrate_v1_to_v2, StorageVersion};
use crate::error::{AgitError, Result};

/// Execute the `migrate` command.
pub fn execute(args: MigrateArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let agit_dir = cwd.join(".agit");

    // Check if initialized
    if !agit_dir.exists() {
        return Err(AgitError::NotInitialized);
    }

    // Open the git repository
    let repo = Repository::discover(&cwd)?;

    // Detect current storage version
    let current_version = detect_version(&agit_dir, &repo);

    println!("Current storage version: {:?}", current_version);

    match current_version {
        StorageVersion::V1FileSystem => {
            println!("\nMigrating from V1 (file-based) to V2 (Git-native)...\n");

            let result = migrate_v1_to_v2(&agit_dir, &repo)?;

            println!("Migration complete:");
            println!("  - Objects migrated: {}", result.objects_migrated);
            println!("  - Refs migrated: {}", result.refs_migrated);

            if args.cleanup {
                println!("\nCleaning up old V1 storage...");
                cleanup_v1_storage(&agit_dir)?;
                println!("Old storage removed.");
            } else {
                println!("\nOld V1 storage preserved. Run with --cleanup to remove.");
            }

            println!("\nStorage is now using V2 Git-native format.");
            println!("Objects are stored in Git ODB, refs are in refs/agit/*");
        }

        StorageVersion::V2GitNative => {
            if args.force {
                println!("\nAlready using V2 Git-native storage.");
                println!("Nothing to migrate.");
            } else {
                println!("\nAlready using V2 Git-native storage. Nothing to do.");
                println!("Use --force to re-run migration.");
            }
        }

        StorageVersion::Mixed => {
            println!("\nMixed storage detected (both V1 and V2 refs exist).");
            println!("This may indicate a partial previous migration.\n");

            println!("Migrating remaining V1 data to V2...\n");

            let result = migrate_v1_to_v2(&agit_dir, &repo)?;

            println!("Migration complete:");
            println!("  - Objects migrated: {}", result.objects_migrated);
            println!("  - Refs migrated: {}", result.refs_migrated);

            if args.cleanup {
                println!("\nCleaning up old V1 storage...");
                cleanup_v1_storage(&agit_dir)?;
                println!("Old storage removed.");
            }
        }
    }

    Ok(())
}

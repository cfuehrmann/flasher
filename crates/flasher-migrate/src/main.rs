//! `flasher-migrate --from <dir> --db <path> [--dry-run] [--overwrite]`
//!
//! Imports an old .NET `FileStore` directory into the `SQLite` store.
//! Re-running over an unchanged database is a no-op; if database cards
//! have diverged from the snapshot (e.g. SRS progress made in the new
//! app) the import refuses to write unless `--overwrite` is given.
//! Exit codes: 0 = OK, 1 = import/verification failure, 2 = bad CLI usage.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use flasher_migrate::{dry_run, import, import_with_overwrite, render_report};
use flasher_store::Store;

const USAGE: &str = "usage: flasher-migrate --from <dir> --db <path> [--dry-run] [--overwrite]";

struct Args {
    from: PathBuf,
    db: PathBuf,
    dry_run: bool,
    overwrite: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut from = None;
    let mut db = None;
    let mut dry_run = false;
    let mut overwrite = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--from" => {
                from = Some(PathBuf::from(args.next().ok_or("--from requires a value")?));
            }
            "--db" => {
                db = Some(PathBuf::from(args.next().ok_or("--db requires a value")?));
            }
            "--dry-run" => dry_run = true,
            "--overwrite" => overwrite = true,
            "--help" | "-h" => return Err(String::new()),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Args {
        from: from.ok_or("missing --from <dir>")?,
        db: db.ok_or("missing --db <path>")?,
        dry_run,
        overwrite,
    })
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            if message.is_empty() {
                println!("{USAGE}");
            } else {
                eprintln!("{message}\n{USAGE}");
            }
            return ExitCode::from(2);
        }
    };

    let now = now_millis();

    if args.dry_run {
        return match dry_run(&args.from, now) {
            Ok(report) => {
                print!("{}", render_report(&report));
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            }
        };
    }

    let result = async {
        let store = Store::connect(&args.db).await?;
        if args.overwrite {
            import_with_overwrite(&args.from, &store, now).await
        } else {
            import(&args.from, &store, now).await
        }
    }
    .await;

    match result {
        Ok(report) => {
            let ok = report.is_ok();
            print!("{}", render_report(&report));
            if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Independent wall clock for the test (deliberately not `now_millis`,
    /// so a `now_millis` mutant can't move both the reference and the
    /// value under test).
    fn wall_millis() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
    }

    #[test]
    fn now_millis_is_current_epoch_millis() {
        let t = super::now_millis();
        assert!(
            t > 1_700_000_000_000,
            "expected a time after 2023-11-14, got {t}"
        );
        assert!(t <= wall_millis(), "time from the future: {t}");
    }
}

use std::{
    error::Error,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use sqlx::{postgres::PgPoolOptions, Executor};
use zeroclaw_core::{
    Category, FindingKind, NewFinding, NewScan, RiskLevel, ScanPhase, ScanScoreUpdate, ScanStatus,
    ScanStatusUpdate, Severity,
};
use zeroclaw_storage::{migrate, Repository};

const PG_BIN_DIR: &str = "/usr/lib/postgresql/16/bin";

#[tokio::test]
async fn repository_persists_scans_and_findings() -> Result<(), Box<dyn Error>> {
    let cluster = TestCluster::start()?;
    let database_name = format!("repo_test_{}", unique_suffix());
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&cluster.postgres_url("postgres"))
        .await?;

    admin_pool
        .execute(format!("CREATE DATABASE {database_name}").as_str())
        .await?;

    let database_url = cluster.postgres_url(&database_name);
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    migrate(&pool).await?;

    let repo = Repository::new(pool.clone());

    let queued_scan = repo
        .insert_scan(&NewScan {
            url: "https://example.com".to_owned(),
            normalized_url: "https://example.com/".to_owned(),
            status: ScanStatus::Pending,
            phase: ScanPhase::Queued,
        })
        .await?;

    assert_eq!(queued_scan.status, ScanStatus::Pending);
    assert_eq!(queued_scan.phase, ScanPhase::Queued);
    assert_eq!(queued_scan.accessibility_score, None);
    assert!(!queued_scan.content_safety_skipped);

    let updated_scan = repo
        .update_scan_status(
            queued_scan.id,
            &ScanStatusUpdate {
                status: ScanStatus::Running,
                phase: ScanPhase::Accessibility,
                error_reason: None,
            },
        )
        .await?
        .ok_or("scan should exist after status update")?;

    assert_eq!(updated_scan.status, ScanStatus::Running);
    assert_eq!(updated_scan.phase, ScanPhase::Accessibility);

    let scored_scan = repo
        .update_scan_scores(
            queued_scan.id,
            &ScanScoreUpdate {
                accessibility_score: Some(4),
                inappropriate_score: Some(7),
                risk_level: Some(RiskLevel::High),
                content_safety_skipped: true,
            },
        )
        .await?
        .ok_or("scan should exist after score update")?;

    assert_eq!(scored_scan.accessibility_score, Some(4));
    assert_eq!(scored_scan.inappropriate_score, Some(7));
    assert_eq!(scored_scan.risk_level, Some(RiskLevel::High));
    assert!(scored_scan.content_safety_skipped);

    let completed_scan = repo
        .update_scan_status(
            queued_scan.id,
            &ScanStatusUpdate {
                status: ScanStatus::Completed,
                phase: ScanPhase::Completed,
                error_reason: None,
            },
        )
        .await?
        .ok_or("scan should exist after completion update")?;

    assert_eq!(completed_scan.status, ScanStatus::Completed);
    assert_eq!(completed_scan.phase, ScanPhase::Completed);

    let another_scan = repo
        .insert_scan(&NewScan {
            url: "https://example.com?fresh=1".to_owned(),
            normalized_url: "https://example.com/".to_owned(),
            status: ScanStatus::Completed,
            phase: ScanPhase::Completed,
        })
        .await?;

    let recent_completed = repo
        .find_recent_completed_by_url("https://example.com/")
        .await?
        .ok_or("completed scan should exist")?;

    assert_eq!(recent_completed.id, another_scan.id);

    let fetched_scan = repo
        .find_scan_by_id(queued_scan.id)
        .await?
        .ok_or("scan should exist by id")?;

    assert_eq!(fetched_scan.id, queued_scan.id);
    assert_eq!(fetched_scan.risk_level, Some(RiskLevel::High));

    let inserted_findings = repo
        .insert_findings_batch(
            queued_scan.id,
            &[
                NewFinding {
                    kind: FindingKind::Accessibility,
                    title: "Missing alt text".to_owned(),
                    category: Category::Accessibility,
                    severity: Severity::Medium,
                    summary: "Image element is missing alt text".to_owned(),
                    location: Some("img.hero".to_owned()),
                    suggestion: Some("Provide descriptive alt text.".to_owned()),
                    example_excerpt: None,
                    why_unsafe: None,
                },
                NewFinding {
                    kind: FindingKind::ContentSafety,
                    title: "Weapon promotion".to_owned(),
                    category: Category::Weapons,
                    severity: Severity::High,
                    summary: "Page markets realistic weapon usage.".to_owned(),
                    location: None,
                    suggestion: Some("Restrict or remove the content.".to_owned()),
                    example_excerpt: Some("Buy tactical rifles today".to_owned()),
                    why_unsafe: Some("Promotes weapon acquisition.".to_owned()),
                },
            ],
        )
        .await?;

    assert_eq!(inserted_findings.len(), 2);
    assert_eq!(inserted_findings[0].scan_id, queued_scan.id);
    assert_eq!(inserted_findings[1].category, Category::Weapons);

    let listed_findings = repo.list_findings_for_scan(queued_scan.id).await?;

    assert_eq!(listed_findings, inserted_findings);

    drop(repo);
    pool.close().await;
    admin_pool
        .execute(format!("DROP DATABASE {database_name}").as_str())
        .await?;
    admin_pool.close().await;

    Ok(())
}

struct TestCluster {
    data_dir: PathBuf,
    log_file: PathBuf,
    port: u16,
    socket_dir: PathBuf,
}

impl TestCluster {
    fn start() -> Result<Self, Box<dyn Error>> {
        let data_dir = create_temp_dir("zeroclaw-pgdata")?;
        let socket_dir = create_temp_dir("zeroclaw-pgsock")?;
        let log_file = create_temp_file("zeroclaw-pglog")?;
        let port = pick_unused_port()?;

        run_as_postgres(&[
            "initdb",
            "-A",
            "trust",
            "-U",
            "postgres",
            "-D",
            path_arg(&data_dir)?,
        ])?;

        run_as_postgres(&[
            "pg_ctl",
            "-D",
            path_arg(&data_dir)?,
            "-l",
            path_arg(&log_file)?,
            "-o",
            &format!("-h 127.0.0.1 -k {} -p {port}", socket_dir.display()),
            "start",
        ])?;

        Ok(Self {
            data_dir,
            log_file,
            port,
            socket_dir,
        })
    }

    fn postgres_url(&self, database_name: &str) -> String {
        format!("postgresql://postgres@127.0.0.1:{}/{}", self.port, database_name)
    }
}

impl Drop for TestCluster {
    fn drop(&mut self) {
        let _ = run_as_postgres(&[
            "pg_ctl",
            "-D",
            self.data_dir.to_string_lossy().as_ref(),
            "stop",
            "-m",
            "fast",
        ]);

        let _ = std::fs::remove_dir_all(&self.data_dir);
        let _ = std::fs::remove_dir_all(&self.socket_dir);
        let _ = std::fs::remove_file(&self.log_file);
    }
}

fn run_as_postgres(args: &[&str]) -> Result<(), Box<dyn Error>> {
    let binary = format!("{PG_BIN_DIR}/{}", args[0]);
    let status = Command::new("runuser")
        .args(["-u", "postgres", "--", &binary])
        .args(&args[1..])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;

    if !status.success() {
        return Err(format!("command failed: runuser -u postgres -- {binary}").into());
    }

    Ok(())
}

fn create_temp_dir(prefix: &str) -> Result<PathBuf, Box<dyn Error>> {
    let dir = std::env::temp_dir().join(format!("{prefix}-{}", unique_suffix()));
    std::fs::create_dir_all(&dir)?;
    chown_to_postgres(&dir)?;
    Ok(dir)
}

fn create_temp_file(prefix: &str) -> Result<PathBuf, Box<dyn Error>> {
    let path = std::env::temp_dir().join(format!("{prefix}-{}", unique_suffix()));
    std::fs::File::create(&path)?;
    chown_to_postgres(&path)?;
    Ok(path)
}

fn chown_to_postgres(path: &Path) -> Result<(), Box<dyn Error>> {
    let status = Command::new("chown")
        .arg("postgres:postgres")
        .arg(path)
        .status()?;

    if !status.success() {
        return Err(format!("failed to chown {}", path.display()).into());
    }

    Ok(())
}

fn pick_unused_port() -> Result<u16, Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

fn path_arg(path: &Path) -> Result<&str, Box<dyn Error>> {
    path.to_str()
        .ok_or_else(|| format!("non-utf8 path: {}", path.display()).into())
}

fn unique_suffix() -> u128 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(_) => 0,
    }
}

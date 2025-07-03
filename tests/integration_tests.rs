use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

#[derive(Debug, Clone)]
struct TestError {
    details: String,
}

impl TestError {
    fn new(msg: &str) -> TestError {
        TestError {
            details: msg.to_string(),
        }
    }
}

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl std::error::Error for TestError {
    fn description(&self) -> &str {
        &self.details
    }
}

fn get_slurmtail_path() -> PathBuf {
    // Use the binary that cargo test builds for us
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    let debug_path = current_dir.join("target/debug/slurmtail");
    let release_path = current_dir.join("target/release/slurmtail");

    // Prefer release if it exists and is newer, otherwise use debug
    if release_path.exists() && debug_path.exists() {
        let release_meta = std::fs::metadata(&release_path).ok();
        let debug_meta = std::fs::metadata(&debug_path).ok();

        if let (Some(release), Some(debug)) = (release_meta, debug_meta) {
            if release.modified().unwrap_or(std::time::UNIX_EPOCH)
                >= debug.modified().unwrap_or(std::time::UNIX_EPOCH)
            {
                return release_path;
            }
        }
    } else if release_path.exists() {
        return release_path;
    }

    debug_path
}

fn create_test_script(temp_dir: &TempDir) -> std::path::PathBuf {
    let script_content = r#"#!/usr/bin/env bash
#SBATCH --job-name=slurmtail_test
#SBATCH --output=test_output.%j.log
#SBATCH --error=test_error.%j.log
#SBATCH --time=00:01:00
#SBATCH --ntasks=1

echo "Test job started at $(date)"
sleep 5
echo "Test job processing..."
sleep 5
echo "Test job completed at $(date)"
"#;
    let script_path = temp_dir.path().join("test_job.sh");
    fs::write(&script_path, script_content).expect("Failed to create test script");

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    script_path
}

fn create_test_script_with_job_name(temp_dir: &TempDir) -> std::path::PathBuf {
    let script_content = r#"#!/usr/bin/env bash
#SBATCH --job-name=test_job_name
#SBATCH --output=test_output.%x.%j.log
#SBATCH --time=00:01:00
#SBATCH --ntasks=1

echo "Test job with name started at $(date)"
sleep 2
echo "Job name should be: test_job_name"
echo "Test job with name completed at $(date)"
"#;
    let script_path = temp_dir.path().join("test_job_with_name.sh");
    fs::write(&script_path, script_content).expect("Failed to create test script");

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    script_path
}

fn create_test_script_jobname(temp_dir: &TempDir) -> std::path::PathBuf {
    let script_content = r#"#!/usr/bin/env bash
#SBATCH --job-name=slurmtail_test
#SBATCH --output=test_output.%j.log
#SBATCH --error=test_error.%x.%j.log
#SBATCH --time=00:01:00
#SBATCH --ntasks=1

echo "Test job started at $(date)"
sleep 5
echo "Test job processing..."
sleep 5
echo "Test job completed at $(date)"
"#;
    let script_path = temp_dir.path().join("test_job.sh");
    fs::write(&script_path, script_content).expect("Failed to create test script");

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }

    script_path
}

#[test]
fn test_run_command_basic() -> Result<(), Box<dyn std::error::Error>> {
    // Create temporary directory for this test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let script_path = create_test_script(&temp_dir);
    let resume_file = temp_dir.path().join("._slurmtail");

    // Check if SLURM is available
    let slurm_check = Command::new("sinfo").output();

    if slurm_check.is_err() {
        println!("SLURM not available, failing integration test");
        return Err(Box::new(TestError::new("Slurm not available!")));
    }

    // Run slurmtail with a very short timeout and capture output
    let output = Command::new(get_slurmtail_path())
        .args(&["run", script_path.to_str().unwrap(), "--timeout", "5"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run slurmtail");

    // Check the output to see if job was submitted successfully
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // If job submission failed (e.g., SLURM not available), skip this test
    if stderr.contains("sbatch failed") || stderr.contains("No such file") {
        println!("SLURM not available or test script failed, skipping test");
        return Err("Slurm not available, or test script failed to submit to Slurm.".into());
    }

    // Job should have been submitted
    assert!(
        stdout.contains("Job submitted with ID:"),
        "Should submit job successfully: stdout={}, stderr={}",
        stdout,
        stderr
    );

    // Check that resume file was created
    assert!(resume_file.exists(), "Resume file should be created");

    // Check that the resume file contains a log path
    let resume_content = fs::read_to_string(&resume_file).expect("Failed to read resume file");
    assert!(
        resume_content.contains("test_output."),
        "Resume file should contain log path pattern"
    );

    Ok(())
}

#[test]
fn test_run_command_with_job_name() -> Result<(), Box<dyn std::error::Error>> {
    // Create temporary directory for this test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let script_path = create_test_script_with_job_name(&temp_dir);
    let resume_file = temp_dir.path().join("._slurmtail");

    // Check if SLURM is available
    let slurm_check = Command::new("sinfo").output();

    if slurm_check.is_err() {
        println!("SLURM not available, failing integration test");
        return Err(Box::new(TestError::new("Slurm not available!")));
    }

    // Run slurmtail with a short timeout and capture output
    let output = Command::new(get_slurmtail_path())
        .args(&["run", script_path.to_str().unwrap(), "--timeout", "10"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run slurmtail");

    // Check the output to see if job was submitted successfully
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // If job submission failed (e.g., SLURM not available), skip this test
    if stderr.contains("sbatch failed") || stderr.contains("No such file") {
        println!("SLURM not available or test script failed, skipping test");
        return Err("Slurm not available, or test script failed to submit to Slurm.".into());
    }

    // Job should have been submitted
    assert!(
        stdout.contains("Job submitted with ID:"),
        "Should submit job successfully: stdout={}, stderr={}",
        stdout,
        stderr
    );

    // Check that the debug output shows the job name in the log path
    assert!(
        stdout.contains("test_job_name"),
        "Debug output should show job name in log path: {}",
        stdout
    );

    // Check that resume file was created
    assert!(resume_file.exists(), "Resume file should be created");

    // Check that the resume file contains a log path with job name
    let resume_content = fs::read_to_string(&resume_file).expect("Failed to read resume file");
    assert!(
        resume_content.contains("test_job_name"),
        "Resume file should contain job name in log path: {}",
        resume_content
    );

    Ok(())
}

#[test]
fn test_run_command_jobname() -> Result<(), Box<dyn std::error::Error>> {
    // Create temporary directory for this test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let script_path = create_test_script_jobname(&temp_dir);
    let resume_file = temp_dir.path().join("._slurmtail");

    // Check if SLURM is available
    let slurm_check = Command::new("sinfo").output();

    if slurm_check.is_err() {
        println!("SLURM not available, failing integration test");
        return Err(Box::new(TestError::new("Slurm not available!")));
    }

    // Run slurmtail with a very short timeout and capture output
    let output = Command::new(get_slurmtail_path())
        .args(&["run", script_path.to_str().unwrap(), "--timeout", "5"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run slurmtail");

    // Check the output to see if job was submitted successfully
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // If job submission failed (e.g., SLURM not available), skip this test
    if stderr.contains("sbatch failed") || stderr.contains("No such file") {
        println!("SLURM not available or test script failed, skipping test");
        return Err("Slurm not available, or test script failed to submit to Slurm.".into());
    }

    // Job should have been submitted
    assert!(
        stdout.contains("Job submitted with ID:"),
        "Should submit job successfully: stdout={}, stderr={}",
        stdout,
        stderr
    );

    // Check that resume file was created
    assert!(resume_file.exists(), "Resume file should be created");

    // Check that the resume file contains a valid log path
    let resume_content = fs::read_to_string(&resume_file).expect("Failed to read resume file");
    assert!(
        resume_content.contains("test_output."),
        "Resume file should contain log path pattern"
    );

    Ok(())
}

#[test]
fn test_resume_command() {
    // Create temporary directory for this test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_log_path = temp_dir.path().join("fake_test.log");
    let resume_file = temp_dir.path().join("._slurmtail");

    // Create a fake log file
    fs::write(&test_log_path, "Test log content\nLine 2\n").expect("Failed to create test log");

    // Create resume file pointing to the test log
    fs::write(&resume_file, test_log_path.to_string_lossy().as_ref())
        .expect("Failed to create resume file");

    // Test resume command with very short timeout
    let output = Command::new(get_slurmtail_path())
        .args(&["resume", "--timeout", "1"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run slurmtail resume");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("Resuming monitoring"),
        "Should indicate resuming: {}",
        combined
    );
}

#[test]
fn test_extract_log_pattern() {
    // Create temporary directory for this test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Test with non-existent script file
    let output = Command::new(get_slurmtail_path())
        .args(&["run", "nonexistent.sh"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run slurmtail");

    assert!(
        !output.status.success(),
        "Should fail with non-existent script"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not exist"),
        "Should indicate file doesn't exist"
    );
}

#[test]
fn test_resume_without_file() {
    // Create temporary directory for this test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Test resume without any resume file
    let output = Command::new(get_slurmtail_path())
        .args(&["resume"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run slurmtail resume");

    assert!(!output.status.success(), "Should fail without resume file");
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        combined.contains("No resume file found"),
        "Should indicate no resume file: {}",
        combined
    );
}

#[test]
fn test_invalid_resume_file() {
    // Create temporary directory for this test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let resume_file = temp_dir.path().join("._slurmtail");

    // Create resume file pointing to non-existent log
    fs::write(&resume_file, "/non/existent/log.file").expect("Failed to create resume file");

    let output = Command::new(get_slurmtail_path())
        .args(&["resume"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run slurmtail resume");

    assert!(
        !output.status.success(),
        "Should fail with invalid log path"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no longer exists"),
        "Should indicate log file doesn't exist"
    );
}

#[test]
fn test_resume_with_job_name_log() {
    // Create temporary directory for this test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let test_log_path = temp_dir.path().join("output.test_job_name.12345.log");
    let resume_file = temp_dir.path().join("._slurmtail");

    // Create a fake log file with job name format
    fs::write(
        &test_log_path,
        "Test job with name started\nJob name: test_job_name\nTest completed\n",
    )
    .expect("Failed to create test log");

    // Create resume file pointing to the test log with job name
    fs::write(&resume_file, test_log_path.to_string_lossy().as_ref())
        .expect("Failed to create resume file");

    // Test resume command with short timeout
    let output = Command::new(get_slurmtail_path())
        .args(&["resume", "--timeout", "2"])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run slurmtail resume");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(
        combined.contains("Resuming monitoring"),
        "Should indicate resuming: {}",
        combined
    );

    // Should show the job name in the log content
    assert!(
        combined.contains("test_job_name"),
        "Should show job name from log content: {}",
        combined
    );
}

#[test]
fn test_no_file_timeout_flag() -> Result<(), Box<dyn std::error::Error>> {
    // Create temporary directory for this test
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let script_path = create_test_script(&temp_dir);

    // Check if SLURM is available
    let slurm_check = Command::new("sinfo").output();

    if slurm_check.is_err() {
        println!("SLURM not available, failing integration test");
        return Err(Box::new(TestError::new("Slurm not available!")));
    }

    // Run slurmtail with no-file-timeout flag and very short timeout for monitoring
    let output = Command::new(get_slurmtail_path())
        .args(&[
            "run",
            script_path.to_str().unwrap(),
            "--no-file-timeout",
            "--timeout",
            "5",
        ])
        .current_dir(temp_dir.path())
        .output()
        .expect("Failed to run slurmtail");

    // Check the output to see if job was submitted successfully
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // If job submission failed (e.g., SLURM not available), skip this test
    if stderr.contains("sbatch failed") || stderr.contains("No such file") {
        println!("SLURM not available or test script failed, skipping test");
        return Err("Slurm not available, or test script failed to submit to Slurm.".into());
    }

    // Job should have been submitted
    assert!(
        stdout.contains("Job submitted with ID:"),
        "Should submit job successfully: stdout={}, stderr={}",
        stdout,
        stderr
    );

    // Should not contain file timeout message (since we're using --no-file-timeout)
    // but might timeout on monitoring after 5 seconds
    let combined = format!("{}{}", stdout, stderr);
    assert!(
        !combined.contains("File took too long to appear")
            || combined.contains("Timed out after 5 seconds"),
        "Should not timeout on file appearance with --no-file-timeout flag: {}",
        combined
    );

    Ok(())
}

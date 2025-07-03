use clap::{Arg, Command};
use jiff::{Unit, Zoned};
use std::env;
use std::fs::{File, read_to_string};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::thread::sleep;
use std::time::Duration;

// Function responsible for monitoring ('tailing') a log file given to it
fn mon_logfile(
    log_path: &Path,
    file_appear_timeout_s: Option<u32>,
    timeout_s: Option<u32>,
    no_file_timeout: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Handle args
    let file_appear_timeout = if no_file_timeout {
        i64::MAX // Effectively infinite timeout
    } else {
        file_appear_timeout_s.unwrap_or(120u32) as i64
    };
    let timeout = timeout_s.unwrap_or(120u32) as i64;

    // Log start time
    let start_time = Zoned::now()
        .round(Unit::Second)
        .expect("Could not get date/time information!");

    // Retry opening the file until it is created
    let mut printed_stat = false; // Only print the status once
    let mut file = loop {
        match File::open(log_path) {
            Ok(f) => {
                println!("[INFO] Found file: {:?}", log_path);
                break f;
            }
            Err(_) => {
                if !printed_stat {
                    println!("[INFO] Waiting for log file to be created: {:?}", log_path);
                    printed_stat = true;
                }
                sleep(Duration::from_secs(1));
            }
        }

        // Exit if we have been waiting longer than the timeout
        let time_now = Zoned::now()
            .round(Unit::Second)
            .expect("[FATAL] Could not get date/time information!");
        if start_time
            .until((Unit::Second, &time_now))
            .expect("[FATAL] Error while comparing times! Exiting.")
            .get_seconds()
            > file_appear_timeout
        {
            println!(
                "[FATAL] File took too long to appear (longer than timeout of {} seconds). Exiting.",
                file_appear_timeout
            );
            return Err("Timeout waiting for log file".into());
        }
    };

    // Find the starting position for the last 150 lines (or beginning if fewer than 150 lines)
    let start_position = {
        let file_size = file.metadata()?.len();

        if file_size == 0 {
            0
        } else {
            let mut newline_count = 0;
            let mut position = file_size;
            let mut buffer = [0u8; 8192]; // 8KB buffer

            // Seek backwards to find the position where the last 150 lines start
            // We need to find 149 newlines to get to the start of the 150th line from the end
            while position > 0 && newline_count < 149 {
                let chunk_size = std::cmp::min(buffer.len() as u64, position);
                position -= chunk_size;

                file.seek(SeekFrom::Start(position))?;
                file.read_exact(&mut buffer[0..chunk_size as usize])?;

                // Count newlines backwards in this chunk
                for i in (0..chunk_size as usize).rev() {
                    if buffer[i] == b'\n' {
                        newline_count += 1;
                        if newline_count == 149 {
                            // Found the position where the 150th line from the end starts
                            position += i as u64 + 1;
                            break;
                        }
                    }
                }
            }

            // If we reached the beginning and haven't found 149 newlines, start from the beginning
            if position == 0 && newline_count < 149 {
                0
            } else {
                position
            }
        }
    };

    // Start reading from the calculated position (this will print last 150 lines + any new content)
    file.seek(SeekFrom::Start(start_position))?;
    let mut reader = BufReader::new(file);

    // Set initial timestamp
    let mut last_updated = Zoned::now().round(Unit::Second).expect(
        "[FATAL] Could not get date/time information! Won't be able to compare times, so exiting.",
    );

    // Continuously read new lines
    // Note: Times out after set time without new bytes read
    loop {
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;

        let time_now = Zoned::now().round(Unit::Second).expect(
            "[FATAL] Could not get date/time information! Won't be able to compare times, so exiting.",
        );

        if bytes_read > 0 {
            // Print any new lines
            print!("{}", line);
            last_updated = time_now.clone();
        } else if last_updated
            .until((Unit::Second, &time_now))
            .expect("Error while comparing times! Exiting.")
            .get_seconds()
            > timeout
        {
            println!(
                "[WARNING] Timed out after {} seconds with no new bytes read! Exiting.",
                timeout
            );
            return Err("Timeout while monitoring - no new bytes read".into());
        } else {
            // No new data, wait a bit
            sleep(Duration::from_secs(1));
        }
    }
}

// Function responsible for saving to a tiny file (somewhere) that allows resuming a given tail
fn save_turd(project_dir: &Path, log_path: &Path) {
    let turd_path: PathBuf = project_dir.to_path_buf().join("._slurmtail");

    let mut file = File::create(turd_path.as_path()).unwrap_or_else(|_| {
        panic!(
            "[FATAL] Could not write resume file to: {:?}",
            turd_path.clone().to_str()
        )
    });

    // KISS: Just store the log file path
    let turd_message: &str = log_path
        .to_str()
        .expect("[FATAL] Could not turn log path into path during resume file creation! Exiting.");

    file.write_all(turd_message.as_bytes())
        .expect("[FATAL] Could not write resume file! Exiting.");
}

// Searches a project directory for a resume marker and returns the path of the logfile if it finds it (by reading the resume marker, which contains the path). Also verifies the logfile exists.
fn read_turd(project_dir: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let turd_path: PathBuf = project_dir.to_path_buf().join("._slurmtail");

    if !turd_path.exists() {
        return Err("No resume file found".into());
    }

    let content = read_to_string(&turd_path)?;
    let log_path = PathBuf::from(content.trim());

    if !log_path.exists() {
        return Err("Log file from resume file no longer exists".into());
    }

    Ok(log_path)
}

// Remove resume file if it exists
fn clean_turd(project_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let turd_path: PathBuf = project_dir.to_path_buf().join("._slurmtail");

    if turd_path.exists() {
        std::fs::remove_file(&turd_path)?;
        println!("Removed resume file: {:?}", turd_path);
    } else {
        println!("No resume file found to clean");
    }

    Ok(())
}

// Read the batch file and extract the log output pattern (in SLURM batch file format)
// e.g.: #SBATCH --output output.%j.log
//       -> "output.%j.log"
fn extract_log_output_pattern(script_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let content = read_to_string(script_path)?;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("#SBATCH --output") || line.starts_with("#SBATCH -o") {
            // Handle both "--output=value" and "--output value" formats
            if line.contains('=') {
                if let Some(output_part) = line.split('=').nth(1) {
                    return Ok(output_part.to_string());
                }
            } else if let Some(output_part) = line.split_whitespace().nth(2) {
                return Ok(output_part.to_string());
            }
        }
    }

    Err("No SBATCH output directive found in script".into())
}

// Extract job name from SLURM script
fn extract_job_name(script_path: &Path) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let content = read_to_string(script_path)?;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("#SBATCH --job-name") || line.starts_with("#SBATCH -J") {
            // Handle both "--job-name=value" and "--job-name value" formats
            if line.contains('=') {
                if let Some(job_name_part) = line.split('=').nth(1) {
                    return Ok(Some(job_name_part.to_string()));
                }
            } else if let Some(job_name_part) = line.split_whitespace().nth(2) {
                return Ok(Some(job_name_part.to_string()));
            }
        }
    }

    Ok(None)
}

// Take a SLURM-formatted output path and format it using a known jobid and optional job name
fn format_log_output_string(
    logfile_pattern_string: String,
    jobid: u64,
    job_name: Option<&String>,
) -> String {
    let mut result = logfile_pattern_string.replace("%j", &jobid.to_string());

    if let Some(name) = job_name {
        result = result.replace("%x", name);
    }

    result
}

// Take a now fully formed logfile path and transform it into a full path based on the location of the original script
fn logfile_string_to_path(
    script_path: &Path,
    logfile_string: String,
    use_cwd: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let base_dir: PathBuf = match use_cwd {
        true => env::current_dir().expect("Could not get current working directory! Exiting."),
        false => script_path.parent().unwrap_or(Path::new(".")).to_path_buf(),
    };

    // Handle given absolute path
    let log_path = if Path::new(&logfile_string).is_absolute() {
        if use_cwd {
            println!(
                "[WARNING] Gave instruction to use current directory to find logfile, but the logfile is an absolute path! Will use that instead."
            );
        }
        PathBuf::from(logfile_string)
    } else {
        base_dir.join(logfile_string)
    };

    Ok(log_path)
}

// Submit a job using sbatch
fn run_sbatch(script_path: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let output = ProcessCommand::new("sbatch")
        .arg(script_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("sbatch failed: {}", stderr).into());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    for word in stdout.split_whitespace() {
        if let Ok(job_id) = word.parse::<u64>() {
            return Ok(job_id);
        }
    }

    Err("Could not extract job ID from sbatch output".into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("slurmtail")
        .about("Submit SLURM jobs and monitor their log files")
        .subcommand(
            Command::new("run")
                .about("Run a SLURM batch script and monitor its output")
                .arg(
                    Arg::new("script")
                        .help("Path to the SLURM batch script")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::new("timeout")
                        .help("Timeout in seconds (default: 120)")
                        .short('t')
                        .long("timeout")
                        .value_parser(clap::value_parser!(u32)),
                )
                .arg(
                    Arg::new("no-file-timeout")
                        .help("Disable timeout for file appearance")
                        .short('n')
                        .long("no-file-timeout")
                        .action(clap::ArgAction::SetTrue),
                ),
        )
        .subcommand(
            Command::new("resume")
                .about("Resume monitoring a previously started job")
                .alias("r")
                .arg(
                    Arg::new("timeout")
                        .help("Timeout in seconds (default: 120)")
                        .short('t')
                        .long("timeout")
                        .value_parser(clap::value_parser!(u32)),
                )
                .arg(
                    Arg::new("no-file-timeout")
                        .help("Disable timeout for file appearance")
                        .short('n')
                        .long("no-file-timeout")
                        .action(clap::ArgAction::SetTrue),
                ),
        )
        .subcommand(
            Command::new("clean")
                .about("Remove any existing resume files")
                .alias("c"),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("run", sub_matches)) => {
            let script_path = Path::new(sub_matches.get_one::<String>("script").unwrap());
            let timeout = sub_matches.get_one::<u32>("timeout").copied();
            let no_file_timeout = sub_matches.get_flag("no-file-timeout");

            if !script_path.exists() {
                eprintln!("Error: Script file does not exist: {:?}", script_path);
                std::process::exit(1);
            }

            // Extract log output pattern from the script
            let log_pattern = extract_log_output_pattern(script_path)?;

            // Extract job name if present
            let job_name = extract_job_name(script_path)?;

            // Submit the job
            println!("Submitting job...");
            let job_id = run_sbatch(script_path)?;
            println!("Job submitted with ID: {}", job_id);

            // Format the log file path
            let log_filename = format_log_output_string(log_pattern, job_id, job_name.as_ref());
            let log_path = logfile_string_to_path(script_path, log_filename, true)?;
            println!(
                "[DEBUG] Will try to use {} as logfile path.",
                log_path.to_path_buf().to_str().unwrap()
            );

            // Save resume file
            let current_dir = env::current_dir()?;
            save_turd(&current_dir, &log_path);

            // Start monitoring
            println!("Monitoring log file: {:?}", log_path);
            mon_logfile(&log_path, timeout, timeout, no_file_timeout)?;
        }
        Some(("resume", sub_matches)) => {
            let timeout = sub_matches.get_one::<u32>("timeout").copied();
            let no_file_timeout = sub_matches.get_flag("no-file-timeout");
            let current_dir = env::current_dir()?;

            match read_turd(&current_dir) {
                Ok(log_path) => {
                    println!("Resuming monitoring of: {:?}", log_path);
                    mon_logfile(&log_path, timeout, timeout, no_file_timeout)?;
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(("clean", _)) => {
            let current_dir = env::current_dir()?;
            clean_turd(&current_dir)?;
        }
        _ => {
            eprintln!("Use 'slurmtail run <script>', 'slurmtail resume', or 'slurmtail clean'");
            std::process::exit(1);
        }
    }

    Ok(())
}

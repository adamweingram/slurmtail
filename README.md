# slurmtail

A simple utility that submits jobs to SLURM and immediately starts monitoring the resulting log file in a `tail -f --retry`-like fashion.

## Why?
Because I kept finding myself writing small scripts to help me monitor SLURM jobs as they run while doing debugging on clusters.

## Features

- **Submit & Monitor**: Submit SLURM batch jobs and automatically start tailing their output logs.
- **Resume Capability**: Resume monitoring a previously submitted job's log file.
- **Smart Log Detection**: Automatically extracts log file patterns from SLURM batch scripts (though this is a bit janky ATM).
- **Timeout Handling**: Configurable timeout for both file creation and monitoring.
- **Last 150 Lines**: Shows the last 150 lines when starting to monitor an existing log file. (In the future, this will probably be configurable too)

## Installation

1. Clone the repo:
  ```bash
  git clone https://github.com/adamweingram/slurmtail.git
  ```
2. Install with `cargo`:
  ```bash
  cargo install --path ./slurmtail
  ```

The binary will automatically be added to your PATH (`cargo` handles this). When you pull a new update and run the `cargo` command again, it automatically updates in your PATH as well.

If you don't like using `cargo` like this, you can simply run `cargo build --release` and then `cp` the binary at `target/release/slurmtail` to your `~/.local/bin` (or wherever).

## Usage

### Submit and Monitor a Job

```bash
slurmtail run <script.sh>
# or
slurmtail r <script.sh>
```

This will:
1. Submit your SLURM batch script using `sbatch`.
2. Extract the log output pattern from the script (e.g., `#SBATCH --output output.%j.log`).
3. Wait for the log file to be created (or until TIMEOUT seconds).
4. Start monitoring the log file, showing new content as it's written.
5. Create a hidden resume file (`._slurmtail`) for later resumption.

### Resume Monitoring

```bash
slurmtail resume
# or
slurmtail m
```

Resume monitoring a previously submitted job using the stored resume file.

### Clean Resume Files

```bash
slurmtail clean
# or
slurmtail c
```

Remove any existing resume files from the current directory.

## SLURM Script Requirements

Your SLURM batch script **must include an output directive**, such as:

```bash
#!/usr/bin/env bash
#SBATCH --output=output.%j.log
# or
#SBATCH -o output.%j.log

# Your job commands here
```

> [!NOTE]
> The `%j` and `%x` placeholders will be automatically replaced with the actual job ID and job name, respectively.

## Options
- `--timeout, -t`: Timeout in seconds for waiting for log file creation or monitoring inactivity (default: 120)
- `--no-file-timeout, -n`: Disable timeout for waiting for the log file to appear (will wait indefinitely)
- `--no-bytes-timeout, -n`: Disable timeout for waiting for new bytes to be written to the SLURM output file (will wait indefinitely)

For others, see `slurmtail --help`.

## Examples

```bash
# Submit a job and monitor its output
slurmtail run my_job.sh

# Submit with a longer timeout
slurmtail run --timeout 300 my_job.sh

# Submit without file timeout (wait indefinitely for log file)
slurmtail run --no-file-timeout my_job.sh

# Submit without file timeout (wait indefinitely new bytes to be written, at least once the file appears)
slurmtail run --no-bytes-timeout my_job.sh

# Resume monitoring a previous job
slurmtail resume

# Resume without file timeout
slurmtail resume --no-file-timeout

# Clean up resume files
slurmtail clean
```

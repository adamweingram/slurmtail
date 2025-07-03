# slurmtail

A simple utility that submits jobs to SLURM and immediately starts monitoring the resulting log file in a `tail -f --retry`-like fashion.

## Why?
Because I kept finding myself writing small scripts to help me monitor SLURM jobs as they run while doing debugging on clusters.

## Features

- **Submit & Monitor**: Submit SLURM batch jobs and automatically start tailing their output logs.
- **Resume Capability**: Resume monitoring a previously submitted job's log file.
- **Smart Log Detection**: Automatically extracts log file patterns from SLURM batch scripts (though this is a bit janky ATM).
- **Timeout Handling**: Configurable timeout for both file creation and monitoring.
- **Last 150 Lines**: Shows the last 150 lines when starting to monitor an existing log file.

## Installation

Build from source using Cargo (after cloning):

```bash
cargo build --release
```

The binary will be available at `target/release/slurmtail`. You can `cp` it to your `~/.local/bin` (or somewhere else in your `PATH`) for ease of use.

## Usage

### Submit and Monitor a Job

```bash
slurmtail run <script.sh> [--timeout SECONDS] [--no-file-timeout]
```

This will:
1. Submit your SLURM batch script using `sbatch`.
2. Extract the log output pattern from the script (e.g., `#SBATCH --output output.%j.log`).
3. Wait for the log file to be created (or until TIMEOUT seconds).
4. Start monitoring the log file, showing new content as it's written.
5. Create a hidden resume file (`._slurmtail`) for later resumption.

### Resume Monitoring

```bash
slurmtail resume [--timeout SECONDS] [--no-file-timeout]
# or
slurmtail r [--timeout SECONDS] [--no-file-timeout]
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

Your SLURM batch script must include an output directive, such as:

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

## Examples

```bash
# Submit a job and monitor its output
slurmtail run my_job.sh

# Submit with a longer timeout
slurmtail run my_job.sh --timeout 300

# Submit without file timeout (wait indefinitely for log file)
slurmtail run my_job.sh --no-file-timeout

# Resume monitoring a previous job
slurmtail resume

# Resume without file timeout
slurmtail resume --no-file-timeout

# Clean up resume files
slurmtail clean
```

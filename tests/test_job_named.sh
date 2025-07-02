#!/usr/bin/env bash
#SBATCH --job-name=slurmtail_test
#SBATCH --output=test_output.%x.%j.log
#SBATCH --time=00:01:00
#SBATCH --ntasks=1

echo "Test job started at $(date)"
sleep 5
echo "Test job processing..."
sleep 5
echo "Test job completed at $(date)"

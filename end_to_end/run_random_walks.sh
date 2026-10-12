#!/usr/bin/env bash

set -uo pipefail

iterations="${1:-10000}"
parallelism="${WALKER_JOBS:-1}"
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
project_root="$(cd -- "$script_dir/.." && pwd)"
failures_dir="$project_root/failures"
target_dir="${CARGO_TARGET_DIR:-$project_root/target}"

if [[ "$target_dir" != /* ]]; then
    target_dir="$project_root/$target_dir"
fi

if ! [[ "$iterations" =~ ^[1-9][0-9]*$ ]]; then
    echo "iteration count must be a positive integer" >&2
    exit 2
fi

if ! [[ "$parallelism" =~ ^[1-9][0-9]*$ ]]; then
    echo "WALKER_JOBS must be a positive integer" >&2
    exit 2
fi

mkdir -p -- "$failures_dir"

cd -- "$project_root"
cargo build --release -p end_to_end

binary="$target_dir/release/end_to_end"
if [[ ! -x "$binary" ]]; then
    echo "release binary was not created at $binary" >&2
    exit 1
fi

run_one() {
    local iteration="$1"
    local seed="$2"
    local run_dir
    local trace
    local destination
    local status

    run_dir="$(mktemp -d "${TMPDIR:-/tmp}/tiny-diplomacy-walk.XXXXXXXX")"
    trace="$run_dir/failure-trace-$seed.json"
    destination="$failures_dir/failure-trace-run-$iteration-seed-$seed.json"

    if (cd -- "$run_dir" && "$binary" "$seed" >/dev/null); then
        status=0
    else
        status=$?
    fi

    if [[ -f "$trace" ]]; then
        mv -- "$trace" "$destination"
        echo "run $iteration/$iterations (seed $seed) produced $destination" >&2
    elif (( status != 0 )); then
        echo "run $iteration/$iterations (seed $seed) exited with status $status without a trace" >&2
    fi

    if ! rmdir -- "$run_dir"; then
        echo "run $iteration/$iterations left unexpected files in $run_dir" >&2
    fi
}

running=0
for ((iteration = 1; iteration <= iterations; iteration++)); do
    seed="$(od -An -N8 -tu8 /dev/urandom | tr -d '[:space:]')"
    run_one "$iteration" "$seed" &
    ((running += 1))

    if (( running >= parallelism )); then
        wait -n || true
        ((running -= 1))
    fi
done

wait || true
echo "completed $iterations random walks; failure traces are in $failures_dir"

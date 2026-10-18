#!/usr/bin/env bash
# Runs every jade-office-perf scenario on the fixtures of its kind under
# callgrind.
#
#   scripts/perf/run-callgrind.sh <jade-office-perf binary> <out dir> <fixture>...
#
# A fixture is a file name under jade-core/public/fixtures; each scenario runs
# on the fixtures whose extension it takes (`jade-office-perf --list`). Writes
# <out dir>/<scenario>@<fixture>.out per run. Only the scenario's measured
# region is collected (see `perf_measured_region`), so opening the fixture
# does not count.
set -euo pipefail

bin=$1
out=$2
shift 2
fixtures=("$@")
fixture_dir="$(cd "$(dirname "$0")/../.." && pwd)/jade-core/public/fixtures"

mkdir -p "$out"
jobs=()
while read -r scenario kind; do
  for fixture in "${fixtures[@]}"; do
    if [ "${fixture##*.}" = "$kind" ]; then
      jobs+=("$scenario $fixture")
    fi
  done
done < <("$bin" --list)

run_one() {
  local scenario=$1 fixture=$2
  valgrind --tool=callgrind \
    --collect-atstart=no \
    --toggle-collect='*perf_measured_region*' \
    --callgrind-out-file="$out/$scenario@$fixture.out" \
    "$bin" "$scenario" "$fixture_dir/$fixture" \
    >"$out/$scenario@$fixture.log" 2>&1 || {
    echo "failed: $scenario $fixture" >&2
    tail -20 "$out/$scenario@$fixture.log" >&2
    return 1
  }
}
export -f run_one
export bin out fixture_dir

parallel=${PERF_JOBS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu)}
printf '%s\n' "${jobs[@]}" | xargs -P "$parallel" -L 1 bash -c 'run_one $0 $1'

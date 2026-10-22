#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TKPIR_DIR="$REPO_ROOT/tkpir"
RESULTS_DIR="$REPO_ROOT/results"
CSV_PATH="$RESULTS_DIR/table_ix.csv"

fail() {
  echo "Error: $*" >&2
  exit 1
}

require_tooling() {
  command -v cargo >/dev/null 2>&1 || fail "cargo is not available in PATH"
  [[ -d "$TKPIR_DIR" ]] || fail "expected benchmark crate at $TKPIR_DIR"
}

extract_scheme() {
  local file="$1"
  local value
  value="$(
    awk '
      index($0, "Scheme: ") == 1 {
        last = substr($0, length("Scheme: ") + 1)
      }
      END {
        if (last == "") {
          exit 1
        }
        print last
      }
    ' "$file"
  )" || fail "failed to parse Scheme field"
  printf '%s' "$value"
}

extract_avg_seconds() {
  local prefix="$1"
  local file="$2"
  local value
  value="$(
    awk -v prefix="$prefix" '
      index($0, prefix) == 1 {
        line = $0
        sub(/^.*: /, "", line)
        sub(/ s$/, "", line)
        if (line ~ /^[-+]?[0-9]+([.][0-9]+)?([eE][-+]?[0-9]+)?$/) {
          last = line
        }
      }
      END {
        if (last == "") {
          exit 1
        }
        print last
      }
    ' "$file"
  )" || fail "failed to parse seconds field with prefix: $prefix"
  printf '%s' "$value"
}

run_case() {
  local scheme="$1"
  local threads="$2"
  local log_m="$3"
  local data_bits="$4"
  local warmup_runs="$5"
  local measured_runs="$6"

  local tmp_output
  tmp_output="$(mktemp)"

  if ! (
    cd "$TKPIR_DIR"
    TKPIR_SCHEME="$scheme" \
    TKPIR_NUM_THREADS="$threads" \
    TKPIR_MAX_KEYWORD_BITLEN="$log_m" \
    TKPIR_MAX_DATA_BITLEN="$data_bits" \
    TKPIR_NUM_WARMUP_RUNS="$warmup_runs" \
    TKPIR_NUM_MEASURED_RUNS="$measured_runs" \
    cargo test --lib tests::test_tkpir --release -- --exact --nocapture
  ) 2>&1 | tee "$tmp_output"; then
    rm -f "$tmp_output"
    fail "benchmark command failed for scheme=$scheme log_m=$log_m data_bits=$data_bits"
  fi

  local parsed_scheme
  parsed_scheme="$(extract_scheme "$tmp_output")"
  if [[ "$parsed_scheme" != "$scheme" ]]; then
    rm -f "$tmp_output"
    fail "expected scheme '$scheme' but parsed '$parsed_scheme'"
  fi

  local avg_client_encode_s
  avg_client_encode_s="$(extract_avg_seconds "Avg client-side encode time (" "$tmp_output")"
  local avg_client_decode_s
  avg_client_decode_s="$(extract_avg_seconds "Avg client-side decode time (" "$tmp_output")"
  local avg_client_s
  avg_client_s="$(extract_avg_seconds "Avg client-side time (" "$tmp_output")"

  printf '%s,%s,%s,%s,%s,%s,%s,%s,%s\n' \
    "$scheme" "$threads" "$log_m" "$data_bits" "$warmup_runs" "$measured_runs" \
    "$avg_client_encode_s" "$avg_client_decode_s" "$avg_client_s" >>"$CSV_PATH"

  rm -f "$tmp_output"
}

main() {
  require_tooling
  mkdir -p "$RESULTS_DIR"

  printf '%s\n' \
    "scheme,threads,log_m,data_bits,warmup_runs,measured_runs,avg_client_side_encode_time_s,avg_client_side_decode_time_s,avg_client_side_time_s" \
    >"$CSV_PATH"

  local warmup_runs=0
  local measured_runs=1

  for scheme in TCWKPIR TCAKPIR TCAKPIR_OPT; do
    for log_m in 12 14 16; do
      for data_bits in 64 1024; do
        run_case "$scheme" 64 "$log_m" "$data_bits" "$warmup_runs" "$measured_runs"
      done
    done
  done

  echo "Wrote $CSV_PATH"
}

main "$@"

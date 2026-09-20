#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TKPIR_DIR="$REPO_ROOT/tkpir"
RESULTS_DIR="$REPO_ROOT/results"
CSV_PATH="$RESULTS_DIR/table_x.csv"

fail() {
  echo "Error: $*" >&2
  exit 1
}

usage() {
  echo "Usage: ./scripts/reproduce_table_x.sh --quick|--full" >&2
}

require_tooling() {
  command -v cargo >/dev/null 2>&1 || fail "cargo is not available in PATH"
  [[ -d "$TKPIR_DIR" ]] || fail "expected benchmark crate at $TKPIR_DIR"
}

parse_rows() {
  local output_file="$1"
  local mode="$2"
  local scheme="$3"
  local eval_function="$4"
  local log_m="$5"
  local data_bits="$6"
  local threads="$7"
  local warmup_runs="$8"
  local measured_runs="$9"

  awk \
    -v mode="$mode" \
    -v scheme="$scheme" \
    -v eval_function="$eval_function" \
    -v log_m="$log_m" \
    -v data_bits="$data_bits" \
    -v threads="$threads" \
    -v warmup_runs="$warmup_runs" \
    -v measured_runs="$measured_runs" '
      function die(msg) {
        print "Error: " msg > "/dev/stderr"
        exit 1
      }

      function parse_token(token, prefix, suffix,    value) {
        if (index(token, prefix) != 1) {
          return ""
        }
        value = substr(token, length(prefix) + 1)
        if (suffix != "") {
          if (length(value) <= length(suffix)) {
            return ""
          }
          if (substr(value, length(value) - length(suffix) + 1) != suffix) {
            return ""
          }
          value = substr(value, 1, length(value) - length(suffix))
        }
        if (value !~ /^[-+]?[0-9]+([.][0-9]+)?([eE][-+]?[0-9]+)?$/) {
          return ""
        }
        return value
      }

      BEGIN {
        pending_t = ""
        parsed_rows = 0
      }

      {
        if ($0 ~ /^=== / && $0 ~ / \| t=[0-9]+ \| / && $0 ~ / \| m=2\^[0-9]+/) {
          line = $0
          sub(/^.* \| t=/, "", line)
          if (line == $0) {
            die("failed to locate t token in header: " $0)
          }
          sub(/ \|.*$/, "", line)
          if (line !~ /^[0-9]+$/) {
            die("failed to parse numeric t from header: " $0)
          }
          pending_t = line
          next
        }

        if (index($0, "response=") == 1) {
          if (pending_t == "") {
            die("summary line without preceding t header: " $0)
          }
          if (NF < 9) {
            die("unexpected summary format: " $0)
          }

          response_s = parse_token($1, "response=", "s")
          default_s = parse_token($2, "default=", "s")
          evaluate_s = parse_token($3, "evaluate=", "s")
          total_s = parse_token($4, "total=", "s")
          upload_kb = parse_token($5, "upload=", "KB")
          download_kb = parse_token($6, "download=", "KB")
          recovered_result = parse_token($8, "recovered=", "")
          expected_result = parse_token($9, "expected=", "")

          if (response_s == "" || default_s == "" || evaluate_s == "" || total_s == "" || upload_kb == "" || download_kb == "" || recovered_result == "" || expected_result == "") {
            die("failed to parse one or more summary tokens: " $0)
          }

          if (index($7, "correct=") != 1) {
            die("missing correct token: " $0)
          }
          correct = substr($7, length("correct=") + 1)
          if (!(correct == "true" || correct == "false")) {
            die("unexpected correctness value: " correct)
          }

          printf "%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s\n",
            mode,
            scheme,
            eval_function,
            log_m,
            data_bits,
            threads,
            warmup_runs,
            measured_runs,
            pending_t,
            response_s,
            default_s,
            evaluate_s,
            total_s,
            upload_kb,
            download_kb,
            correct,
            recovered_result,
            expected_result

          parsed_rows++
          pending_t = ""
        }
      }

      END {
        if (parsed_rows == 0) {
          die("no summary rows were parsed")
        }
        if (parsed_rows != 5) {
          die("expected 5 summary rows (t in {1,2,4,8,16}), parsed " parsed_rows)
        }
        if (pending_t != "") {
          die("missing summary line after header for t=" pending_t)
        }
      }
    ' "$output_file" >>"$CSV_PATH"
}

run_case() {
  local mode="$1"
  local scheme="$2"
  local eval_function="$3"
  local log_m="$4"
  local data_bits="$5"
  local threads="$6"
  local warmup_runs="$7"
  local measured_runs="$8"

  local tmp_output
  tmp_output="$(mktemp)"

  local skip_args=()
  if [[ "$mode" == "quick" ]]; then
    skip_args=(--skip-kpir)
  fi

  if ! (
    cd "$TKPIR_DIR"
    TKPIR_NUM_THREADS="$threads" \
    TKPIR_MAX_KEYWORD_BITLEN="$log_m" \
    TKPIR_MAX_DATA_BITLEN="$data_bits" \
    TKPIR_NUM_WARMUP_RUNS="$warmup_runs" \
    TKPIR_NUM_MEASURED_RUNS="$measured_runs" \
    cargo run --release --bin bench_kpir_c_multi -- \
      --scheme "$scheme" \
      --function "$eval_function" \
      "${skip_args[@]}"
  ) 2>&1 | tee "$tmp_output"; then
    rm -f "$tmp_output"
    fail "benchmark command failed for scheme=$scheme function=$eval_function log_m=$log_m data_bits=$data_bits mode=$mode"
  fi

  parse_rows "$tmp_output" "$mode" "$scheme" "$eval_function" "$log_m" "$data_bits" "$threads" "$warmup_runs" "$measured_runs"

  rm -f "$tmp_output"
}

main() {
  if [[ $# -ne 1 ]]; then
    usage
    exit 2
  fi

  local mode
  case "$1" in
    --quick)
      mode="quick"
      ;;
    --full)
      mode="full"
      ;;
    *)
      usage
      exit 2
      ;;
  esac

  require_tooling
  mkdir -p "$RESULTS_DIR"

  printf '%s\n' \
    "mode,scheme,function,log_m,data_bits,threads,warmup_runs,measured_runs,t,response_s,default_s,evaluate_s,total_s,upload_kb,download_kb,correct,recovered_result,expected_result" \
    >"$CSV_PATH"

  local warmup_runs=0
  local measured_runs=1

  for log_m in 12 16; do
    for data_bits in 64 1024; do
      for scheme in TCWKPIR TCAKPIR TCAKPIR_OPT; do
        for eval_function in sum threshold; do
          run_case "$mode" "$scheme" "$eval_function" "$log_m" "$data_bits" 64 "$warmup_runs" "$measured_runs"
        done
      done
    done
  done

  echo "Wrote $CSV_PATH"
}

main "$@"

# Artifact Evaluation

## Requirements

### Software

* Stable Rust toolchain with `cargo`
* TFHE-rs v0.10.0, as used by this implementation
* Linux recommended; macOS supported
  * Tested on Ubuntu 22.04.1 LTS (`x86_64`)
  * Tested on macOS 26.5.2, Apple Silicon (`arm64`)
* The helper scripts under `scripts/` require a Bash-compatible shell; the benchmark commands can also be run manually with environment-variable syntax adapted to the user's shell

### Hardware

The required resources depend on the experiment:

* **Quick check:** 8 GB RAM is expected to be sufficient for the `m = 2^12`, `8 B` validation runs. These experiments use 1 CPU thread.

* **Tables IV, V, and IX:** 8 GB RAM is expected to be sufficient. These experiments use 1 CPU thread and evaluate databases up to `m = 2^16`.

* **Table VII:** 32 GB RAM is recommended for the largest configurations (`m = 2^20`). The reported runtimes use 64 CPU threads.

* **Table X:** 8 GB RAM is expected to be sufficient. The reported multi-query runtimes use 64 CPU threads and evaluate `m = 2^12` and `m = 2^16`.

* **Figures 5 and 6:** 8 GB RAM is sufficient. These routines compute memory and communication estimates and do not materialize the largest selection vectors shown in the figures.

Runtime results depend on CPU performance and the number of available CPU threads. The 64-thread experiments can be executed on machines with fewer CPU threads, but their runtimes may differ from those reported in the paper. Correctness and communication results are unaffected.

## Quick Check

This section reproduces the `m = 2^12`, `8 B` configurations reported in Tables IV and V for the three schemes implemented in this repository. It is intended for evaluators with limited time or computational resources who want to quickly validate representative configurations before running the full experiments.

To keep validation short, each configuration is executed once with no warmup runs. For more stable runtime measurements consistent with the paper, use one warmup run and 10 measured runs as described under Full Reproduction. Runtime may vary across systems, while correctness and communication values should be directly reproducible.

### Using the helper script

On systems with Bash, run the quick validation from the repository root:

```bash
bash scripts/quick_check.sh
```

The script runs the three configurations below and writes the extracted results to:

```text
results/quick_check.csv
```

### Running the commands manually

If the helper script is not supported in your environment, run the corresponding commands directly. The examples below use Bash-style environment-variable assignment; adapt the syntax as needed for your shell.

From the repository root:

```bash
cd tkpir
```

#### TCWKPIR

```bash
TKPIR_SCHEME=TCWKPIR \
TKPIR_NUM_THREADS=1 \
TKPIR_MAX_KEYWORD_BITLEN=12 \
TKPIR_MAX_DATA_BITLEN=64 \
TKPIR_NUM_WARMUP_RUNS=0 \
TKPIR_NUM_MEASURED_RUNS=1 \
cargo test --lib tests::test_tkpir --release -- --exact --nocapture
```

#### TCAKPIR

```bash
TKPIR_SCHEME=TCAKPIR \
TKPIR_NUM_THREADS=1 \
TKPIR_MAX_KEYWORD_BITLEN=12 \
TKPIR_MAX_DATA_BITLEN=64 \
TKPIR_NUM_WARMUP_RUNS=0 \
TKPIR_NUM_MEASURED_RUNS=1 \
cargo test --lib tests::test_tkpir --release -- --exact --nocapture
```

#### TCAKPIR+

`TCAKPIR_OPT` is the implementation name for `TCAKPIR+`.

```bash
TKPIR_SCHEME=TCAKPIR_OPT \
TKPIR_NUM_THREADS=1 \
TKPIR_MAX_KEYWORD_BITLEN=12 \
TKPIR_MAX_DATA_BITLEN=64 \
TKPIR_NUM_WARMUP_RUNS=0 \
TKPIR_NUM_MEASURED_RUNS=1 \
cargo test --lib tests::test_tkpir --release -- --exact --nocapture
```

Each run should finish with:

```text
test tests::test_tkpir ... ok
```

The output also reports runtime and communication costs. The corresponding paper values are:

#### Table IV

| Scheme  | SV (s) | IP (s) | Total (s) | Upload (KB) | Download (KB) |
| ------- | -----: | -----: | --------: | ----------: | ------------: |
| TCWKPIR |  61.21 |   4.02 |     65.84 |        8.27 |          1.68 |
| TCAKPIR |   0.03 |   0.51 |      2.32 |          44 |          1.73 |

#### Table V

| Scheme   | Offline (s) | Online (s) | Total (s) | Upload (KB) | Download (KB) |
| -------- | ----------: | ---------: | --------: | ----------: | ------------: |
| TCAKPIR+ |        1.01 |       1.17 |      2.18 |          44 |          1.73 |

`TCAKPIR` appears in both Tables IV and V, so no additional run is required. The external `CWKPIR` and `ChalametPIR` baselines are not included in this repository.

## Full Reproduction

This section describes how to run the reported experiments for the schemes implemented in this repository. By default, the helper scripts and commands use no warmup and one measured run for faster validation:

```text
TKPIR_NUM_WARMUP_RUNS=0
TKPIR_NUM_MEASURED_RUNS=1
```

For more stable runtime measurements consistent with the paper, use:

```text
TKPIR_NUM_WARMUP_RUNS=1
TKPIR_NUM_MEASURED_RUNS=10
```

Set these values in the corresponding helper script or manual command before running the experiment.

### Using the helper scripts

On systems with Bash, the reported experiments can be reproduced from the repository root with:

```bash
bash scripts/reproduce_table_iv.sh
bash scripts/reproduce_table_v.sh
bash scripts/reproduce_table_vii.sh
bash scripts/reproduce_table_x.sh --quick
```

For Table X, `--quick` is the recommended reproduction mode because Table X does not report the KPIR `Response` time. This mode skips the `Response` stage and reproduces the reported default-handling time, `Evaluate` time, download cost, and correctness measurements.

To additionally execute the complete KPIR-C protocol, use:

```bash
bash scripts/reproduce_table_x.sh --full
```

The scripts preserve benchmark output in the terminal and also write structured CSV files under `results/`.

If the helper scripts are not supported in your environment, use the manual commands and configuration tables below.

### 1) Single-Query KPIR Benchmarks — Tables IV, V, VII, and IX

Entrypoint: `tests::test_tkpir` in `tkpir/src/lib.rs`.

Use the following command template:

```bash
cd tkpir

TKPIR_SCHEME=<SCHEME> \
TKPIR_NUM_THREADS=<THREADS> \
TKPIR_MAX_KEYWORD_BITLEN=<LOG_M> \
TKPIR_MAX_DATA_BITLEN=<DATA_BITS> \
TKPIR_NUM_WARMUP_RUNS=0 \
TKPIR_NUM_MEASURED_RUNS=1 \
cargo test --lib tests::test_tkpir --release -- --exact --nocapture
```

The example above uses Bash-style environment-variable assignment; adapt the syntax as needed for your shell.

Example (`TCAKPIR`, `m = 2^12`, `8 B`, Table IV):

```bash
cd tkpir

TKPIR_SCHEME=TCAKPIR \
TKPIR_NUM_THREADS=1 \
TKPIR_MAX_KEYWORD_BITLEN=12 \
TKPIR_MAX_DATA_BITLEN=64 \
TKPIR_NUM_WARMUP_RUNS=0 \
TKPIR_NUM_MEASURED_RUNS=1 \
cargo test --lib tests::test_tkpir --release -- --exact --nocapture
```

Use the following configurations:

| Table | `SCHEME`                            | `THREADS` | `LOG_M`          | `DATA_BITS`           |
| ----- | ----------------------------------- | --------: | ---------------- | --------------------- |
| IV    | `TCWKPIR`, `TCAKPIR`                |         1 | `12`, `14`, `16` | `64`, `1024`          |
| V     | `TCAKPIR`, `TCAKPIR_OPT`            |         1 | `12`, `16`       | `64`, `1024`          |
| VII   | `TCWKPIR`, `TCAKPIR`, `TCAKPIR_OPT` |        64 | `16`, `18`, `20` | `512`, `1024`, `2048` |
| IX    | `TCWKPIR`, `TCAKPIR`, `TCAKPIR_OPT` |         1 | `12`, `14`, `16` | `64`, `1024`          |

`LOG_M = x` corresponds to `m = 2^x`. `DATA_BITS` is the entry size in bits:

* `64` = `8 B`
* `512` = `64 B`
* `1024` = `128 B`
* `2048` = `256 B`

Table IX uses the client-side timings produced by the corresponding single-query runs:

* `Avg client-side encode time` corresponds to Query
* `Avg client-side decode time` corresponds to Recover
* `Avg client-side time` corresponds to Total

Separate executions are therefore not required when those configurations have already been run.

### 2) KPIR-C Multi-Query Benchmark — Table X

Entrypoint: `bench_kpir_c_multi` in `tkpir/src/bin/bench_kpir_c_multi.rs`.

Table X reports default-handling time, `Evaluate` time, download cost, and correctness, but does not include the KPIR `Response` time. Therefore, the recommended reproduction mode skips the `Response` stage using `--skip-kpir`; this corresponds to the helper script's `--quick` mode.

Use the following command template:

```bash
cd tkpir

TKPIR_NUM_THREADS=64 \
TKPIR_MAX_KEYWORD_BITLEN=<LOG_M> \
TKPIR_MAX_DATA_BITLEN=<DATA_BITS> \
TKPIR_NUM_WARMUP_RUNS=0 \
TKPIR_NUM_MEASURED_RUNS=1 \
cargo run --release --bin bench_kpir_c_multi -- \
  --scheme <SCHEME> \
  --function <FUNCTION> \
  --skip-kpir
```

To additionally execute the complete KPIR-C protocol, omit `--skip-kpir`; this corresponds to the helper script's `--full` mode.

The example above uses Bash-style environment-variable assignment; adapt the syntax as needed for your shell.

Example (`TCAKPIR`, `m = 2^12`, `8 B`, `sum`):

```bash
cd tkpir

TKPIR_NUM_THREADS=64 \
TKPIR_MAX_KEYWORD_BITLEN=12 \
TKPIR_MAX_DATA_BITLEN=64 \
TKPIR_NUM_WARMUP_RUNS=0 \
TKPIR_NUM_MEASURED_RUNS=1 \
cargo run --release --bin bench_kpir_c_multi -- \
  --scheme TCAKPIR \
  --function sum \
  --skip-kpir
```

Use the following values:

| Parameter   | Values                              |
| ----------- | ----------------------------------- |
| `LOG_M`     | `12`, `16`                          |
| `DATA_BITS` | `64`, `1024`                        |
| `SCHEME`    | `TCWKPIR`, `TCAKPIR`, `TCAKPIR_OPT` |
| `FUNCTION`  | `sum`, `threshold`                  |

The benchmark evaluates `t ∈ {1, 2, 4, 8, 16}` automatically and reports runtime, communication, and correctness results in the terminal. The individual runs produce the measurements underlying Table X; the table reports averages across the configurations described in the paper.

### 3) Additional Evaluations

The following routines generate the data for our schemes used in Figures 5 and 6. External baseline data shown in these figures are not generated by this repository.

#### Figure 5 — Selection-Vector Memory

```bash
cd tkpir

cargo test tests::test_selection_vector_memory_costs \
  --release -- --ignored --exact --nocapture
```

This generates the server-side selection-vector memory estimates for `TCWKPIR`, `TCAKPIR`, and `TCAKPIR+`.

Outputs are written to:

* `results/sv_memory/selection_vector_memory_tcwkpir.csv`
* `results/sv_memory/selection_vector_memory_tcakpir.csv`
* `results/sv_memory/selection_vector_memory_tcakpir_opt.csv`

#### Figure 6 — Communication Costs

```bash
cd tkpir

cargo test tests::test_communication_costs \
  --release -- --ignored --exact --nocapture
```

This generates the communication-cost data for `TCWKPIR` and `TCAKPIR`.

Outputs are written to:

* `results/communication_costs/tcwkpir_comm_costs.csv`
* `results/communication_costs/tcakpir_comm_costs.csv`

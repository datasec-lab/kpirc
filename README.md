# KPIR-C: Keyword PIR with Arbitrary Server-side Computation

This repository provides the implementation and evaluation artifacts for our paper **“KPIR-C: Keyword PIR with Arbitrary Server-side Computation,”** accepted at **NDSS 2027**.

## Contents

* [Repository Layout](#repository-layout)
* [Requirements](#requirements)
* [Installation and Build](#installation-and-build)
* [Artifact Evaluation](#artifact-evaluation)
* [Configuration](#configuration)
* [Running Single-Query KPIR](#running-single-query-kpir)
* [Running Multi-Query KPIR-C](#running-multi-query-kpir-c)
* [Citation and License](#citation-and-license)

## Repository Layout

```text
.
├── README.md
├── .gitignore
├── artifact/
│   └── README.md
├── scripts/
├── results/
└── tkpir/
    ├── Cargo.toml
    ├── Cargo.lock
    ├── benches/bench.rs
    ├── common_tool/
    └── src/
        ├── lib.rs
        └── bin/
            └── bench_kpir_c_multi.rs
```

## Requirements

* Stable Rust toolchain with `cargo`
* Linux is recommended; macOS is also supported
  * Tested on Ubuntu 22.04.1 LTS (`x86_64`) with stable Rust/Cargo and TFHE-rs v0.10.0
  * Tested on macOS 26.5.2, Apple Silicon (`arm64`), with Rust/Cargo 1.97.1
* 8 GB RAM is expected to be sufficient for databases of size `m ≤ 2^16`; at least 32 GB RAM is recommended for `m > 2^16`

## Installation and Build

Install Rust if needed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
```

Then, from the repository root, build the artifact:

```bash
cargo build --manifest-path tkpir/Cargo.toml --release --all-targets
```

## Artifact Evaluation

Detailed instructions for validating and reproducing the paper's experimental results are provided in [`artifact/README.md`](artifact/README.md).

For a quick validation, run from the repository root:

```bash
bash scripts/quick_check.sh
```

The full reproduction scripts are:

```bash
bash scripts/reproduce_table_iv.sh
bash scripts/reproduce_table_v.sh
bash scripts/reproduce_table_vii.sh
bash scripts/reproduce_table_x.sh --quick
```

The helper scripts are convenience wrappers around the benchmark commands described below. See [`artifact/README.md`](artifact/README.md) for experiment configurations, expected outputs, resource requirements, and paper results.

## Configuration

The benchmark entrypoints use the following environment variables:

* `TKPIR_SCHEME`: `TCWKPIR`, `TCAKPIR`, or `TCAKPIR_OPT`
* `TKPIR_NUM_THREADS`: number of threads
* `TKPIR_MAX_KEYWORD_BITLEN`: keyword bit length, where `x` corresponds to `m = 2^x` database entries
* `TKPIR_MAX_DATA_BITLEN`: entry size in bits
* `TKPIR_NUM_WARMUP_RUNS`: number of warmup runs (default: `0`)
* `TKPIR_NUM_MEASURED_RUNS`: number of measured runs (default: `1`)

By default, benchmarks use no warmup and one measured run for fast validation. For more stable runtime measurements consistent with the paper, set:

```text
TKPIR_NUM_WARMUP_RUNS=1
TKPIR_NUM_MEASURED_RUNS=10
```

`TCAKPIR_OPT` is the implementation name for `TCAKPIR+`.

## Running Single-Query KPIR

The single-query benchmark entrypoint is `tests::test_tkpir` in `tkpir/src/lib.rs`.

From `tkpir/`, use:

```bash
TKPIR_SCHEME=<SCHEME> \
TKPIR_NUM_THREADS=<THREADS> \
TKPIR_MAX_KEYWORD_BITLEN=<LOG_M> \
TKPIR_MAX_DATA_BITLEN=<DATA_BITS> \
TKPIR_NUM_WARMUP_RUNS=<WARMUP_RUNS> \
TKPIR_NUM_MEASURED_RUNS=<MEASURED_RUNS> \
cargo test --lib tests::test_tkpir --release -- --exact --nocapture
```

For example, to run `TCAKPIR` with `m = 2^12`, 8-byte entries, and one thread:

```bash
TKPIR_SCHEME=TCAKPIR \
TKPIR_NUM_THREADS=1 \
TKPIR_MAX_KEYWORD_BITLEN=12 \
TKPIR_MAX_DATA_BITLEN=64 \
TKPIR_NUM_WARMUP_RUNS=0 \
TKPIR_NUM_MEASURED_RUNS=1 \
cargo test --lib tests::test_tkpir --release -- --exact --nocapture
```

Supported schemes are:

```text
TCWKPIR
TCAKPIR
TCAKPIR_OPT
```

## Running Multi-Query KPIR-C

The multi-query benchmark entrypoint is `bench_kpir_c_multi` in `tkpir/src/bin/bench_kpir_c_multi.rs`.

From `tkpir/`, use:

```bash
TKPIR_NUM_THREADS=<THREADS> \
TKPIR_MAX_KEYWORD_BITLEN=<LOG_M> \
TKPIR_MAX_DATA_BITLEN=<DATA_BITS> \
TKPIR_NUM_WARMUP_RUNS=<WARMUP_RUNS> \
TKPIR_NUM_MEASURED_RUNS=<MEASURED_RUNS> \
cargo run --release --bin bench_kpir_c_multi -- \
  --scheme <SCHEME> \
  --function <FUNCTION>
```

Supported schemes are:

```text
TCWKPIR
TCAKPIR
TCAKPIR_OPT
```

Supported functions are:

```text
sum
threshold
```

For example:

```bash
TKPIR_NUM_THREADS=64 \
TKPIR_MAX_KEYWORD_BITLEN=12 \
TKPIR_MAX_DATA_BITLEN=64 \
TKPIR_NUM_WARMUP_RUNS=0 \
TKPIR_NUM_MEASURED_RUNS=1 \
cargo run --release --bin bench_kpir_c_multi -- \
  --scheme TCAKPIR \
  --function sum
```

The benchmark evaluates `t ∈ {1, 2, 4, 8, 16}` automatically.

To skip the KPIR `Response` stage, append:

```text
--skip-kpir
```

## Citation and License

Citation (ePrint version):

```bibtex
@article{DBLP:journals/iacr/ArastehfardLZSPQFH25,
  author       = {Ali Arastehfard and
                  Weiran Liu and
                  Qixian Zhou and
                  Zinan Shen and
                  Liqiang Peng and
                  Lin Qu and
                  Shuya Feng and
                  Yuan Hong},
  title        = {{KPIR-C:} Keyword {PIR} with Arbitrary Server-side Computation},
  journal      = {{IACR} Cryptol. ePrint Arch.},
  volume       = {2025},
  pages        = {1952},
  year         = {2025}
}
```

Licensing terms are provided in `LICENSE`.
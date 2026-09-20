//! Multi-query KPIR-C Evaluate benchmark (Sum / Threshold-of-Sum).
//!
//! Separate from `tests::test_tkpir` so existing paper results are unaffected.
//!
//! Defaults: m = 2^5, 8 B entries, 64 threads, t ∈ {1,2,4,8,16}.
//! Full paper size: TKPIR_MAX_KEYWORD_BITLEN=16 (m = 2^16).

use std::env;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

use rayon::prelude::*;
use tfhe::core_crypto::seeders::new_seeder;
use tfhe::prelude::*;
use tfhe::shortint::ciphertext::Ciphertext as ShortintCiphertext;
use tfhe::shortint::gen_keys;
use tfhe::shortint::parameters::COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64;
use tfhe::shortint::ClientKey as ShortintClientKey;
use tfhe::set_server_key;

use tkpir::client;
use tkpir::common::*;
use tkpir::evaluate::{
    apply_default_mux, choose_evaluate_width, evaluate_sum, evaluate_threshold_of_sum,
    pack_payload, prepare_download_sum, prepare_download_threshold, selection_to_fhebool,
    PackedValue, EVALUATE_DEFAULT,
};
use tkpir::server::{self, Database};
use tkpir::utils::tfhe_utils::*;
use tkpir::utils::{
    calc_total_comm_cost, decompose_hashed, from_decomposition, partition_u64, set_num_threads,
};

const DB_SEED: u64 = 0x4B50_4952;
/// Seeded DB values are 32-bit; recover/compare the low 64 bits of Evaluate
/// output so sums of up to 16 entries fit and high FheUint* limbs are ignored.
const EVALUATE_VALUE_BITS: u64 = 64;
const DEFAULT_KEYWORD_BITLEN: u64 = 5;
const DEFAULT_DATA_BITLEN: u64 = 64;
const DEFAULT_THREADS: usize = 64;
const DEFAULT_NUM_WARMUP_RUNS: usize = 0;
const DEFAULT_NUM_MEASURED_RUNS: usize = 1;
const QUERY_COUNTS: [usize; 5] = [1, 2, 4, 8, 16];

/// Overwrite payloads with deterministic 32-bit values in the existing limb layout.
fn fill_seeded_payloads_u32(db: &mut Database<u8>, seed: u64) {
    use rand::{rngs::StdRng, Rng, SeedableRng};

    let data_bitlen = db.db_params.max_data_bitlen as usize;
    let partition_bitlen = db.db_params.partition_bitlen as usize;
    let mut rng = StdRng::seed_from_u64(seed);

    for row in db.get_data_mut() {
        let value = rng.gen::<u32>() as u64;
        let limbs = partition_u64(value, data_bitlen, partition_bitlen);
        assert_eq!(limbs.len(), row.len());
        row.copy_from_slice(&limbs);
    }
}

/// Reconstruct a plaintext integer from the low `max_bits` of a stored payload row.
fn plaintext_value_low_bits(db: &Database<u8>, keyword: u64, max_bits: u64) -> Option<u64> {
    let data = db.get_data_by_keyword(keyword)?;
    let partition_bitlen = db.db_params.partition_bitlen;
    let radix = 1u64 << partition_bitlen;
    let max_limbs = ((max_bits + partition_bitlen - 1) / partition_bitlen) as usize;
    let digits: Vec<u8> = data.iter().take(max_limbs).copied().collect();
    Some(from_decomposition(radix, &digits))
}

/// Like [`server::prep_db`], but seed payloads before any Byte Fuse Filter is built.
fn prep_db_seeded_u32<'a>(
    db_params: &'a DatabaseParameters,
    cw_params: Option<&ConstantWeightParameters>,
    bff_params: Option<&mut ByteFuseFilterParameters>,
    seed: u64,
) -> Database<'a, u8> {
    let mut db = server::prep_db(db_params, cw_params, None);
    fill_seeded_payloads_u32(&mut db, seed);
    if let Some(bff_params) = bff_params {
        db.build_byte_fuse_filter(bff_params);
    }
    db
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EvalFunction {
    Sum,
    Threshold,
}

impl EvalFunction {
    fn as_str(self) -> &'static str {
        match self {
            EvalFunction::Sum => "sum",
            EvalFunction::Threshold => "threshold",
        }
    }
}

#[derive(Clone, Debug)]
struct Cli {
    out: Option<String>,
    scheme_filter: Option<Scheme>,
    function_filter: Option<EvalFunction>,
    query_counts: &'static [usize],
    mismatch_only: bool,
    /// Skip KPIR Response; run real default mux + Evaluate on real (non-trivial) CTs.
    skip_kpir: bool,
    threads: Option<usize>,
    keyword_bitlen: u64,
    data_bitlen: u64,
    warmup: usize,
    measured: usize,
}

fn main() {
    let cli = parse_cli();
    set_num_threads(cli.threads);

    if cli.mismatch_only {
        if cli.skip_kpir {
            eprintln!("--mismatch-only cannot be combined with --skip-kpir");
            std::process::exit(2);
        }
        run_mismatch_suite(&cli);
        return;
    }

    let schemes: Vec<Scheme> = match cli.scheme_filter {
        Some(s) => vec![s],
        // Under --skip-kpir, TCA and TCA+ share the same default+Evaluate path; keep TCA only.
        None if cli.skip_kpir => vec![Scheme::TCWKPIR, Scheme::TCAKPIR],
        None => vec![Scheme::TCWKPIR, Scheme::TCAKPIR, Scheme::TCAKPIR_OPT],
    };
    if cli.skip_kpir {
        println!(
            "NOTE: --skip-kpir set; skipping Response IP. Running real per-scheme default mux + Evaluate. TCAKPIR+ omitted (identical to TCAKPIR here)."
        );
    }
    let functions: Vec<EvalFunction> = match cli.function_filter {
        Some(f) => vec![f],
        None => vec![EvalFunction::Sum, EvalFunction::Threshold],
    };

    let mut rows: Vec<CsvRow> = Vec::new();
    for scheme in &schemes {
        for &t in cli.query_counts {
            for &func in &functions {
                println!(
                    "\n=== {} | t={} | {} | m=2^{}{} ===",
                    scheme_label(scheme),
                    t,
                    func.as_str(),
                    cli.keyword_bitlen,
                    if cli.skip_kpir { " | skip-kpir" } else { "" }
                );
                let row = if cli.skip_kpir {
                    run_skip_kpir(&cli, *scheme, t, func)
                } else {
                    run_config(&cli, *scheme, t, func)
                };
                print_row_summary(&row);
                rows.push(row);
            }
        }
    }

    print_markdown_table(&rows);

    if let Some(path) = &cli.out {
        write_csv(path, &rows).expect("failed to write CSV");
        println!("\nWrote CSV: {}", path);
    }
}

fn parse_cli() -> Cli {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut out = None;
    let mut scheme_filter = None;
    let mut function_filter = None;
    let mut mismatch_only = false;
    let mut skip_kpir = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out = Some(args.get(i).expect("--out needs a path").clone());
            }
            "--scheme" => {
                i += 1;
                scheme_filter = Some(parse_scheme(args.get(i).expect("--scheme needs a value")));
            }
            "--function" => {
                i += 1;
                function_filter =
                    Some(parse_function(args.get(i).expect("--function needs a value")));
            }
            "--mismatch-only" => mismatch_only = true,
            "--skip-kpir" => skip_kpir = true,
            other => panic!("unknown argument: {}", other),
        }
        i += 1;
    }

    Cli {
        out,
        scheme_filter,
        function_filter,
        query_counts: &QUERY_COUNTS,
        mismatch_only,
        skip_kpir,
        threads: parse_env_threads(Some(DEFAULT_THREADS)),
        keyword_bitlen: parse_env_u64("TKPIR_MAX_KEYWORD_BITLEN", DEFAULT_KEYWORD_BITLEN),
        data_bitlen: parse_env_u64("TKPIR_MAX_DATA_BITLEN", DEFAULT_DATA_BITLEN),
        warmup: parse_env_usize("TKPIR_NUM_WARMUP_RUNS", DEFAULT_NUM_WARMUP_RUNS),
        measured: {
            let m = parse_env_usize("TKPIR_NUM_MEASURED_RUNS", DEFAULT_NUM_MEASURED_RUNS);
            assert!(m > 0, "TKPIR_NUM_MEASURED_RUNS must be > 0");
            m
        },
    }
}

fn parse_scheme(raw: &str) -> Scheme {
    match raw.trim().to_ascii_uppercase().as_str() {
        "TCWKPIR" => Scheme::TCWKPIR,
        "TCAKPIR" => Scheme::TCAKPIR,
        "TCAKPIR_OPT" | "TCAKPIR+" | "TCAKPIRPLUS" => Scheme::TCAKPIR_OPT,
        _ => panic!("invalid scheme: {}", raw),
    }
}

fn parse_function(raw: &str) -> EvalFunction {
    match raw.trim().to_ascii_lowercase().as_str() {
        "sum" => EvalFunction::Sum,
        "threshold" | "threshold-of-sum" | "thresh" => EvalFunction::Threshold,
        _ => panic!("invalid function: {}", raw),
    }
}

fn scheme_label(scheme: &Scheme) -> &'static str {
    match scheme {
        Scheme::TCWKPIR => "TCWKPIR",
        Scheme::TCAKPIR => "TCAKPIR",
        Scheme::TCAKPIR_OPT => "TCAKPIR+",
    }
}

fn parse_env_u64(name: &str, default: u64) -> u64 {
    match env::var(name) {
        Ok(v) => v
            .parse()
            .unwrap_or_else(|_| panic!("invalid {}: {}", name, v)),
        Err(_) => default,
    }
}

fn parse_env_usize(name: &str, default: usize) -> usize {
    match env::var(name) {
        Ok(v) => v
            .parse()
            .unwrap_or_else(|_| panic!("invalid {}: {}", name, v)),
        Err(_) => default,
    }
}

fn parse_env_threads(default: Option<usize>) -> Option<usize> {
    match env::var("TKPIR_NUM_THREADS") {
        Ok(raw) => {
            if raw.trim().eq_ignore_ascii_case("none") {
                None
            } else {
                Some(
                    raw.parse()
                        .unwrap_or_else(|_| panic!("invalid TKPIR_NUM_THREADS: {}", raw)),
                )
            }
        }
        Err(_) => default,
    }
}

fn average(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn keyword_step(db_params: &DatabaseParameters) -> u64 {
    1 << (db_params.max_keyword_bitlen - (db_params.num_keywords as f64).log2().ceil() as u64)
}

fn present_keywords(db_params: &DatabaseParameters, t: usize) -> Vec<u64> {
    let step = keyword_step(db_params);
    assert!(
        t <= db_params.num_keywords,
        "need at least {} DB entries for t={}, have {}",
        t,
        t,
        db_params.num_keywords
    );
    (0..t).map(|i| (i as u64) * step).collect()
}

fn choose_threshold(plaintext_sum: u64) -> u64 {
    std::cmp::max(1, (plaintext_sum + 1) / 2)
}

#[derive(Clone, Debug)]
struct CsvRow {
    scheme: String,
    db_entries: usize,
    entry_bytes: u64,
    num_queries: usize,
    function: String,
    threads: usize,
    response_time_s: f64,
    default_time_s: f64,
    evaluate_time_s: f64,
    total_server_time_s: f64,
    upload_kb: f64,
    download_kb: f64,
    threshold: u64,
    recovered_result: u64,
    expected_result: u64,
    correct: bool,
    offline_time_s: Option<f64>,
    query_time_s: f64,
    recover_time_s: f64,
    measured_runs: usize,
}

fn print_row_summary(row: &CsvRow) {
    println!(
        "response={:.6}s default={:.6}s evaluate={:.6}s total={:.6}s upload={:.3}KB download={:.3}KB correct={} recovered={} expected={}",
        row.response_time_s,
        row.default_time_s,
        row.evaluate_time_s,
        row.total_server_time_s,
        row.upload_kb,
        row.download_kb,
        row.correct,
        row.recovered_result,
        row.expected_result
    );
}

fn print_markdown_table(rows: &[CsvRow]) {
    println!("\n## KPIR-C multi-query results\n");
    println!(
        "| scheme | t | function | response_s | default_s | evaluate_s | total_s | upload_KB | download_KB | correct |"
    );
    println!("|---|---:|---|---:|---:|---:|---:|---:|---:|---|");
    for r in rows {
        println!(
            "| {} | {} | {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.3} | {:.3} | {} |",
            r.scheme,
            r.num_queries,
            r.function,
            r.response_time_s,
            r.default_time_s,
            r.evaluate_time_s,
            r.total_server_time_s,
            r.upload_kb,
            r.download_kb,
            r.correct
        );
    }
}

fn write_csv(path: &str, rows: &[CsvRow]) -> std::io::Result<()> {
    let mut f = File::create(path)?;
    writeln!(
        f,
        "scheme,db_entries,entry_bytes,num_queries,function,threads,response_time_s,default_time_s,evaluate_time_s,total_server_time_s,upload_kb,download_kb,threshold,recovered_result,expected_result,correct,offline_time_s,query_time_s,recover_time_s,measured_runs"
    )?;
    for r in rows {
        let offline = r
            .offline_time_s
            .map(|v| format!("{:.6}", v))
            .unwrap_or_default();
        writeln!(
            f,
            "{},{},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{},{},{},{:.6},{:.6},{}",
            r.scheme,
            r.db_entries,
            r.entry_bytes,
            r.num_queries,
            r.function,
            r.threads,
            r.response_time_s,
            r.default_time_s,
            r.evaluate_time_s,
            r.total_server_time_s,
            r.upload_kb,
            r.download_kb,
            r.threshold,
            r.recovered_result,
            r.expected_result,
            r.correct,
            offline,
            r.query_time_s,
            r.recover_time_s,
            r.measured_runs
        )?;
    }
    Ok(())
}

fn make_db_params(cli: &Cli, params: &tfhe::shortint::ClassicPBSParameters) -> DatabaseParameters {
    assert!(
        cli.keyword_bitlen < usize::BITS as u64,
        "TKPIR_MAX_KEYWORD_BITLEN too large"
    );
    let num_keywords = 1usize << (cli.keyword_bitlen as usize);
    let partition_bitlen = params.message_modulus.0.ilog2() as u64;
    DatabaseParameters::new(
        num_keywords,
        cli.keyword_bitlen,
        cli.data_bitlen,
        partition_bitlen,
    )
}

fn thread_count(cli: &Cli) -> usize {
    cli.threads.unwrap_or_else(num_cpus::get)
}

fn recover_low_u64_from_shortints(
    cks: &ShortintClientKey,
    params: &tfhe::shortint::ClassicPBSParameters,
    limbs: &[ShortintCiphertext],
    max_bits: u64,
) -> u64 {
    let message_bits = params.message_modulus.0.ilog2() as u64;
    let max_limbs = ((max_bits + message_bits - 1) / message_bits) as usize;
    let take = limbs.len().min(max_limbs);
    let radix = params.message_modulus.0 as u128;
    let mut result = 0u128;
    let mut power = 1u128;
    for ct in &limbs[..take] {
        result += (cks.decrypt(ct) as u128) * power;
        power *= radix;
    }
    assert!(
        result <= u64::MAX as u128,
        "recovered low-{}-bit value {} does not fit in u64",
        max_bits,
        result
    );
    result as u64
}

fn run_config(cli: &Cli, scheme: Scheme, t: usize, func: EvalFunction) -> CsvRow {
    let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;
    let db_params = make_db_params(cli, &params);
    let keywords = present_keywords(&db_params, t);

    match scheme {
        Scheme::TCWKPIR => run_tcwkpir(cli, &params, &db_params, &keywords, func),
        Scheme::TCAKPIR => run_tcakpir(cli, &params, &db_params, &keywords, func, false),
        Scheme::TCAKPIR_OPT => run_tcakpir(cli, &params, &db_params, &keywords, func, true),
    }
}

fn plaintext_sum(db: &Database<u8>, keywords: &[u64]) -> u64 {
    keywords
        .iter()
        .map(|kw| {
            plaintext_value_low_bits(&db, *kw, EVALUATE_VALUE_BITS)
                .expect("keyword present")
        })
        .sum()
}

fn run_tcwkpir(
    cli: &Cli,
    params: &tfhe::shortint::ClassicPBSParameters,
    db_params: &DatabaseParameters,
    keywords: &[u64],
    func: EvalFunction,
) -> CsvRow {
    let total_runs = cli.warmup + cli.measured;
    let cw_params = ConstantWeightParameters::new(2, db_params.max_keyword_bitlen);
    let db = prep_db_seeded_u32(db_params, Some(&cw_params), None, DB_SEED);

    let psum = plaintext_sum(&db, keywords);
    let threshold = choose_threshold(psum);
    let expected = match func {
        EvalFunction::Sum => psum,
        EvalFunction::Threshold => u64::from(psum >= threshold),
    };

    let (cks, sks) = gen_keys(*params);
    let (_ck_int, sk_int) = shortint_key_to_client_key(params, cks.clone());
    set_server_key(sk_int);

    let private_comp_key =
        cks.new_compression_private_key(COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64);
    let (comp_key, decomp_key) = cks.new_compression_decompression_keys(&private_comp_key);

    let mut response_times = Vec::new();
    let mut default_times = Vec::new();
    let mut evaluate_times = Vec::new();
    let mut query_times = Vec::new();
    let mut recover_times = Vec::new();
    let mut uploads = Vec::new();
    let mut downloads = Vec::new();
    let mut last_recovered = 0u64;
    let mut last_correct = false;

    for run_idx in 0..total_runs {
        let (queries, query_time, upload) = gen_tcw_queries(&cks, &cw_params, keywords);

        let start_resp = Instant::now();
        let resp_sv: Vec<(Vec<ShortintCiphertext>, Vec<ShortintCiphertext>)> = queries
            .par_iter()
            .map(|q| {
                let decompressed = decompress_shortint_from_slice(q);
                let sv = server::generate_selection_vector_shortint(&sks, &db, &decompressed);
                let resp = server::inner_product_lwe(&sks, &db, &sv);
                (sv, resp)
            })
            .collect();
        let response_time = start_resp.elapsed().as_secs_f64();

        let eval_width = choose_evaluate_width(db_params.max_data_bitlen);
        println!("Evaluate packing: {}", eval_width.label());

        // Default mux on main thread only (TFHE HLAPI needs set_server_key on this thread).
        let start_default = Instant::now();
        let packed: Vec<PackedValue> = resp_sv
            .iter()
            .map(|(sv, resp)| {
                let payload = pack_payload(params, resp, 0, eval_width);
                let sel = server::accumulate_selection(&sks, sv);
                let present = selection_to_fhebool(sel);
                apply_default_mux(&present, &payload, EVALUATE_DEFAULT)
            })
            .collect();
        let default_time = start_default.elapsed().as_secs_f64();

        let start_eval = Instant::now();
        let (compressed, recovered, recover_time) = evaluate_and_recover(
            &cks, &comp_key, &decomp_key, params, &packed, func, threshold,
        );
        let evaluate_time = start_eval.elapsed().as_secs_f64();

        let mut download = 0.0;
        calc_total_comm_cost(&[compressed], &mut download);

        let correct = recovered == expected;
        if !correct {
            eprintln!(
                "ERROR: mismatch recovered={} expected={} scheme=TCWKPIR t={} func={}",
                recovered,
                expected,
                keywords.len(),
                func.as_str()
            );
        }

        if run_idx >= cli.warmup {
            response_times.push(response_time);
            default_times.push(default_time);
            evaluate_times.push(evaluate_time);
            query_times.push(query_time);
            recover_times.push(recover_time);
            uploads.push(upload);
            downloads.push(download);
            last_recovered = recovered;
            last_correct = correct;
        }
    }

    make_row(
        cli,
        "TCWKPIR",
        db_params,
        keywords.len(),
        func,
        threshold,
        last_recovered,
        expected,
        last_correct,
        None,
        &response_times,
        &default_times,
        &evaluate_times,
        &uploads,
        &downloads,
        &query_times,
        &recover_times,
    )
}

fn gen_tcw_queries(
    cks: &ShortintClientKey,
    cw_params: &ConstantWeightParameters,
    keywords: &[u64],
) -> (
    Vec<Vec<tfhe::shortint::ciphertext::CompressedCiphertext>>,
    f64,
    f64,
) {
    let start = Instant::now();
    let queries: Vec<_> = keywords
        .iter()
        .map(|kw| {
            let codeword = client::encode_to_cw_codeword(*kw, cw_params);
            client::encrypt_and_compress_query_shortint(&codeword, cks)
        })
        .collect();
    let query_time = start.elapsed().as_secs_f64();
    let mut upload = 0.0;
    for q in &queries {
        calc_total_comm_cost(q, &mut upload);
    }
    (queries, query_time, upload)
}

fn evaluate_and_recover(
    cks: &ShortintClientKey,
    comp_key: &tfhe::shortint::list_compression::CompressionKey,
    decomp_key: &tfhe::shortint::list_compression::DecompressionKey,
    params: &tfhe::shortint::ClassicPBSParameters,
    packed: &[PackedValue],
    func: EvalFunction,
    threshold: u64,
) -> (
    tfhe::shortint::ciphertext::CompressedCiphertextList,
    u64,
    f64,
) {
    let result_ct = match func {
        EvalFunction::Sum => evaluate_sum(packed),
        EvalFunction::Threshold => evaluate_threshold_of_sum(packed, threshold),
    };

    let compressed = match func {
        EvalFunction::Sum => prepare_download_sum(comp_key, &result_ct),
        EvalFunction::Threshold => prepare_download_threshold(comp_key, &result_ct),
    };

    let start_rec = Instant::now();
    let limbs = decompress_shortint(decomp_key, &compressed);
    let recovered = match func {
        EvalFunction::Sum => {
            recover_low_u64_from_shortints(cks, params, &limbs, EVALUATE_VALUE_BITS)
        }
        EvalFunction::Threshold => cks.decrypt(&limbs[0]),
    };
    let recover_time = start_rec.elapsed().as_secs_f64();
    (compressed, recovered, recover_time)
}

fn run_tcakpir(
    cli: &Cli,
    params: &tfhe::shortint::ClassicPBSParameters,
    db_params: &DatabaseParameters,
    keywords: &[u64],
    func: EvalFunction,
    optimized: bool,
) -> CsvRow {
    let total_runs = cli.warmup + cli.measured;
    let lwe_dim = find_lwe_dim(params);
    let mut bff_params = ByteFuseFilterParameters {
        filter_len: 0,
        filter_seed: Vec::new(),
        digest_byte_len: 8,
        partition_bitlen: db_params.partition_bitlen as usize,
    };
    let key_byte_len = bff_params.key_byte_len();

    let db = prep_db_seeded_u32(db_params, None, Some(&mut bff_params), DB_SEED);
    let eval_width = choose_evaluate_width(db_params.max_data_bitlen);
    println!("Evaluate packing: {}", eval_width.label());
    let psum = plaintext_sum(&db, keywords);
    let threshold = choose_threshold(psum);
    let expected = match func {
        EvalFunction::Sum => psum,
        EvalFunction::Threshold => u64::from(psum >= threshold),
    };

    let mut seeder = new_seeder();
    let seeder = seeder.as_mut();

    let (cks, sks) = gen_keys(*params);
    let (lwe_secret_key, noise_dist) = cks.encryption_key_and_noise();
    let secret_key: &[u64] = lwe_secret_key.into_container();
    let (_ck_int, sk_int) = shortint_key_to_client_key(params, cks.clone());
    set_server_key(sk_int);

    let private_comp_key =
        cks.new_compression_private_key(COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64);
    let (comp_key, decomp_key) = cks.new_compression_decompression_keys(&private_comp_key);

    let mut offline_time_s = None;
    let masked_data_opt = if optimized {
        let mut offline_times = Vec::new();
        let mut last_masked = None;
        for run_idx in 0..total_runs {
            let mask_seed = seeder.seed();
            let start = Instant::now();
            let masks = generate_raw_masks(mask_seed, lwe_dim.0, bff_params.filter_len);
            let masked = server::mask_database(&masks, lwe_dim.0, &db, &bff_params);
            if run_idx >= cli.warmup {
                offline_times.push(start.elapsed().as_secs_f64());
            }
            last_masked = Some((masks, masked));
        }
        offline_time_s = Some(average(&offline_times));
        last_masked
    } else {
        None
    };

    let mut response_times = Vec::new();
    let mut default_times = Vec::new();
    let mut evaluate_times = Vec::new();
    let mut query_times = Vec::new();
    let mut recover_times = Vec::new();
    let mut uploads = Vec::new();
    let mut downloads = Vec::new();
    let mut last_recovered = 0u64;
    let mut last_correct = false;

    for run_idx in 0..total_runs {
        let start_query = Instant::now();
        let mut upload = 0.0;
        // bodies, optional masks, compressed client digest (for default mux)
        let query_material: Vec<(
            Vec<u64>,
            Option<Vec<Vec<u64>>>,
            Vec<tfhe::shortint::ciphertext::CompressedCiphertext>,
        )> = keywords
            .iter()
            .map(|kw| {
                let digest = decompose_hashed(
                    *kw,
                    bff_params.digest_byte_len,
                    db_params.partition_bitlen as usize,
                );
                let digest_ct = client::encrypt_and_compress_query_shortint(&digest, &cks);
                calc_total_comm_cost(&digest_ct, &mut upload);

                if optimized {
                    let (masks, _masked) = masked_data_opt.as_ref().expect("offline masks");
                    let noise = client::generate_raw_noise(
                        seeder.seed(),
                        noise_dist,
                        bff_params.filter_len,
                    );
                    let b = client::generate_raw_bodies(
                        masks,
                        secret_key,
                        &noise,
                        bff_params.filter_len,
                    );
                    let bodies =
                        client::generate_selection_vector(*kw, &b, params, &bff_params, db_params);
                    calc_total_comm_cost(&bodies, &mut upload);
                    (bodies, None, digest_ct)
                } else {
                    let mask_seed = seeder.seed();
                    let masks = generate_raw_masks(mask_seed, lwe_dim.0, bff_params.filter_len);
                    let noise = client::generate_raw_noise(
                        seeder.seed(),
                        noise_dist,
                        bff_params.filter_len,
                    );
                    let b = client::generate_raw_bodies(
                        &masks,
                        secret_key,
                        &noise,
                        bff_params.filter_len,
                    );
                    let bodies =
                        client::generate_selection_vector(*kw, &b, params, &bff_params, db_params);
                    calc_total_comm_cost(&bodies, &mut upload);
                    (bodies, Some(masks), digest_ct)
                }
            })
            .collect();
        let query_time = start_query.elapsed().as_secs_f64();

        let start_resp = Instant::now();
        let responses: Vec<Vec<ShortintCiphertext>> = if optimized {
            let (_masks, masked_data) = masked_data_opt.as_ref().expect("offline masked data");
            query_material
                .par_iter()
                .map(|(bodies, _, _)| {
                    server::client_aided_inner_product_from_raw_parts(
                        bodies,
                        masked_data,
                        &db,
                        params,
                        &bff_params,
                    )
                })
                .collect()
        } else {
            query_material
                .par_iter()
                .map(|(bodies, masks, _)| {
                    let masks = masks.as_ref().unwrap();
                    let sv = create_shortint_ciphertexts_from_raw_parts(
                        params,
                        masks,
                        bodies,
                        bff_params.filter_len,
                    );
                    server::client_aided_inner_product(&sks, &bff_params, &db, &sv)
                })
                .collect()
        };
        let response_time = start_resp.elapsed().as_secs_f64();

        // Default mux timed separately on main thread (HLAPI / set_server_key).
        let start_default = Instant::now();
        let packed: Vec<PackedValue> = responses
            .iter()
            .zip(query_material.iter())
            .map(|(resp, (_, _, digest_comp))| {
                let payload = pack_payload(params, resp, key_byte_len, eval_width);
                let selected_digest =
                    shortint_ciphertexts_to_fheuint64(params, &resp[..key_byte_len]);
                let client_digest = shortint_ciphertexts_to_fheuint64(
                    params,
                    &decompress_shortint_from_slice(digest_comp),
                );
                let present = selected_digest.eq(&client_digest);
                apply_default_mux(&present, &payload, EVALUATE_DEFAULT)
            })
            .collect();
        let default_time = start_default.elapsed().as_secs_f64();

        let start_eval = Instant::now();
        let (compressed, recovered, recover_time) = evaluate_and_recover(
            &cks, &comp_key, &decomp_key, params, &packed, func, threshold,
        );
        let evaluate_time = start_eval.elapsed().as_secs_f64();

        let mut download = 0.0;
        calc_total_comm_cost(&[compressed], &mut download);

        let correct = recovered == expected;
        if !correct {
            eprintln!(
                "ERROR: mismatch recovered={} expected={} scheme={} t={} func={}",
                recovered,
                expected,
                scheme_label(if optimized {
                    &Scheme::TCAKPIR_OPT
                } else {
                    &Scheme::TCAKPIR
                }),
                keywords.len(),
                func.as_str()
            );
        }

        if run_idx >= cli.warmup {
            response_times.push(response_time);
            default_times.push(default_time);
            evaluate_times.push(evaluate_time);
            query_times.push(query_time);
            recover_times.push(recover_time);
            uploads.push(upload);
            downloads.push(download);
            last_recovered = recovered;
            last_correct = correct;
        }
    }

    make_row(
        cli,
        if optimized { "TCAKPIR+" } else { "TCAKPIR" },
        db_params,
        keywords.len(),
        func,
        threshold,
        last_recovered,
        expected,
        last_correct,
        offline_time_s,
        &response_times,
        &default_times,
        &evaluate_times,
        &uploads,
        &downloads,
        &query_times,
        &recover_times,
    )
}

/// Escape hatch: skip KPIR Response (inner product / SV gen), but run the
/// **real** scheme-specific default mux, then Evaluate.
///
/// All synthesized inputs are **real client-key encryptions** (not trivial CTs),
/// so both schemes feed Evaluate the same ciphertext class.
///
/// - TCWKPIR: encrypted one-hot SV → `accumulate_selection` → `FheBool` + mux
/// - TCA*: real client digest + real encrypted retrieved digest → `eq` + mux
fn run_skip_kpir(cli: &Cli, scheme: Scheme, t: usize, func: EvalFunction) -> CsvRow {
    let total_runs = cli.warmup + cli.measured;
    let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;
    let db_params = make_db_params(cli, &params);
    let keywords = present_keywords(&db_params, t);
    let eval_width = choose_evaluate_width(db_params.max_data_bitlen);
    println!(
        "Evaluate packing: {} (skip-kpir; real {} default)",
        eval_width.label(),
        scheme_label(&scheme)
    );

    let db = prep_db_seeded_u32(&db_params, None, None, DB_SEED);
    let values: Vec<u64> = keywords
        .iter()
        .map(|kw| {
            plaintext_value_low_bits(&db, *kw, EVALUATE_VALUE_BITS)
                .expect("keyword present")
        })
        .collect();
    let indices: Vec<usize> = keywords
        .iter()
        .map(|kw| db.get_index_by_keyword(*kw).expect("keyword present"))
        .collect();
    let psum: u64 = values.iter().sum();
    let threshold = choose_threshold(psum);
    let expected = match func {
        EvalFunction::Sum => psum,
        EvalFunction::Threshold => u64::from(psum >= threshold),
    };

    let (cks, sks) = gen_keys(params);
    let (ck_int, sk_int) = shortint_key_to_client_key(&params, cks.clone());
    set_server_key(sk_int);
    let private_comp_key =
        cks.new_compression_private_key(COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64);
    let (comp_key, decomp_key) = cks.new_compression_decompression_keys(&private_comp_key);

    let digest_byte_len = 8usize;
    let partition_bitlen = db_params.partition_bitlen as usize;

    let mut default_times = Vec::new();
    let mut evaluate_times = Vec::new();
    let mut query_times = Vec::new();
    let mut recover_times = Vec::new();
    let mut uploads = Vec::new();
    let mut downloads = Vec::new();
    let mut last_recovered = 0u64;
    let mut last_correct = false;

    for run_idx in 0..total_runs {
        let start_query = Instant::now();
        let mut upload = 0.0;

        // Client-side / synthesized encrypted inputs (real encrypt, not trivial).
        let payloads: Vec<PackedValue> = values
            .iter()
            .map(|&value| PackedValue::encrypt_u64(&ck_int, eval_width, value))
            .collect();

        let encrypted_svs: Vec<Vec<ShortintCiphertext>> = if matches!(scheme, Scheme::TCWKPIR) {
            indices
                .iter()
                .map(|&idx| encrypted_one_hot_sv(&cks, db_params.num_keywords, idx))
                .collect()
        } else {
            Vec::new()
        };

        let client_digests: Vec<Vec<tfhe::shortint::ciphertext::CompressedCiphertext>> =
            if matches!(scheme, Scheme::TCAKPIR | Scheme::TCAKPIR_OPT) {
                keywords
                    .iter()
                    .map(|kw| {
                        let digest =
                            decompose_hashed(*kw, digest_byte_len, partition_bitlen);
                        let dig_ct = client::encrypt_and_compress_query_shortint(&digest, &cks);
                        calc_total_comm_cost(&dig_ct, &mut upload);
                        dig_ct
                    })
                    .collect()
            } else {
                Vec::new()
            };

        // Synthesize "retrieved" digests as real encryptions of the clear hash (hit case).
        let retrieved_digests: Vec<Vec<ShortintCiphertext>> =
            if matches!(scheme, Scheme::TCAKPIR | Scheme::TCAKPIR_OPT) {
                keywords
                    .iter()
                    .map(|kw| {
                        let digest =
                            decompose_hashed(*kw, digest_byte_len, partition_bitlen);
                        digest
                            .iter()
                            .map(|&d| cks.encrypt(d as u64))
                            .collect()
                    })
                    .collect()
            } else {
                Vec::new()
            };
        let query_time = start_query.elapsed().as_secs_f64();

        let start_default = Instant::now();
        let packed: Vec<PackedValue> = match scheme {
            Scheme::TCWKPIR => encrypted_svs
                .iter()
                .zip(payloads.iter())
                .map(|(sv, payload)| {
                    let sel = server::accumulate_selection(&sks, sv);
                    let present = selection_to_fhebool(sel);
                    apply_default_mux(&present, payload, EVALUATE_DEFAULT)
                })
                .collect(),
            Scheme::TCAKPIR | Scheme::TCAKPIR_OPT => retrieved_digests
                .iter()
                .zip(client_digests.iter())
                .zip(payloads.iter())
                .map(|((selected_limbs, dig_comp), payload)| {
                    let selected_digest =
                        shortint_ciphertexts_to_fheuint64(&params, selected_limbs);
                    let client_digest = shortint_ciphertexts_to_fheuint64(
                        &params,
                        &decompress_shortint_from_slice(dig_comp),
                    );
                    let present = selected_digest.eq(&client_digest);
                    apply_default_mux(&present, payload, EVALUATE_DEFAULT)
                })
                .collect(),
        };
        let default_time = start_default.elapsed().as_secs_f64();

        let start_eval = Instant::now();
        let (compressed, recovered, recover_time) = evaluate_and_recover(
            &cks, &comp_key, &decomp_key, &params, &packed, func, threshold,
        );
        let evaluate_time = start_eval.elapsed().as_secs_f64();

        let mut download = 0.0;
        calc_total_comm_cost(&[compressed], &mut download);

        let correct = recovered == expected;
        if !correct {
            eprintln!(
                "ERROR: skip-kpir mismatch recovered={} expected={} scheme={} t={} func={}",
                recovered,
                expected,
                scheme_label(&scheme),
                t,
                func.as_str()
            );
        }

        if run_idx >= cli.warmup {
            default_times.push(default_time);
            evaluate_times.push(evaluate_time);
            query_times.push(query_time);
            recover_times.push(recover_time);
            uploads.push(upload);
            downloads.push(download);
            last_recovered = recovered;
            last_correct = correct;
        }
    }

    make_row(
        cli,
        scheme_label(&scheme),
        &db_params,
        t,
        func,
        threshold,
        last_recovered,
        expected,
        last_correct,
        None,
        &[], // response skipped
        &default_times,
        &evaluate_times,
        &uploads,
        &downloads,
        &query_times,
        &recover_times,
    )
}

fn encrypted_one_hot_sv(
    cks: &ShortintClientKey,
    len: usize,
    index: usize,
) -> Vec<ShortintCiphertext> {
    (0..len)
        .map(|i| cks.encrypt(if i == index { 1u64 } else { 0u64 }))
        .collect()
}

fn make_row(
    cli: &Cli,
    scheme: &str,
    db_params: &DatabaseParameters,
    t: usize,
    func: EvalFunction,
    threshold: u64,
    recovered: u64,
    expected: u64,
    correct: bool,
    offline_time_s: Option<f64>,
    response_times: &[f64],
    default_times: &[f64],
    evaluate_times: &[f64],
    uploads: &[f64],
    downloads: &[f64],
    query_times: &[f64],
    recover_times: &[f64],
) -> CsvRow {
    let response_time_s = average(response_times);
    let default_time_s = average(default_times);
    let evaluate_time_s = average(evaluate_times);
    CsvRow {
        scheme: scheme.to_string(),
        db_entries: db_params.num_keywords,
        entry_bytes: db_params.max_data_bitlen / 8,
        num_queries: t,
        function: func.as_str().to_string(),
        threads: thread_count(cli),
        response_time_s,
        default_time_s,
        evaluate_time_s,
        total_server_time_s: response_time_s + default_time_s + evaluate_time_s,
        upload_kb: average(uploads),
        download_kb: average(downloads),
        threshold,
        recovered_result: recovered,
        expected_result: expected,
        correct,
        offline_time_s,
        query_time_s: average(query_times),
        recover_time_s: average(recover_times),
        measured_runs: cli.measured,
    }
}

/// Separate correctness test: t=8 with 6 hits + 2 misses, delta=0 (exact match).
fn run_mismatch_suite(cli: &Cli) {
    println!("=== Mismatch correctness suite (t=8, 6 hit / 2 miss, delta=0) ===");
    let schemes = match cli.scheme_filter {
        Some(s) => vec![s],
        None => vec![Scheme::TCWKPIR, Scheme::TCAKPIR, Scheme::TCAKPIR_OPT],
    };
    for scheme in schemes {
        let ok_sum = run_mismatch_one(cli, scheme, EvalFunction::Sum);
        let ok_thr = run_mismatch_one(cli, scheme, EvalFunction::Threshold);
        println!(
            "{} sum={} threshold={}",
            scheme_label(&scheme),
            if ok_sum { "PASS" } else { "FAIL" },
            if ok_thr { "PASS" } else { "FAIL" }
        );
        if !ok_sum || !ok_thr {
            std::process::exit(1);
        }
    }
}

fn run_mismatch_one(cli: &Cli, scheme: Scheme, func: EvalFunction) -> bool {
    let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;
    let db_params = make_db_params(cli, &params);
    let step = keyword_step(&db_params);
    let present: Vec<u64> = (0..6).map(|i| i * step).collect();
    let absent: Vec<u64> = vec![
        (db_params.num_keywords as u64) * step,
        (db_params.num_keywords as u64 + 1) * step,
    ];
    let mut keywords = present.clone();
    keywords.extend_from_slice(&absent);

    match scheme {
        Scheme::TCWKPIR => mismatch_tcwkpir(cli, &params, &db_params, &keywords, &present, func),
        Scheme::TCAKPIR => {
            mismatch_tcakpir(cli, &params, &db_params, &keywords, &present, func, false)
        }
        Scheme::TCAKPIR_OPT => {
            mismatch_tcakpir(cli, &params, &db_params, &keywords, &present, func, true)
        }
    }
}

fn mismatch_tcwkpir(
    _cli: &Cli,
    params: &tfhe::shortint::ClassicPBSParameters,
    db_params: &DatabaseParameters,
    keywords: &[u64],
    present: &[u64],
    func: EvalFunction,
) -> bool {
    let cw_params = ConstantWeightParameters::new(2, db_params.max_keyword_bitlen);
    let db = prep_db_seeded_u32(db_params, Some(&cw_params), None, DB_SEED);
    let psum: u64 = present
        .iter()
        .map(|kw| {
            plaintext_value_low_bits(&db, *kw, EVALUATE_VALUE_BITS)
                .unwrap()
        })
        .sum();
    let threshold = choose_threshold(psum);
    let expected = match func {
        EvalFunction::Sum => psum,
        EvalFunction::Threshold => u64::from(psum >= threshold),
    };

    let (cks, sks) = gen_keys(*params);
    let (_ck_int, sk_int) = shortint_key_to_client_key(params, cks.clone());
    set_server_key(sk_int);
    let private_comp_key =
        cks.new_compression_private_key(COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64);
    let (comp_key, decomp_key) = cks.new_compression_decompression_keys(&private_comp_key);

    let (queries, _, _) = gen_tcw_queries(&cks, &cw_params, keywords);

    let resp_sv: Vec<(Vec<ShortintCiphertext>, Vec<ShortintCiphertext>)> = queries
        .par_iter()
        .map(|q| {
            let decompressed = decompress_shortint_from_slice(q);
            let sv = server::generate_selection_vector_shortint(&sks, &db, &decompressed);
            let resp = server::inner_product_lwe(&sks, &db, &sv);
            (sv, resp)
        })
        .collect();

    let eval_width = choose_evaluate_width(db_params.max_data_bitlen);
    let packed: Vec<PackedValue> = resp_sv
        .iter()
        .map(|(sv, resp)| {
            let payload = pack_payload(params, resp, 0, eval_width);
            let sel = server::accumulate_selection(&sks, sv);
            let present = selection_to_fhebool(sel);
            apply_default_mux(&present, &payload, EVALUATE_DEFAULT)
        })
        .collect();

    let (_c, recovered, _) =
        evaluate_and_recover(&cks, &comp_key, &decomp_key, params, &packed, func, threshold);
    recovered == expected
}

fn mismatch_tcakpir(
    _cli: &Cli,
    params: &tfhe::shortint::ClassicPBSParameters,
    db_params: &DatabaseParameters,
    keywords: &[u64],
    present: &[u64],
    func: EvalFunction,
    optimized: bool,
) -> bool {
    let lwe_dim = find_lwe_dim(params);
    let mut bff_params = ByteFuseFilterParameters {
        filter_len: 0,
        filter_seed: Vec::new(),
        digest_byte_len: 8,
        partition_bitlen: db_params.partition_bitlen as usize,
    };
    let key_byte_len = bff_params.key_byte_len();
    let db = prep_db_seeded_u32(db_params, None, Some(&mut bff_params), DB_SEED);
    let psum: u64 = present
        .iter()
        .map(|kw| {
            plaintext_value_low_bits(&db, *kw, EVALUATE_VALUE_BITS)
                .unwrap()
        })
        .sum();
    let threshold = choose_threshold(psum);
    let expected = match func {
        EvalFunction::Sum => psum,
        EvalFunction::Threshold => u64::from(psum >= threshold),
    };

    let mut seeder = new_seeder();
    let seeder = seeder.as_mut();
    let (cks, sks) = gen_keys(*params);
    let (lwe_secret_key, noise_dist) = cks.encryption_key_and_noise();
    let secret_key: &[u64] = lwe_secret_key.into_container();
    let (_ck_int, sk_int) = shortint_key_to_client_key(params, cks.clone());
    set_server_key(sk_int);
    let private_comp_key =
        cks.new_compression_private_key(COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64);
    let (comp_key, decomp_key) = cks.new_compression_decompression_keys(&private_comp_key);

    let masked_data_opt = if optimized {
        let mask_seed = seeder.seed();
        let masks = generate_raw_masks(mask_seed, lwe_dim.0, bff_params.filter_len);
        let masked = server::mask_database(&masks, lwe_dim.0, &db, &bff_params);
        Some((masks, masked))
    } else {
        None
    };

    let materials: Vec<_> = keywords
        .iter()
        .map(|kw| {
            if optimized {
                let (masks, _masked) = masked_data_opt.as_ref().unwrap();
                let noise =
                    client::generate_raw_noise(seeder.seed(), noise_dist, bff_params.filter_len);
                let b =
                    client::generate_raw_bodies(masks, secret_key, &noise, bff_params.filter_len);
                let bodies =
                    client::generate_selection_vector(*kw, &b, params, &bff_params, db_params);
                let digest = decompose_hashed(
                    *kw,
                    bff_params.digest_byte_len,
                    db_params.partition_bitlen as usize,
                );
                let digest_ct = client::encrypt_and_compress_query_shortint(&digest, &cks);
                (bodies, None, digest_ct)
            } else {
                let mask_seed = seeder.seed();
                let masks = generate_raw_masks(mask_seed, lwe_dim.0, bff_params.filter_len);
                let noise =
                    client::generate_raw_noise(seeder.seed(), noise_dist, bff_params.filter_len);
                let b =
                    client::generate_raw_bodies(&masks, secret_key, &noise, bff_params.filter_len);
                let bodies =
                    client::generate_selection_vector(*kw, &b, params, &bff_params, db_params);
                let digest = decompose_hashed(
                    *kw,
                    bff_params.digest_byte_len,
                    db_params.partition_bitlen as usize,
                );
                let digest_ct = client::encrypt_and_compress_query_shortint(&digest, &cks);
                (bodies, Some(masks), digest_ct)
            }
        })
        .collect();

    let responses: Vec<Vec<ShortintCiphertext>> = materials
        .par_iter()
        .map(|(bodies, masks, _digest_comp)| {
            if optimized {
                let (_m, masked) = masked_data_opt.as_ref().unwrap();
                server::client_aided_inner_product_from_raw_parts(
                    bodies, masked, &db, params, &bff_params,
                )
            } else {
                let masks = masks.as_ref().unwrap();
                let sv = create_shortint_ciphertexts_from_raw_parts(
                    params,
                    masks,
                    bodies,
                    bff_params.filter_len,
                );
                server::client_aided_inner_product(&sks, &bff_params, &db, &sv)
            }
        })
        .collect();

    let eval_width = choose_evaluate_width(db_params.max_data_bitlen);
    let packed: Vec<PackedValue> = responses
        .iter()
        .zip(materials.iter())
        .map(|(resp, (_, _, digest_comp))| {
            let payload = pack_payload(params, resp, key_byte_len, eval_width);
            let selected_digest =
                shortint_ciphertexts_to_fheuint64(params, &resp[..key_byte_len]);
            let client_digest = shortint_ciphertexts_to_fheuint64(
                params,
                &decompress_shortint_from_slice(digest_comp),
            );
            let is_present = selected_digest.eq(&client_digest);
            apply_default_mux(&is_present, &payload, EVALUATE_DEFAULT)
        })
        .collect();

    let (_c, recovered, _) =
        evaluate_and_recover(&cks, &comp_key, &decomp_key, params, &packed, func, threshold);
    recovered == expected
}

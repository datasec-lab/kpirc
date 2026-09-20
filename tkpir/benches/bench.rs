use criterion::{criterion_group, criterion_main, Criterion};
use rand::Rng;
use std::time::Duration;
use std::time::Instant;
use tfhe::prelude::*;
use tfhe::shortint::ClassicPBSParameters;
use tfhe::{
    core_crypto::seeders::new_seeder, shortint::gen_keys, CompressedCiphertextListBuilder,
    FheUint64,
};
use tkpir::client;
use tkpir::common::*;
use tkpir::server;
use tkpir::time;
use tkpir::utils::{tfhe_utils::*, *};

#[derive(Debug)]
pub struct BenchResult {
    pub ip_time: Duration, // time to perform inner product
    pub sv_time: Duration, // time to genrate selection vector
    pub offline_time: Duration,
    pub online_time: Duration,
    pub total_time: Duration,
    pub total_comm_kb: f64,
}

fn print_criteria(results: &Vec<BenchResult>) {
    let ip_avg = results.iter().map(|r| r.ip_time).sum::<Duration>() / results.len() as u32;
    let sv_avg = results.iter().map(|r| r.sv_time).sum::<Duration>() / results.len() as u32;
    let offline_avg =
        results.iter().map(|r| r.offline_time).sum::<Duration>() / results.len() as u32;
    let online_avg = results.iter().map(|r| r.online_time).sum::<Duration>() / results.len() as u32;
    let total_avg = results.iter().map(|r| r.total_time).sum::<Duration>() / results.len() as u32;
    let comm_avg = results.iter().map(|r| r.total_comm_kb).sum::<f64>() / results.len() as f64;

    println!("Average Inner Product Time: {:?}", ip_avg);
    println!("Average Selection Vector Time: {:?}", sv_avg);
    println!("Average Offline Time: {:?}", offline_avg);
    println!("Average Online Time: {:?}", online_avg);
    println!("Average Total Time: {:?}", total_avg);
    println!("Average Communication: {:.2} KB", comm_avg);
}

// Run with `cargo bench --bench benches`
fn benchmark_tkpir(c: &mut Criterion) {
    let mut group = c.benchmark_group("tkpir");
    group.sample_size(10);

    // Use:
    // 1. `Some(num_threads)` for a fixed number of threads
    // 2. `None` for as many threads as available
    set_num_threads(None);

    let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;

    let max_keyword_bitlen = 12;
    let num_keywords = 1 << 12;
    let max_data_bitlen = 32;
    let partition_bitlen = params.message_modulus.0.ilog2() as u64;
    let db_params = DatabaseParameters::new(
        num_keywords,
        max_keyword_bitlen,
        max_data_bitlen,
        partition_bitlen,
    );

    println!("Parameters: {:?}", params);
    println!("Database Parameters: {:?}", db_params);

    let mut results = Vec::new();

    println!("{:?}", "Running benchmark for TCAKPIR_OPT...");
    group.bench_function("tcakpir_opt", |b| {
        b.iter(|| {
            let result = run_tcakpir_opt(params, &db_params, false);
            results.push(result);
        });
    });
    print_criteria(&results);
    results.clear();

    println!("{:?}", "Running benchmark for TCAKPIR_OPT with default...");
    group.bench_function("tcakpir_opt_with_default", |b| {
        b.iter(|| {
            let result = run_tcakpir_opt(params, &db_params, true);
            results.push(result);
        });
    });
    print_criteria(&results);
    results.clear();

    println!("{:?}", "Running benchmark for TCAKPIR...");
    group.bench_function("tcakpir", |b| {
        b.iter(|| {
            let result = run_tcakpir(params, &db_params, false);
            results.push(result);
        });
    });
    print_criteria(&results);
    results.clear();

    // println!("{:?}", "Running benchmark for TCAKPIR with default...");
    // group.bench_function("tcakpir_with_default", |b| {
    //     b.iter(|| {
    //         let result = run_tcakpir(params, &db_params, true);
    //         results.push(result);
    //     });
    // });
    // print_criteria(&results);
    // results.clear();

    group.finish();
}

fn run_tcakpir_opt(
    params: ClassicPBSParameters,
    db_params: &DatabaseParameters,
    with_default: bool,
) -> BenchResult {
    let lwe_dim = find_lwe_dim(&params);

    let max_keyword_bitlen = db_params.max_keyword_bitlen;
    let partition_bitlen = params.message_modulus.0.ilog2() as u64;

    let mut bff_params = ByteFuseFilterParameters {
        filter_len: 0,
        filter_seed: Vec::new(),
        digest_byte_len: 8,
        partition_bitlen: partition_bitlen as usize,
    };

    let mut seeder = new_seeder();
    let seeder = seeder.as_mut();

    let start_time = Instant::now();

    // -----------------
    //  Offline --------
    // -----------------

    // Server ----------
    // -----------------

    let (db, db_prep_time) = time!(
        "Prepare database",
        server::prep_db::<u8>(&db_params, None, Some(&mut bff_params))
    );

    let mask_seed = seeder.seed();

    // Generate masks
    let masks = generate_raw_masks(mask_seed, lwe_dim.0, bff_params.filter_len);

    let (masked_data, ip_offline_time) = time!(
        "Generate masks part of the inner product",
        server::mask_database(&masks, lwe_dim.0, &db, &bff_params)
    );

    // -----------------
    // Client ----------
    // -----------------

    // Generate keys
    let (cks, _sks) = gen_keys(params);
    let (ck, _sk) = shortint_key_to_client_key(&params, cks.clone());

    let (lwe_secret_key, noise_dist) = cks.encryption_key_and_noise();

    let secret_key: &[u64] = lwe_secret_key.into_container();

    // Generate noise
    let noise = client::generate_raw_noise(seeder.seed(), noise_dist, bff_params.filter_len);

    // Generate raw bodies
    let (b, sv_offline_time) = time!(
        "Generate raw bodies",
        client::generate_raw_bodies(&masks, secret_key, &noise, bff_params.filter_len)
    );

    // -----------------
    //  Online ---------
    // -----------------

    // Client ----------
    // -----------------

    let kw = rand::thread_rng().gen::<u64>() & ((1 << max_keyword_bitlen) - 1);

    // Decompose hashed keyword
    let kw_decomposed = match with_default {
        true => decompose_hashed(kw, bff_params.digest_byte_len, partition_bitlen as usize),
        false => vec![],
    };

    // Encrypt and compress query
    let kw_enc_compressed = match with_default {
        true => client::encrypt_and_compress_query_shortint(&kw_decomposed, &cks),
        false => vec![],
    };

    let (bodies, sv_time) = time!(
        "Generate selection vector",
        client::generate_selection_vector(kw, &b, &params, &bff_params, &db_params)
    );

    // -----------------
    // Server ----------
    // -----------------

    // Decompress query
    let kw_enc = match with_default {
        true => decompress_shortint_from_slice(&kw_enc_compressed),
        false => vec![],
    };

    let (response, ip_time) = time!("Perform inner product", {
        let result = server::client_aided_inner_product_from_raw_parts(
            &bodies,
            &masked_data,
            &db,
            &params,
            &bff_params,
        );

        let response = &result[bff_params.key_byte_len()..];
        let mut response = shortint_ciphertexts_to_fheuint64(&params, &response);

        if with_default {
            let selected_kw = &result[..bff_params.key_byte_len()];

            let d1 = shortint_ciphertexts_to_fheuint64(&params, &kw_enc); // digest shared by the client
            let d2 = shortint_ciphertexts_to_fheuint64(&params, &selected_kw); // digest retrieved from the inner product

            let sel = d1.eq(&d2);

            // Select the default value if the keyword is not found
            response = sel.select(&response, &FheUint64::encrypt_trivial(DEFAULT));
        }

        response
    });

    // Compress response
    let response_compressed = CompressedCiphertextListBuilder::new()
        .push(response)
        .build()
        .unwrap();

    // -----------------
    // Client ----------
    // -----------------

    // Decompress response
    let response: FheUint64 = response_compressed.get(0).unwrap().unwrap();

    // Decrypt response
    let response: u64 = response.decrypt(&ck);

    let total_time = start_time.elapsed();

    let expected_value = match db.get_data_by_keyword(kw) {
        Some(data) => from_decomposition(params.message_modulus.0 as u64, &data),
        None => DEFAULT,
    };
    assert_eq!(response, expected_value);

    let mut total_comm_kb: f64 = 0.0;
    if with_default {
        calc_total_comm_cost(&kw_enc_compressed, &mut total_comm_kb);
    }
    calc_total_comm_cost(&bodies, &mut total_comm_kb);
    calc_total_comm_cost(&[response_compressed], &mut total_comm_kb);

    let offline_time = db_prep_time + ip_offline_time + sv_offline_time;
    let online_time = ip_time + sv_time;

    BenchResult {
        ip_time,
        sv_time,
        offline_time,
        online_time,
        total_time,
        total_comm_kb,
    }
}

fn run_tcakpir(
    params: ClassicPBSParameters,
    db_params: &DatabaseParameters,
    with_default: bool,
) -> BenchResult {
    let lwe_dim = find_lwe_dim(&params);

    let max_keyword_bitlen = db_params.max_keyword_bitlen;
    let partition_bitlen = params.message_modulus.0.ilog2() as u64;

    let mut bff_params = ByteFuseFilterParameters {
        filter_len: 0,
        filter_seed: Vec::new(),
        digest_byte_len: 8,
        partition_bitlen: partition_bitlen as usize,
    };

    let mut seeder = new_seeder();
    let seeder = seeder.as_mut();

    let start_time = Instant::now();

    // -----------------
    //  Offline --------
    // -----------------

    // Server ----------
    // -----------------

    let (db, db_prep_time) = time!(
        "Preprocess database",
        server::prep_db::<u8>(&db_params, None, Some(&mut bff_params))
    );

    // -----------------
    // Client ----------
    // -----------------

    // Generate keys
    let (cks, sks) = gen_keys(params);
    let (ck, _sk) = shortint_key_to_client_key(&params, cks.clone());

    let (lwe_secret_key, noise_dist) = cks.encryption_key_and_noise();

    let secret_key: &[u64] = lwe_secret_key.into_container();

    // -----------------
    //  Online ---------
    // -----------------

    // Client ----------
    // -----------------

    let mask_seed = seeder.seed();

    // Generate masks
    let masks = generate_raw_masks(mask_seed, lwe_dim.0, bff_params.filter_len);

    // Generate noise
    let noise = client::generate_raw_noise(seeder.seed(), noise_dist, bff_params.filter_len);

    // Generate raw bodies
    let b = client::generate_raw_bodies(&masks, secret_key, &noise, bff_params.filter_len);

    let kw = rand::thread_rng().gen::<u64>() & ((1 << max_keyword_bitlen) - 1);

    // Decompose hashed keyword
    let kw_decomposed = match with_default {
        true => decompose_hashed(
            kw,
            bff_params.digest_byte_len,
            db_params.partition_bitlen as usize,
        ),
        false => vec![],
    };

    // Encrypt and compress query
    let kw_enc_compressed = match with_default {
        true => client::encrypt_and_compress_query_shortint(&kw_decomposed, &cks),
        false => vec![],
    };

    let (bodies, sv_time) = time!(
        "Generate selection vector",
        client::generate_selection_vector(kw, &b, &params, &bff_params, &db_params)
    );

    // -----------------
    // Server ----------
    // -----------------

    let selection_vector =
        create_shortint_ciphertexts_from_raw_parts(&params, &masks, &bodies, bff_params.filter_len);

    // Decompress query
    let kw_enc = match with_default {
        true => decompress_shortint_from_slice(&kw_enc_compressed),
        false => vec![],
    };

    // Perform inner product
    let (response, ip_time) = time!("Perform inner product", {
        let result = server::client_aided_inner_product(&sks, &bff_params, &db, &selection_vector);

        let response = &result[bff_params.key_byte_len()..];
        let mut response = shortint_ciphertexts_to_fheuint64(&params, &response);

        if with_default {
            let selected_kw = &result[..bff_params.key_byte_len()];

            let d1 = shortint_ciphertexts_to_fheuint64(&params, &selected_kw); // digest retrieved from the inner product
            let d2 = shortint_ciphertexts_to_fheuint64(&params, &kw_enc); // digest shared by the client

            let sel = d1.eq(&d2);

            response = sel.select(&response, &FheUint64::encrypt_trivial(DEFAULT));
        }
        response
    });

    // Compress response
    let response_compressed = CompressedCiphertextListBuilder::new()
        .push(response)
        .build()
        .unwrap();

    // -----------------
    // Client ----------
    // -----------------

    // Decompress response
    let result: FheUint64 = response_compressed.get(0).unwrap().unwrap();

    // Decrypt response
    let result: u64 = result.decrypt(&ck);

    let offline_time = db_prep_time;
    let online_time = ip_time + sv_time;
    let total_time = start_time.elapsed();

    let expected_value = match db.get_data_by_keyword(kw) {
        Some(data) => from_decomposition(params.message_modulus.0 as u64, data),
        None => DEFAULT,
    };
    assert_eq!(result, expected_value);

    let mut total_comm_kb: f64 = 0.0;
    if with_default {
        calc_total_comm_cost(&kw_enc_compressed, &mut total_comm_kb);
    }
    calc_total_comm_cost(&bodies, &mut total_comm_kb);
    calc_total_comm_cost(&[response_compressed], &mut total_comm_kb);

    BenchResult {
        ip_time,
        sv_time,
        offline_time,
        online_time,
        total_time,
        total_comm_kb,
    }
}

criterion_group!(benches, benchmark_tkpir);
criterion_main!(benches);

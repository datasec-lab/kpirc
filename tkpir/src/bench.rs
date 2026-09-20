//! Disclaimer: Uses `#[test]` for basic benchmarks on stable Rust.  
//! Proper benchmarks require `#[bench]` (nightly) or `criterion`.
//!
//! Run with `cargo test bench::[TEST_NAME] --release -- --exact --nocapture`
//! Example: `cargo test bench::test_client_aided_kw_pir_with_default --release -- --exact --nocapture`

use super::*;
use crate::client;
use crate::common::*;
use crate::server;
use crate::utils::{tfhe_utils::*, *};
use core::time;
use rayon::ThreadPoolBuilder;
use std::time::Instant;
use tfhe::boolean::prelude::LweDimension;
use tfhe::boolean::prelude::PolynomialSize;
use tfhe::prelude::*;
use tfhe::shortint::ClientKey;
use tfhe::shortint::CompressedServerKey;
use tfhe::{
    core_crypto::{prelude::slice_algorithms::slice_wrapping_dot_product, seeders::new_seeder},
    prelude::*,
    set_server_key,
    shortint::{gen_keys, parameters::COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64},
    CompressedCiphertextList, CompressedCiphertextListBuilder, FheUint6, FheUint64,
};

// cargo test bench::bench_tcakpir_opt --release -- --exact --nocapture
#[test]
fn bench_tcakpir_opt() {
    set_num_threads(Some(1));

    let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;

    let lwe_dim = find_lwe_dim(&params);

    let max_keyword_bitlen = 8;
    let num_keywords = 1 << 8;
    let max_data_bitlen = 64;
    let partition_bitlen = params.message_modulus.0.ilog2() as u64;
    let db_params = DatabaseParameters::new(
        num_keywords,
        max_keyword_bitlen,
        max_data_bitlen,
        partition_bitlen,
    );
    let mut bff_params = ByteFuseFilterParameters {
        filter_len: 0,
        filter_seed: Vec::new(),
        digest_byte_len: 8,
        partition_bitlen: partition_bitlen as usize,
    };

    let mut seeder = new_seeder();
    let seeder = seeder.as_mut();

    let start_time = Instant::now();
    let start_time_offline = Instant::now();

    // -----------------
    //  Offline --------
    // -----------------

    // Server ----------
    // -----------------

    let db = time!(
        "Prepare database",
        server::prep_db::<u8>(&db_params, None, Some(&mut bff_params))
    )
    .0;

    let mask_seed = seeder.seed();

    // filter_len * lwe_dim
    let masks = time!(
        "Generate masks",
        generate_raw_masks(mask_seed, lwe_dim.0, bff_params.filter_len)
    )
    .0;

    let masked_data = time!(
        "Mask database",
        server::mask_database(&masks, lwe_dim.0, &db, &bff_params)
    )
    .0;

    // -----------------
    // Client ----------
    // -----------------

    let (cks, sks) = gen_keys(params);
    let (ck, sk) = shortint_key_to_client_key(&params, cks.clone());

    let (lwe_secret_key, noise_dist) = cks.encryption_key_and_noise();

    // 1 x lwe_dim
    let secret_key: &[u64] = lwe_secret_key.into_container();

    // 1 x filter_len
    let noise = time!(
        "Generate noise",
        client::generate_raw_noise(seeder.seed(), noise_dist, bff_params.filter_len)
    )
    .0;

    // 1 x filter_len
    let mut b = time!(
        "Generate raw bodies",
        client::generate_raw_bodies(&masks, secret_key, &noise, bff_params.filter_len)
    )
    .0;

    let end_time_offline = start_time_offline.elapsed();
    let start_time_online = Instant::now();

    // -----------------
    //  Online ---------
    // -----------------

    // Client ----------
    // -----------------

    let kw = 251;

    let kw_decomposed = time!(
        "Decompose hashed keyword",
        decompose_hashed(
            kw,
            bff_params.digest_byte_len,
            db_params.partition_bitlen as usize,
        )
    )
    .0;

    let kw_enc_compressed = time!(
        "Encrypt and compress query",
        client::encrypt_and_compress_query_shortint(&kw_decomposed, &cks)
    )
    .0;

    let bodies = time!(
        "Generate selection vector",
        client::generate_selection_vector(kw, &b, &params, &bff_params, &db_params)
    )
    .0;

    // -----------------
    // Server ----------
    // -----------------

    let kw_enc = time!(
        "Decompress query",
        decompress_shortint_from_slice(&kw_enc_compressed)
    )
    .0;

    let result = time!("Perform inner product", {
        let result = server::client_aided_inner_product_from_raw_parts(
            &bodies,
            &masked_data,
            &db,
            &params,
            &bff_params,
        );

        let num_partitions = db_params.num_partitions();
        let key_byte_len = bff_params.key_byte_len();
        let data = db.get_fused_data();

        let mut result = Vec::new();
        for pidx in 0..num_partitions + key_byte_len {
            let mut sum: u64 = 0;
            for ridx in 0..bff_params.filter_len as usize {
                sum += bodies[ridx].wrapping_mul(data[ridx][pidx] as u64);
            }
            let ct = create_shortint_ciphertext_from_raw_parts(&params, &masked_data[pidx], sum);
            result.push(ct);
        }

        let selected_kw = &result[..bff_params.key_byte_len()];

        let result = &result[bff_params.key_byte_len()..];
        let result = shortint_ciphertexts_to_fheuint64(&params, &result);

        let d1 = shortint_ciphertexts_to_fheuint64(&params, &selected_kw); // digest retrieved from the inner product
        let d2 = shortint_ciphertexts_to_fheuint64(&params, &kw_enc); // digest shared by the client

        let sel = d1.eq(&d2);

        sel.select(&result, &FheUint64::encrypt_trivial(DEFAULT))
    })
    .0;

    let result_compressed = time!(
        "Compress result",
        CompressedCiphertextListBuilder::new()
            .push(result)
            .build()
            .unwrap()
    )
    .0;

    // -----------------
    // Client ----------
    // -----------------

    let result: FheUint64 = time!(
        "Decompress result",
        result_compressed.get(0).unwrap().unwrap()
    )
    .0;

    let result: u64 = time!("Decrypt result", result.decrypt(&ck)).0;

    let end_time_online = start_time_online.elapsed();
    let end_time = start_time.elapsed();

    let expected_value = match db.get_data_by_keyword(kw) {
        Some(data) => from_decomposition(
            params.message_modulus.0 as u64,
            db.get_data_by_keyword(kw).unwrap(),
        ),
        None => DEFAULT,
    };
    assert_eq!(result, expected_value);

    println!("Offline time: {:?}", end_time_offline);
    println!("Online time: {:?}", end_time_online);
    println!("Total time: {:?}", end_time);

    let mut total_com: f64 = 0.0;
    calc_total_comm_cost(&kw_enc_compressed, &mut total_com);
    calc_total_comm_cost(&bodies, &mut total_com);
    calc_total_comm_cost(&[result_compressed], &mut total_com);

    println!("Total communication cost: {} KB", total_com);
}

// cargo test bench::bench_tcakpir --release -- --exact --nocapture
#[test]
fn bench_tcakpir() {
    set_num_threads(Some(1));

    let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;

    let lwe_dim = find_lwe_dim(&params);

    let max_keyword_bitlen = 8;
    let num_keywords = 1 << 6;
    let max_data_bitlen = 4;
    let partition_bitlen = params.message_modulus.0.ilog2() as u64;
    let db_params = DatabaseParameters::new(
        num_keywords,
        max_keyword_bitlen,
        max_data_bitlen,
        partition_bitlen,
    );
    let mut bff_params = ByteFuseFilterParameters {
        filter_len: 0,
        filter_seed: Vec::new(),
        digest_byte_len: 8,
        partition_bitlen: partition_bitlen as usize,
    };

    let mut seeder = new_seeder();
    let seeder = seeder.as_mut();

    let start_time = Instant::now();
    let start_time_offline = Instant::now();

    // -----------------
    //  Offline --------
    // -----------------

    // Server ----------
    // -----------------

    let db = time!(
        "Prepare database",
        server::prep_db::<u8>(&db_params, None, Some(&mut bff_params))
    )
    .0;

    // -----------------
    // Client ----------
    // -----------------

    let (cks, sks) = gen_keys(params);
    let (ck, sk) = time!(
        "Convert key to client key",
        shortint_key_to_client_key(&params, cks.clone())
    )
    .0;

    let (lwe_secret_key, noise_dist) = cks.encryption_key_and_noise();

    // 1 x lwe_dim
    let secret_key: &[u64] = lwe_secret_key.into_container();

    let end_time_offline = start_time_offline.elapsed();
    let start_time_online = Instant::now();

    // -----------------
    //  Online ---------
    // -----------------

    // Client ----------
    // -----------------

    let mask_seed = seeder.seed();

    // filter_len * lwe_dim
    let masks = time!(
        "Generate masks",
        generate_raw_masks(mask_seed, lwe_dim.0, bff_params.filter_len)
    )
    .0;

    // 1 x filter_len
    let noise = time!(
        "Generate noise",
        client::generate_raw_noise(seeder.seed(), noise_dist, bff_params.filter_len)
    )
    .0;

    // 1 x filter_len
    let mut b = time!(
        "Generate raw bodies",
        client::generate_raw_bodies(&masks, secret_key, &noise, bff_params.filter_len)
    )
    .0;

    let kw = 251;

    let kw_decomposed = time!(
        "Decompose hashed keyword",
        decompose_hashed(
            kw,
            bff_params.digest_byte_len,
            db_params.partition_bitlen as usize,
        )
    )
    .0;

    let kw_enc_compressed = time!(
        "Encrypt and compress query",
        client::encrypt_and_compress_query_shortint(&kw_decomposed, &cks)
    )
    .0;

    let bodies = time!(
        "Generate selection vector",
        client::generate_selection_vector(kw, &b, &params, &bff_params, &db_params)
    )
    .0;

    // -----------------
    // Server ----------
    // -----------------

    let selection_vector =
        create_shortint_ciphertexts_from_raw_parts(&params, &masks, &bodies, bff_params.filter_len);

    let kw_enc = time!(
        "Decompress query",
        decompress_shortint_from_slice(&kw_enc_compressed)
    )
    .0;

    let result = time!("Perform inner product", {
        let result = server::client_aided_inner_product(&sks, &bff_params, &db, &selection_vector);

        let selected_kw = &result[..bff_params.key_byte_len()];

        let result = &result[bff_params.key_byte_len()..];
        let result = shortint_ciphertexts_to_fheuint64(&params, &result);

        let d1 = shortint_ciphertexts_to_fheuint64(&params, &selected_kw); // digest retrieved from the inner product
        let d2 = shortint_ciphertexts_to_fheuint64(&params, &kw_enc); // digest shared by the client

        let sel = d1.eq(&d2);

        sel.select(&result, &FheUint64::encrypt_trivial(DEFAULT))
    })
    .0;

    let result_compressed = time!(
        "Compress result",
        CompressedCiphertextListBuilder::new()
            .push(result)
            .build()
            .unwrap()
    )
    .0;

    // -----------------
    // Client ----------
    // -----------------

    let result: FheUint64 = time!(
        "Decompress result",
        result_compressed.get(0).unwrap().unwrap()
    )
    .0;
    let result: u64 = time!("Decrypt result", result.decrypt(&ck)).0;

    let end_time_online = start_time_online.elapsed();
    let end_time = start_time.elapsed();

    let expected_value = match db.get_data_by_keyword(kw) {
        Some(data) => from_decomposition(
            params.message_modulus.0 as u64,
            db.get_data_by_keyword(kw).unwrap(),
        ),
        None => DEFAULT,
    };
    assert_eq!(result, expected_value);

    println!("Offline time: {:?}", end_time_offline);
    println!("Online time: {:?}", end_time_online);
    println!("Total time: {:?}", end_time);

    let mut total_com: f64 = 0.0;
    calc_total_comm_cost(&kw_enc_compressed, &mut total_com);
    calc_total_comm_cost(&bodies, &mut total_com);
    calc_total_comm_cost(&[result_compressed], &mut total_com);

    println!("Total communication cost: {} KB", total_com);
}

// cargo test bench::bench_tcwkpir --release -- --exact --nocapture
#[test]
fn bench_tcwkpir() {
    set_num_threads(Some(1));

    let mut params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;

    let max_keyword_bitlen = 8;
    let num_keywords = 1 << 8;
    let max_data_bitlen = 64;
    let partition_bitlen = params.message_modulus.0.ilog2() as u64;

    let cw_params = ConstantWeightParameters::new(2, max_keyword_bitlen);

    // Server
    let db_params = DatabaseParameters::new(
        num_keywords,
        max_keyword_bitlen,
        max_data_bitlen,
        partition_bitlen,
    );
    let db = server::prep_db::<u8>(&db_params, Some(&cw_params), None);

    // Client
    let cks = ClientKey::new(params);
    let compressed_sks = CompressedServerKey::new(&cks);
    let sks = compressed_sks.decompress();
    let (ck, sk) = shortint_key_to_client_key(&params, cks.clone());

    println!(
        "compressed sks size (KB) : {}",
        bincode::serialize(&compressed_sks).unwrap().len() as f64 / 1024.0
    );

    let step = 1 << (max_keyword_bitlen - (num_keywords as f64).log2().ceil() as u64);
    let x = 3 * step;
    println!("Keyword queried: {}", x);

    let start_time = Instant::now();

    let query = client::encode_to_cw_codeword(x, &cw_params);

    let compressed_query = time!(
        "Encrypt and compress query",
        client::encrypt_and_compress_query_shortint(&query, &cks)
    )
    .0;

    // Server
    let decompressed_query = time!(
        "Decompress query",
        decompress_shortint_from_slice(&compressed_query)
    )
    .0;

    let selection_vector = time!(
        "Generate selection vector",
        server::generate_selection_vector_shortint(&sks, &db, &decompressed_query)
    )
    .0;

    let result = time!(
        "Perform inner product",
        server::select(
            Scheme::TCWKPIR,
            &params,
            &sks,
            num_keywords,
            0,
            db_params.num_partitions(),
            &selection_vector,
            db.get_data(),
            Some(0)
        )
    )
    .0;

    let result_compressed = time!(
        "Compress result",
        CompressedCiphertextListBuilder::new()
            .push(result)
            .build()
            .unwrap()
    )
    .0;

    // Client
    let result: FheUint64 = time!(
        "Decompress result",
        result_compressed.get(0).unwrap().unwrap()
    )
    .0;

    let result: u64 = time!("Decrypt result", result.decrypt(&ck)).0;

    let expected_value = from_decomposition(
        params.message_modulus.0 as u64,
        db.get_data_by_keyword(x).unwrap(),
    );
    assert_eq!(result, expected_value);

    // Total time
    let elapsed_time = start_time.elapsed();

    println!("Total time: {:?}", elapsed_time);

    let mut total_com: f64 = 0.0;
    calc_total_comm_cost(&compressed_query, &mut total_com);
    calc_total_comm_cost(&[result_compressed], &mut total_com);

    println!("Total communication cost: {} KB", total_com);
}

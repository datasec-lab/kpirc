#![allow(warnings)]

pub mod bench;
pub mod byte_fuse_filter;
pub mod client;
pub mod common;
pub mod evaluate;
pub mod server;
pub mod utils;

#[cfg(test)]
mod tests {
    use crate::byte_fuse_filter::abs_arity3_byte_fuse_instances::AbsArity3ByteFuseInstance;
    use crate::client::generate_raw_bodies;

    use super::*;
    use bincode::serialize;
    use common::*;
    use core::num;
    use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
    use std::fs::{create_dir_all, File, OpenOptions};
    use std::io::Write;
    use std::mem::size_of;
    use std::path::PathBuf;
    use std::time::Instant;
    use std::{u64, u8};
    use tfhe::boolean::prelude::{
        params, DynamicDistribution, GlweDimension, LweDimension, PolynomialSize, StandardDev,
    };
    use tfhe::core_crypto::commons::math::random::{
        Distribution, RandomGenerable, RandomGenerator,
    };
    use tfhe::core_crypto::prelude::{
        generate_binary_lwe_secret_key, lwe_ciphertext_add_assign, lwe_ciphertext_cleartext_mul,
        ActivatedRandomGenerator, Cleartext, FloatingPoint, Gaussian, GlweSecretKeyOwned,
        LweSecretKeyOwned, Plaintext,
    };
    use tfhe::shortint::list_compression::{CompressionKey, DecompressionKey};
    use tfhe::shortint::parameters::{Degree, NoiseLevel};
    use tfhe::shortint::{
        Ciphertext, ClassicPBSParameters, ClientKey, CompressedCiphertext, CompressedServerKey,
        ServerKey, ShortintParameterSet,
    };
    use tfhe::{
        core_crypto::{prelude::slice_algorithms::slice_wrapping_dot_product, seeders::new_seeder},
        prelude::*,
        set_server_key,
        shortint::{gen_keys, parameters::COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64},
        CompressedCiphertextList, CompressedCiphertextListBuilder, FheUint64,
    };
    use tfhe::{prelude::*, Seed};
    use utils::{tfhe_utils::*, *};

    const DEFAULT_NUM_WARMUP_RUNS: usize = 0;
    const DEFAULT_NUM_MEASURED_RUNS: usize = 1;

    fn average(values: &[f64]) -> f64 {
        values.iter().sum::<f64>() / values.len() as f64
    }

    /// Print parameters
    fn print_config(
        params: &ClassicPBSParameters,
        db_params: &DatabaseParameters,
        scheme: &Scheme,
    ) {
        let max_data_bitlen = db_params.max_data_bitlen;
        let num_keywords = db_params.num_keywords;

        println!("Scheme: {:?}", scheme);

        println!("DB parameters: {:?}", db_params);

        let data_size_b = max_data_bitlen as f64 / 8.0;
        println!("Entry size: {} B", data_size_b);
        let db_payload_size_kb = num_keywords as f64 * max_data_bitlen as f64 / 8.0 / 1024.0;
        println!("Database payload size: {} KB", db_payload_size_kb);
    }

    fn parse_env_u64(name: &str, default: u64) -> u64 {
        match std::env::var(name) {
            Ok(value) => value
                .parse::<u64>()
                .unwrap_or_else(|_| panic!("Invalid value for {}: {}", name, value)),
            Err(_) => default,
        }
    }

    fn parse_env_usize(name: &str, default: usize) -> usize {
        match std::env::var(name) {
            Ok(value) => value
                .parse::<usize>()
                .unwrap_or_else(|_| panic!("Invalid value for {}: {}", name, value)),
            Err(_) => default,
        }
    }

    fn run_counts() -> (usize, usize, usize) {
        let num_warmup_runs = parse_env_usize("TKPIR_NUM_WARMUP_RUNS", DEFAULT_NUM_WARMUP_RUNS);
        let num_measured_runs =
            parse_env_usize("TKPIR_NUM_MEASURED_RUNS", DEFAULT_NUM_MEASURED_RUNS);

        assert!(num_measured_runs > 0, "TKPIR_NUM_MEASURED_RUNS must be > 0");

        let total_runs = num_warmup_runs + num_measured_runs;
        (num_warmup_runs, num_measured_runs, total_runs)
    }

    fn parse_env_scheme(default: Scheme) -> Scheme {
        match std::env::var("TKPIR_SCHEME") {
            Ok(raw) => match raw.trim().to_ascii_uppercase().as_str() {
                "TCWKPIR" => Scheme::TCWKPIR,
                "TCAKPIR" => Scheme::TCAKPIR,
                "TCAKPIR_OPT" => Scheme::TCAKPIR_OPT,
                _ => panic!(
                    "Invalid value for TKPIR_SCHEME: {} (expected TCWKPIR, TCAKPIR, or TCAKPIR_OPT)",
                    raw
                ),
            },
            Err(_) => default,
        }
    }

    fn parse_env_threads(default: Option<usize>) -> Option<usize> {
        match std::env::var("TKPIR_NUM_THREADS") {
            Ok(raw) => {
                if raw.trim().eq_ignore_ascii_case("none") {
                    None
                } else {
                    Some(
                        raw.parse::<usize>().unwrap_or_else(|_| {
                            panic!("Invalid value for TKPIR_NUM_THREADS: {}", raw)
                        }),
                    )
                }
            }
            Err(_) => default,
        }
    }

    // cargo test tests::test_tkpir --release -- --exact --nocapture >> output.log 2>&1
    #[test]
    fn test_tkpir() {
        // Runtime knobs (env vars):
        // - TKPIR_NUM_THREADS: integer or "none"
        // - TKPIR_SCHEME: TCWKPIR | TCAKPIR | TCAKPIR_OPT
        // - TKPIR_MAX_KEYWORD_BITLEN: u64
        // - TKPIR_MAX_DATA_BITLEN: u64
        set_num_threads(parse_env_threads(None));

        let scheme = parse_env_scheme(Scheme::TCWKPIR);

        let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;

        let max_keyword_bitlen = parse_env_u64("TKPIR_MAX_KEYWORD_BITLEN", 16);
        assert!(
            max_keyword_bitlen < usize::BITS as u64,
            "TKPIR_MAX_KEYWORD_BITLEN ({}) must be < {} on this platform",
            max_keyword_bitlen,
            usize::BITS
        );
        let num_keywords = 1usize << (max_keyword_bitlen as usize);
        let max_data_bitlen = parse_env_u64("TKPIR_MAX_DATA_BITLEN", 8);

        let partition_bitlen = params.message_modulus.0.ilog2() as u64;
        let db_params = DatabaseParameters::new(
            num_keywords,
            max_keyword_bitlen,
            max_data_bitlen,
            partition_bitlen,
        );

        print_config(&params, &db_params, &scheme);

        // KPIR test cases
        // KPIR-D test cases are in tkpir/src/bench.rs
        match scheme {
            Scheme::TCWKPIR => run_tcwkpir(params, &db_params),
            Scheme::TCAKPIR => run_tcakpir(params, &db_params),
            Scheme::TCAKPIR_OPT => run_tcakpir_opt(params, &db_params),
        }
    }

    fn comm_cost_kb<T>(data: &[T]) -> f64
    where
        T: serde::Serialize,
    {
        let bytes: usize = data.iter().map(|item| serialize(item).unwrap().len()).sum();

        bytes as f64 / 1024.0
    }

    /// without default
    fn run_tcakpir_opt(params: ClassicPBSParameters, db_params: &DatabaseParameters) {
        let (num_warmup_runs, num_measured_runs, total_runs) = run_counts();

        let lwe_dim = find_lwe_dim(&params);
        let partition_bitlen = db_params.partition_bitlen;
        let num_keywords = db_params.num_keywords;
        let max_keyword_bitlen = db_params.max_keyword_bitlen;
        let num_partitions = db_params.num_partitions();

        let mut bff_params = ByteFuseFilterParameters {
            filter_len: 0,
            filter_seed: Vec::new(),
            digest_byte_len: 8,
            partition_bitlen: partition_bitlen as usize,
        };
        let key_byte_len = bff_params.key_byte_len();

        let mut seeder = new_seeder();
        let seeder = seeder.as_mut();

        // -----------------
        //  Offline --------
        // -----------------

        // Server ----------
        // -----------------

        let db = time!(
            "Preprocess database:",
            server::prep_db::<u8>(&db_params, None, Some(&mut bff_params))
        )
        .0;

        // Repeat offline preprocessing to get a more robust runtime estimate.
        // Only the last generated masks/masked database are used afterward.
        let mut offline_times_sec = Vec::with_capacity(num_measured_runs);
        let mut last_masks = None;
        let mut last_masked_data = None;

        for run_idx in 0..total_runs {
            let mask_seed = seeder.seed();
            let start_time_offline = Instant::now();

            // filter_len * lwe_dim
            let masks = generate_raw_masks(mask_seed, lwe_dim.0, bff_params.filter_len);

            // num_partitions * lwe_dim
            let masked_data = server::mask_database(&masks, lwe_dim.0, &db, &bff_params);

            if run_idx >= num_warmup_runs {
                offline_times_sec.push(start_time_offline.elapsed().as_secs_f64());
            }

            last_masks = Some(masks);
            last_masked_data = Some(masked_data);
        }

        let avg_offline_sec = average(&offline_times_sec);
        let masks = last_masks.expect("offline masks should be generated");
        let masked_data = last_masked_data.expect("offline masked data should be generated");

        // -----------------
        // Client ----------
        // -----------------

        let start_time_gen_keys = Instant::now();

        let (cks, sks) = gen_keys(params);
        let (lwe_secret_key, noise_dist) = cks.encryption_key_and_noise();

        let private_comp_key =
            cks.new_compression_private_key(COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64);
        let (comp_key, decomp_key) = cks.new_compression_decompression_keys(&private_comp_key);

        // 1 x lwe_dim
        let secret_key: &[u64] = lwe_secret_key.into_container();

        println!("Generate keys took: {:?}", start_time_gen_keys.elapsed());

        // -----------------
        //  Online ---------
        // -----------------

        // Client ----------
        // -----------------

        let step = 1 << (max_keyword_bitlen - (num_keywords as f64).log2().ceil() as u64);
        let kw: u64 = 3 * step;

        let mut online_times_sec = Vec::with_capacity(num_measured_runs);
        let mut client_gen_times_sec = Vec::with_capacity(num_measured_runs);
        let mut client_decode_times_sec = Vec::with_capacity(num_measured_runs);
        let mut client_times_sec = Vec::with_capacity(num_measured_runs);
        let mut uploads_kb = Vec::with_capacity(num_measured_runs);
        let mut downloads_kb = Vec::with_capacity(num_measured_runs);

        for run_idx in 0..total_runs {
            let start_time_online = Instant::now();

            let start_client = Instant::now();

            // 1 x filter_len
            let noise =
                client::generate_raw_noise(seeder.seed(), noise_dist, bff_params.filter_len);

            // 1 x filter_len
            let b = client::generate_raw_bodies(&masks, secret_key, &noise, bff_params.filter_len);

            let bodies =
                client::generate_selection_vector(kw, &b, &params, &bff_params, &db_params);

            let client_gen_time = start_client.elapsed().as_secs_f64();

            // -----------------
            // Server ----------
            // -----------------

            let result = server::client_aided_inner_product_from_raw_parts(
                &bodies,
                &masked_data,
                &db,
                &params,
                &bff_params,
            );

            let result_compressed = compress_shortint(&comp_key, &result);

            // -----------------
            // Client ----------
            // -----------------

            let start_client_decode = Instant::now();
            let result = decompress_shortint(&decomp_key, &result_compressed);
            let result: Vec<u8> = result.iter().map(|enc| cks.decrypt(enc) as u8).collect();
            let client_decode_time = start_client_decode.elapsed().as_secs_f64();
            let client_time_sec = client_gen_time + client_decode_time;

            let online_time_sec = start_time_online.elapsed().as_secs_f64();

            assert_eq!(
                &result[bff_params.key_byte_len()..],
                db.get_data_by_keyword(kw).unwrap()
            );

            let mut upload: f64 = 0.0;
            calc_total_comm_cost(&bodies, &mut upload);

            let mut download: f64 = 0.0;
            calc_total_comm_cost(&[result_compressed], &mut download);

            if run_idx >= num_warmup_runs {
                online_times_sec.push(online_time_sec);
                client_gen_times_sec.push(client_gen_time);
                client_decode_times_sec.push(client_decode_time);
                client_times_sec.push(client_time_sec);
                uploads_kb.push(upload);
                downloads_kb.push(download);
            }
        }

        let avg_online_sec = average(&online_times_sec);
        let avg_client_gen_sec = average(&client_gen_times_sec);
        let avg_client_decode_sec = average(&client_decode_times_sec);
        let avg_client_sec = average(&client_times_sec);
        let avg_upload_kb = average(&uploads_kb);
        let avg_download_kb = average(&downloads_kb);
        let avg_total_sec = avg_offline_sec + avg_online_sec;

        println!(
            "Avg offline time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_offline_sec
        );
        println!(
            "Avg online time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_online_sec
        );
        println!(
            "Avg client-side encode time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_client_gen_sec
        );
        println!(
            "Avg client-side decode time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_client_decode_sec
        );
        println!(
            "Avg client-side time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_client_sec
        );
        println!("Avg total time: {:.6} s", avg_total_sec);
        println!("Avg upload size: {} KB", avg_upload_kb);
        println!("Avg download size: {} KB", avg_download_kb);
        println!(
            "Avg total communication: {} KB",
            avg_upload_kb + avg_download_kb
        );
    }

    /// without default
    fn run_tcakpir(params: ClassicPBSParameters, db_params: &DatabaseParameters) {
        let (num_warmup_runs, num_measured_runs, total_runs) = run_counts();

        let lwe_dim = find_lwe_dim(&params);
        let partition_bitlen = db_params.partition_bitlen;
        let num_keywords = db_params.num_keywords;
        let max_keyword_bitlen = db_params.max_keyword_bitlen;
        let num_partitions = db_params.num_partitions();

        let mut bff_params = ByteFuseFilterParameters {
            filter_len: 0,
            filter_seed: Vec::new(),
            digest_byte_len: 8,
            partition_bitlen: partition_bitlen as usize,
        };
        let key_byte_len = bff_params.key_byte_len();

        let mut seeder = new_seeder();
        let seeder = seeder.as_mut();

        // -----------------
        //  Offline --------
        // -----------------

        // Server ----------
        // -----------------

        let db = time!(
            "Preprocess database:",
            server::prep_db::<u8>(&db_params, None, Some(&mut bff_params))
        )
        .0;

        // -----------------
        // Client ----------
        // -----------------

        let start_time_gen_keys = Instant::now();

        let (cks, sks) = gen_keys(params);

        let private_comp_key =
            cks.new_compression_private_key(COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64);
        let (comp_key, decomp_key) = cks.new_compression_decompression_keys(&private_comp_key);

        let (lwe_secret_key, noise_dist) = cks.encryption_key_and_noise();

        // 1 x lwe_dim
        let secret_key: &[u64] = lwe_secret_key.into_container();

        println!("Generate keys took: {:?}", start_time_gen_keys.elapsed());

        // -----------------
        //  Online ---------
        // -----------------

        // Client ----------
        // -----------------

        let step = 1 << (max_keyword_bitlen - (num_keywords as f64).log2().ceil() as u64);
        let kw: u64 = 3 * step;

        let mut online_times_sec = Vec::with_capacity(num_measured_runs);
        let mut client_gen_times_sec = Vec::with_capacity(num_measured_runs);
        let mut client_decode_times_sec = Vec::with_capacity(num_measured_runs);
        let mut client_times_sec = Vec::with_capacity(num_measured_runs);
        let mut selection_vector_times_sec = Vec::with_capacity(num_measured_runs);
        let mut inner_product_times_sec = Vec::with_capacity(num_measured_runs);
        let mut uploads_kb = Vec::with_capacity(num_measured_runs);
        let mut downloads_kb = Vec::with_capacity(num_measured_runs);

        for run_idx in 0..total_runs {
            let start_time_online = Instant::now();

            let mask_seed = seeder.seed();

            // filter_len * lwe_dim
            let masks = generate_raw_masks(mask_seed, lwe_dim.0, bff_params.filter_len);

            let start_client = Instant::now();

            // 1 x filter_len
            let noise =
                client::generate_raw_noise(seeder.seed(), noise_dist, bff_params.filter_len);

            // 1 x filter_len
            let b = client::generate_raw_bodies(&masks, secret_key, &noise, bff_params.filter_len);

            let start_selection_vector = Instant::now();
            let bodies =
                client::generate_selection_vector(kw, &b, &params, &bff_params, &db_params);
            let selection_vector_time_sec = start_selection_vector.elapsed().as_secs_f64();
            let client_gen_time = start_client.elapsed().as_secs_f64();

            // -----------------
            // Server ----------
            // -----------------

            let sv = create_shortint_ciphertexts_from_raw_parts(
                &params,
                &masks,
                &bodies,
                bff_params.filter_len,
            );

            let start_inner_product = Instant::now();
            let result = server::client_aided_inner_product(&sks, &bff_params, &db, &sv);
            let inner_product_time_sec = start_inner_product.elapsed().as_secs_f64();

            let result_compressed = compress_shortint(&comp_key, &result);

            // -----------------
            // Client ----------
            // -----------------

            let start_client_decode = Instant::now();
            let result = decompress_shortint(&decomp_key, &result_compressed);
            let result: Vec<u8> = result.iter().map(|enc| cks.decrypt(enc) as u8).collect();
            let client_decode_time = start_client_decode.elapsed().as_secs_f64();
            let client_time_sec = client_gen_time + client_decode_time;

            let online_time_sec = start_time_online.elapsed().as_secs_f64();

            assert_eq!(
                &result[bff_params.key_byte_len()..],
                db.get_data_by_keyword(kw).unwrap()
            );

            let mut upload: f64 = 0.0;
            calc_total_comm_cost(&bodies, &mut upload);

            let mut download: f64 = 0.0;
            calc_total_comm_cost(&[result_compressed], &mut download);

            if run_idx >= num_warmup_runs {
                online_times_sec.push(online_time_sec);
                client_gen_times_sec.push(client_gen_time);
                client_decode_times_sec.push(client_decode_time);
                client_times_sec.push(client_time_sec);
                selection_vector_times_sec.push(selection_vector_time_sec);
                inner_product_times_sec.push(inner_product_time_sec);
                uploads_kb.push(upload);
                downloads_kb.push(download);
            }
        }

        let avg_online_sec = average(&online_times_sec);
        let avg_client_gen_sec = average(&client_gen_times_sec);
        let avg_client_decode_sec = average(&client_decode_times_sec);
        let avg_client_sec = average(&client_times_sec);
        let avg_selection_vector_sec = average(&selection_vector_times_sec);
        let avg_inner_product_sec = average(&inner_product_times_sec);
        let avg_upload_kb = average(&uploads_kb);
        let avg_download_kb = average(&downloads_kb);
        let avg_total_sec = avg_online_sec;

        println!(
            "Avg online time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_online_sec
        );
        println!(
            "Avg client-side encode time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_client_gen_sec
        );
        println!(
            "Avg client-side decode time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_client_decode_sec
        );
        println!(
            "Avg client-side time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_client_sec
        );
        println!(
            "Avg selection vector time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_selection_vector_sec
        );
        println!(
            "Avg inner product time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_inner_product_sec
        );
        println!("Avg total time: {:.6} s", avg_total_sec);
        println!("Avg upload size: {} KB", avg_upload_kb);
        println!("Avg download size: {} KB", avg_download_kb);
        println!(
            "Avg total communication: {} KB",
            avg_upload_kb + avg_download_kb
        );
    }

    /// without default
    fn run_tcwkpir(params: ClassicPBSParameters, db_params: &DatabaseParameters) {
        let (num_warmup_runs, num_measured_runs, total_runs) = run_counts();

        let num_keywords = db_params.num_keywords;
        let max_keyword_bitlen = db_params.max_keyword_bitlen;
        let cw_params = ConstantWeightParameters::new(2, max_keyword_bitlen);

        // -----------------
        // Server ----------
        // -----------------

        let db = time!(
            "Preprocess database:",
            server::prep_db::<u8>(&db_params, Some(&cw_params), None)
        )
        .0;

        // -----------------
        // Client ----------
        // -----------------

        let start_time_gen_keys = Instant::now();

        let (cks, sks) = gen_keys(params);
        let private_comp_key =
            cks.new_compression_private_key(COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64);
        let (comp_key, decomp_key) = cks.new_compression_decompression_keys(&private_comp_key);

        let time_gen_keys = start_time_gen_keys.elapsed();
        println!("Generate keys took: {:?}", time_gen_keys);

        let step = 1 << (max_keyword_bitlen - (num_keywords as f64).log2().ceil() as u64);
        let x = 3 * step;
        println!("Keyword queried: {}", x);

        let mut online_times_sec = Vec::with_capacity(num_measured_runs);
        let mut client_gen_times_sec = Vec::with_capacity(num_measured_runs);
        let mut client_decode_times_sec = Vec::with_capacity(num_measured_runs);
        let mut client_times_sec = Vec::with_capacity(num_measured_runs);
        let mut selection_vector_times_sec = Vec::with_capacity(num_measured_runs);
        let mut inner_product_times_sec = Vec::with_capacity(num_measured_runs);
        let mut uploads_kb = Vec::with_capacity(num_measured_runs);
        let mut downloads_kb = Vec::with_capacity(num_measured_runs);

        for run_idx in 0..total_runs {
            let start_time_online = Instant::now();

            let start_client = Instant::now();
            let query = client::encode_to_cw_codeword(x, &cw_params);
            let compressed_query = client::encrypt_and_compress_query_shortint(&query, &cks);
            let client_gen_time = start_client.elapsed().as_secs_f64();

            // -----------------
            // Server ----------
            // -----------------

            let decompressed_query = decompress_shortint_from_slice(&compressed_query);

            let start_selection_vector = Instant::now();
            let selection_vector =
                server::generate_selection_vector_shortint(&sks, &db, &decompressed_query);
            let selection_vector_time_sec = start_selection_vector.elapsed().as_secs_f64();

            let start_inner_product = Instant::now();
            let result = server::inner_product_lwe(&sks, &db, &selection_vector);
            let inner_product_time_sec = start_inner_product.elapsed().as_secs_f64();

            let result_compressed = compress_shortint(&comp_key, &result);

            // Client
            let start_client_decode = Instant::now();
            let result = decompress_shortint(&decomp_key, &result_compressed);
            let result: Vec<u64> = result.iter().map(|enc| cks.decrypt(enc)).collect();
            let client_decode_time = start_client_decode.elapsed().as_secs_f64();
            let client_time_sec = client_gen_time + client_decode_time;

            let online_time_sec = start_time_online.elapsed().as_secs_f64();

            let result: Vec<u8> = result.iter().map(|r| *r as u8).collect();

            assert_eq!(&result, db.get_data_by_keyword(x).unwrap());

            let mut upload = 0.0;
            calc_total_comm_cost(&[compressed_query], &mut upload);

            let mut download = 0.0;
            calc_total_comm_cost(&[result_compressed], &mut download);

            if run_idx >= num_warmup_runs {
                online_times_sec.push(online_time_sec);
                client_gen_times_sec.push(client_gen_time);
                client_decode_times_sec.push(client_decode_time);
                client_times_sec.push(client_time_sec);
                selection_vector_times_sec.push(selection_vector_time_sec);
                inner_product_times_sec.push(inner_product_time_sec);
                uploads_kb.push(upload);
                downloads_kb.push(download);
            }
        }

        let avg_online_sec = average(&online_times_sec);
        let avg_client_gen_sec = average(&client_gen_times_sec);
        let avg_client_decode_sec = average(&client_decode_times_sec);
        let avg_client_sec = average(&client_times_sec);
        let avg_selection_vector_sec = average(&selection_vector_times_sec);
        let avg_inner_product_sec = average(&inner_product_times_sec);
        let avg_upload_kb = average(&uploads_kb);
        let avg_download_kb = average(&downloads_kb);
        let avg_total_sec = avg_online_sec;

        println!(
            "Avg online time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_online_sec
        );
        println!(
            "Avg client-side encode time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_client_gen_sec
        );
        println!(
            "Avg client-side decode time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_client_decode_sec
        );
        println!(
            "Avg client-side time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_client_sec
        );
        println!(
            "Avg selection vector time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_selection_vector_sec
        );
        println!(
            "Avg inner product time ({} runs, {} warmup): {:.6} s",
            num_measured_runs, num_warmup_runs, avg_inner_product_sec
        );
        println!("Avg total time: {:.6} s", avg_total_sec);
        println!("Avg upload size: {} KB", avg_upload_kb);
        println!("Avg download size: {} KB", avg_download_kb);
        println!(
            "Avg total communication: {} KB",
            avg_upload_kb + avg_download_kb
        );
    }

    // --
    // Simulations
    // -->

    fn calibrate_linear<F: Fn(usize) -> usize>(f: F, n0: usize, n1: usize) -> (f64, f64) {
        let y0 = f(n0) as f64;
        let y1 = f(n1) as f64;
        let slope = (y1 - y0) / (n1 - n0) as f64;
        let base = y0 - slope * n0 as f64;
        (base, slope)
    }

    fn estimate_kb(base: f64, slope: f64, n: usize) -> f64 {
        (base + slope * n as f64).max(0.0) / 1024.0
    }

    fn tcwkpir_download_bytes(
        num_data_blocks: usize,
        num_keywords: usize,
        max_keyword_bitlen: u64,
        params: ClassicPBSParameters,
        cks: &ClientKey,
        sks: &ServerKey,
        comp_key: &CompressionKey,
    ) -> usize {
        let partition_bitlen = params.message_modulus.0.ilog2() as u64;
        let db_params = DatabaseParameters::new(
            num_keywords,
            max_keyword_bitlen,
            num_data_blocks as u64 * partition_bitlen,
            partition_bitlen,
        );
        let cw_params = ConstantWeightParameters::new(2, db_params.max_keyword_bitlen);
        let db = server::prep_db::<u8>(&db_params, Some(&cw_params), None);
        let query = client::encode_to_cw_codeword(3, &cw_params);
        let compressed_query = client::encrypt_and_compress_query_shortint(&query, cks);
        let query = decompress_shortint_from_slice(&compressed_query);
        let selection_vector = server::generate_selection_vector_shortint(sks, &db, &query);
        let result = server::inner_product_lwe(sks, &db, &selection_vector);

        serialize(&compress_shortint(comp_key, &result)).unwrap().len()
    }

    fn tcakpir_download_bytes(
        num_data_blocks: usize,
        num_keywords: usize,
        max_keyword_bitlen: u64,
        params: ClassicPBSParameters,
        cks: &ClientKey,
        sks: &ServerKey,
        comp_key: &CompressionKey,
    ) -> usize {
        let partition_bitlen = params.message_modulus.0.ilog2() as u64;
        let db_params = DatabaseParameters::new(
            num_keywords,
            max_keyword_bitlen,
            num_data_blocks as u64 * partition_bitlen,
            partition_bitlen,
        );
        let mut bff_params = ByteFuseFilterParameters {
            filter_len: 0,
            filter_seed: Vec::new(),
            digest_byte_len: 8,
            partition_bitlen: partition_bitlen as usize,
        };
        let db = server::prep_db::<u8>(&db_params, None, Some(&mut bff_params));
        let (lwe_secret_key, noise_dist) = cks.encryption_key_and_noise();
        let mut seeder = new_seeder();
        let seeder = seeder.as_mut();
        let masks = generate_raw_masks(
            seeder.seed(),
            find_lwe_dim(&params).0,
            bff_params.filter_len,
        );
        let noise = client::generate_raw_noise(seeder.seed(), noise_dist, bff_params.filter_len);
        let bodies = client::generate_raw_bodies(
            &masks,
            lwe_secret_key.as_ref(),
            &noise,
            bff_params.filter_len,
        );
        let bodies = client::generate_selection_vector(
            3,
            &bodies,
            &params,
            &bff_params,
            &db_params,
        );
        let selection_vector = create_shortint_ciphertexts_from_raw_parts(
            &params,
            &masks,
            &bodies,
            bff_params.filter_len,
        );
        let result = server::client_aided_inner_product(sks, &bff_params, &db, &selection_vector);

        serialize(&compress_shortint(comp_key, &result)).unwrap().len()
    }

    // cargo test tests::test_communication_costs --release -- --ignored --exact --nocapture
    #[test]
    #[ignore]
    fn test_communication_costs() {
        set_num_threads(Some(1));

        let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;
        let partition_bitlen = params.message_modulus.0.ilog2() as u64;

        let (cks, sks) = gen_keys(params);
        let private_comp_key =
            cks.new_compression_private_key(COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64);
        let (comp_key, _decomp_key) = cks.new_compression_decompression_keys(&private_comp_key);

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .expect("Failed to locate repository root from CARGO_MANIFEST_DIR");
        let comm_costs_dir = repo_root.join("results").join("communication_costs");
        create_dir_all(&comm_costs_dir)
            .expect("Failed to create communication_costs results directory");

        let header = "scheme,sweep,max_keyword_bitlen,num_keywords,entry_size_b,total_data_gb,upload_kb,download_kb,total_kb";

        let mut tcakpir_file = File::create(comm_costs_dir.join("tcakpir_comm_costs.csv"))
            .expect("Failed to create tcakpir_comm_costs.csv");
        let mut tcwkpir_file = File::create(comm_costs_dir.join("tcwkpir_comm_costs.csv"))
            .expect("Failed to create tcwkpir_comm_costs.csv");
        writeln!(tcakpir_file, "{}", header).unwrap();
        writeln!(tcwkpir_file, "{}", header).unwrap();

        let mut write_rows_for_config = |sweep: &str, max_keyword_bitlen: u64, max_data_bitlen: u64| {
            let num_keywords = 1usize << max_keyword_bitlen;
            let db_params = DatabaseParameters::new(
                num_keywords,
                max_keyword_bitlen,
                max_data_bitlen,
                partition_bitlen,
            );

            let entry_size_b = db_params.max_data_bitlen as f64 / 8.0;
            let total_data_gb = db_params.num_keywords as f64
                * db_params.max_data_bitlen as f64
                / 8.0
                / (1024.0 * 1024.0 * 1024.0);

            let bff_params = ByteFuseFilterParameters {
                filter_len: 0,
                filter_seed: Vec::new(),
                digest_byte_len: 8,
                partition_bitlen: partition_bitlen as usize,
            };
            let value_byte_len = bff_params.key_byte_len() + db_params.num_partitions();
            let filter_len =
                AbsArity3ByteFuseInstance::new(db_params.num_keywords, value_byte_len).filter_len;

            let tca_blocks = db_params.num_partitions() + bff_params.key_byte_len();
            let tca_result = vec![sks.create_trivial(0); tca_blocks];
            let tca_result_compressed = compress_shortint(&comp_key, &tca_result);
            let tca_upload_kb = (filter_len * std::mem::size_of::<u64>()) as f64 / 1024.0;
            let tca_download_kb = comm_cost_kb(&[tca_result_compressed]);
            let tca_total_kb = tca_upload_kb + tca_download_kb;

            let tcakpir_row = format!(
                "TCAKPIR,{},{},{},{},{:.6},{:.6},{:.6},{:.6}",
                sweep,
                db_params.max_keyword_bitlen,
                db_params.num_keywords,
                entry_size_b,
                total_data_gb,
                tca_upload_kb,
                tca_download_kb,
                tca_total_kb
            );
            println!("{}", tcakpir_row);
            writeln!(tcakpir_file, "{}", tcakpir_row).unwrap();

            let cw_params = ConstantWeightParameters::new(2, db_params.max_keyword_bitlen);
            let compressed_query = client::encrypt_and_compress_query_shortint(
                &vec![0u8; cw_params.codeword_bitlen as usize],
                &cks,
            );
            let tcw_result = vec![sks.create_trivial(0); db_params.num_partitions()];
            let tcw_result_compressed = compress_shortint(&comp_key, &tcw_result);
            let tcw_upload_kb = comm_cost_kb(&[compressed_query]);
            let tcw_download_kb = comm_cost_kb(&[tcw_result_compressed]);
            let tcw_total_kb = tcw_upload_kb + tcw_download_kb;

            let tcwkpir_row = format!(
                "TCWKPIR,{},{},{},{},{:.6},{:.6},{:.6},{:.6}",
                sweep,
                db_params.max_keyword_bitlen,
                db_params.num_keywords,
                entry_size_b,
                total_data_gb,
                tcw_upload_kb,
                tcw_download_kb,
                tcw_total_kb
            );
            println!("{}", tcwkpir_row);
            writeln!(tcwkpir_file, "{}", tcwkpir_row).unwrap();
        };

        // Sweep #1: database cardinality, fixed entry size = 8 B
        let cardinality_entry_size_b = 8u64;
        let cardinality_max_data_bitlen = cardinality_entry_size_b * 8;
        for max_keyword_bitlen in 0u64..=20 {
            write_rows_for_config("cardinality", max_keyword_bitlen, cardinality_max_data_bitlen);
        }

        // Sweep #2: entry size, fixed database cardinality m = 2^16
        let fixed_max_keyword_bitlen = 16u64;
        for data_byte_len_pow in 0u32..=12 {
            let entry_size_b = 1u64 << data_byte_len_pow;
            let max_data_bitlen = entry_size_b * 8;
            write_rows_for_config("entry_size", fixed_max_keyword_bitlen, max_data_bitlen);
        }
    }

    // cargo test tests::test_selection_vector_memory_costs --release -- --ignored --exact --nocapture
    #[test]
    #[ignore]
    fn test_selection_vector_memory_costs() {
        set_num_threads(Some(1));

        let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;
        let partition_bitlen = params.message_modulus.0.ilog2() as u64;

        let (_cks, sks) = gen_keys(params);

        let ct_bytes = serialize(&sks.create_trivial(0)).unwrap().len();
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .expect("Failed to locate repository root from CARGO_MANIFEST_DIR");
        let sv_memory_dir = repo_root.join("results").join("sv_memory");
        create_dir_all(&sv_memory_dir).expect("Failed to create sv_memory results directory");

        let header = "kw_bitlen,total_sv_mb";
        let mut tcwkpir_file = File::create(sv_memory_dir.join("selection_vector_memory_tcwkpir.csv"))
            .expect("Failed to create selection_vector_memory_tcwkpir.csv");
        let mut tcakpir_file = File::create(sv_memory_dir.join("selection_vector_memory_tcakpir.csv"))
            .expect("Failed to create selection_vector_memory_tcakpir.csv");
        let mut tcakpir_opt_file =
            File::create(sv_memory_dir.join("selection_vector_memory_tcakpir_opt.csv"))
            .expect("Failed to create selection_vector_memory_tcakpir_opt.csv");
        writeln!(tcwkpir_file, "{}", header).unwrap();
        writeln!(tcakpir_file, "{}", header).unwrap();
        writeln!(tcakpir_opt_file, "{}", header).unwrap();

        // Sweep database sizes from 1 to 2^32 entries.
        let max_data_bitlen = 1u64 * 8;
        for k in (0u64..=32u64).step_by(1) {
            let num_keywords = 1usize << k;
            let db_params =
                DatabaseParameters::new(num_keywords, k, max_data_bitlen, partition_bitlen);

            let mut bff_params = ByteFuseFilterParameters {
                filter_len: 0,
                filter_seed: Vec::new(),
                digest_byte_len: 8,
                partition_bitlen: partition_bitlen as usize,
            };

            // Memory estimation only needs the filter length.
            let value_byte_len = bff_params.key_byte_len() + db_params.num_partitions();
            let bff_instance =
                AbsArity3ByteFuseInstance::new(db_params.num_keywords, value_byte_len);
            bff_params.filter_len = bff_instance.filter_len;

            let tcw_sv_bytes = db_params.num_keywords * ct_bytes;
            let tca_sv_bytes = bff_params.filter_len * ct_bytes;

            // In TCAKPIR_OPT the server operates on raw bodies instead of materialized ciphertext SV.
            let tca_opt_sv_bytes = bff_params.filter_len * size_of::<u64>();

            let tcw_mb = tcw_sv_bytes as f64 / (1024.0 * 1024.0);
            let tca_mb = tca_sv_bytes as f64 / (1024.0 * 1024.0);
            let tca_opt_mb = tca_opt_sv_bytes as f64 / (1024.0 * 1024.0);

            writeln!(tcwkpir_file, "{},{:.6}", db_params.max_keyword_bitlen, tcw_mb).unwrap();
            writeln!(tcakpir_file, "{},{:.6}", db_params.max_keyword_bitlen, tca_mb).unwrap();
            writeln!(tcakpir_opt_file, "{},{:.6}", db_params.max_keyword_bitlen, tca_opt_mb)
                .unwrap();
        }
    }

    // cargo test tests::test_single_tlwe_ciphertext_size --release -- --exact --nocapture
    #[test]
    fn test_single_tlwe_ciphertext_size() {
        let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;
        let lwe_dimension = find_lwe_dim(&params);
        let tlwe = tfhe::core_crypto::prelude::LweCiphertext::new(
            0u64,
            lwe_dimension.to_lwe_size(),
            params.ciphertext_modulus,
        );

        println!("TLWE ciphertext size: {} bytes", serialize(&tlwe).unwrap().len());
    }

    // cargo test tests::test_single_glwe_ciphertext_size --release -- --exact --nocapture
    #[test]
    fn test_single_glwe_ciphertext_size() {
        let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;
        let glwe = tfhe::core_crypto::prelude::GlweCiphertext::new(
            0u64,
            params.glwe_dimension.to_glwe_size(),
            params.polynomial_size,
            params.ciphertext_modulus,
        );

        println!("GLWE ciphertext size: {} bytes", serialize(&glwe).unwrap().len());
    }

    // cargo test tests::test_single_compressed_shortint_ciphertext_size --release -- --exact --nocapture
    #[test]
    fn test_single_compressed_shortint_ciphertext_size() {
        let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;
        let (cks, _sks) = gen_keys(params);
        let private_comp_key =
            cks.new_compression_private_key(COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64);
        let (comp_key, _decomp_key) = cks.new_compression_decompression_keys(&private_comp_key);
        let ciphertext = cks.encrypt(1);
        let compressed = compress_shortint(&comp_key, &[ciphertext]);
        let serialized_bytes = serialize(&compressed).unwrap().len();

        println!(
            "Compressed one-bit shortint download size: {serialized_bytes} bytes ({:.3} KB)",
            serialized_bytes as f64 / 1024.0
        );
    }

    // cargo test tests::test_inner_product_simulation --release -- --nocapture
    #[test]
    fn test_inner_product_simulation() {
        set_num_threads(None);

        let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;

        let num_keywords = 1 << 16;
        let num_bs = 1 << 9;
        let data_bitlen = 2;
        let num_iterations = 1024;
        let partition_bitlen = params.message_modulus.0.ilog2() as usize;
        let num_partitions = data_bitlen / partition_bitlen;

        assert!(data_bitlen % partition_bitlen == 0);

        let mut iv = vec![0u8; num_keywords];
        iv[0] = 1;

        let delta = (1_u64 << 63) / (params.message_modulus.0 * params.carry_modulus.0) as u64;
        let threshold = delta / 2;

        let file_path = format!(
            "errors_size{}_bs{}.log",
            (num_keywords as u64).ilog2(),
            (num_bs as u64).ilog2()
        );
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .expect("Failed to open or create file");

        for i in 0..num_iterations {
            // generate data with maximum possible value (worst case)
            let mask = (1 << partition_bitlen) - 1;
            // let max = u8::MAX; // worst case
            // let row = vec![max & mask; num_partitions];
            let rand = rand::random::<u8>() & mask;
            let row = vec![rand; num_partitions];
            let data = vec![row; num_keywords];

            let (errors, prod) =
                simulate_inner_product(params, num_keywords, num_bs, num_partitions, &iv, &data);

            for error in errors {
                let gap = (threshold as i64) - error.abs();
                writeln!(file, "{},{},{},{}", i + 1, error, data[0] == prod, gap).unwrap();
            }
        }
    }

    fn simulate_inner_product(
        params: ClassicPBSParameters,
        num_keywords: usize,
        num_bs: usize,
        num_partitions: usize,
        iv: &[u8],
        data: &Vec<Vec<u8>>,
    ) -> (Vec<i64>, Vec<u8>) {
        let (cks, sks) = simulate_gen_keys(params);
        let private_comp_key =
            cks.new_compression_private_key(COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64);
        let (comp_key, decomp_key) = cks.new_compression_decompression_keys(&private_comp_key);

        // simulate generating the selection vector
        let sv1 = iv
            .par_iter()
            .map(|&bit| simulate_encryption(params, &cks, bit as u64))
            .collect::<Vec<Ciphertext>>();
        let sv1 = simulate_compression_decompression(&comp_key, &decomp_key, &sv1);

        // let sv1 = iv
        //     .par_iter()
        //     .map(|&bit| cks.encrypt_compressed(bit as u64))
        //     .collect::<Vec<CompressedCiphertext>>();
        // let sv1 = decompress_shortint_from_slibce(&sv1);

        let sv2 = iv
            .par_iter()
            .map(|&bit| simulate_encryption(params, &cks, bit as u64))
            .collect::<Vec<Ciphertext>>();
        let sv2 = simulate_compression_decompression(&comp_key, &decomp_key, &sv2);

        // let sv2 = iv
        //     .par_iter()
        //     .map(|&bit| cks.encrypt_compressed(bit as u64))
        //     .collect::<Vec<CompressedCiphertext>>();
        // let sv2 = decompress_shortint_from_slice(&sv2);

        let sv = (0..num_keywords)
            .into_par_iter()
            .map(|idx| sks.mul(&sv1[idx], &sv2[idx]))
            .collect::<Vec<Ciphertext>>();

        // let delta = (1_u64 << 63) / (params.message_modulus.0 * params.carry_modulus.0) as u64;
        // println!("Delta/2: {}", (delta / 2) as i64);

        let result = (0..num_partitions)
            .into_par_iter()
            .map(|pidx| {
                let mut prod = sks.create_trivial(0);
                let mut sum = sks.create_trivial(0);

                for ridx in 0..num_keywords {
                    lwe_ciphertext_cleartext_mul(
                        &mut prod.ct,
                        &sv[ridx].ct,
                        Cleartext(data[ridx][pidx] as u64),
                    );

                    lwe_ciphertext_add_assign(&mut sum.ct, &prod.ct);

                    if ridx > 0 && ridx % (num_bs - 1) == 0 {
                        sks.message_extract_assign(&mut sum);
                    }
                }

                Ciphertext::new(
                    sum.ct,
                    Degree::new(sks.message_modulus.0 - 1),
                    NoiseLevel::NOMINAL,
                    sks.message_modulus,
                    sks.carry_modulus,
                    sks.pbs_order,
                )
            })
            .collect::<Vec<Ciphertext>>();

        let result = simulate_compression_decompression(&comp_key, &decomp_key, &result);

        let errors = (0..num_partitions)
            .into_par_iter()
            .map(|idx| compute_error(&cks, data[0][idx] as u64, &result[idx]) as i64)
            .collect::<Vec<i64>>();

        let prod = result.par_iter().map(|c| cks.decrypt(&c) as u8).collect();

        (errors, prod)
    }

    // Based on: Chillotti et al., "TFHE: Fast Fully Homomorphic Encryption Over the Torus"
    // See: https://eprint.iacr.org/2018/421
    // Discliamer: Might be incorrect...
    fn compute_max_pbs_error(params: &ClassicPBSParameters) -> u64 {
        let n = params.lwe_dimension.0 as f64;
        let k = params.glwe_dimension.0 as f64;
        let l = params.pbs_level.0 as f64;
        let t = params.ks_level.0 as f64;
        let N = params.polynomial_size.0 as f64;
        let beta = (1 << params.pbs_base_log.0) as f64;
        let A_bk = 4.0 * params.glwe_noise_distribution.gaussian_std_dev().0;
        let A_ksk = 4.0 * params.lwe_noise_distribution.gaussian_std_dev().0;
        let epsilon = 2f64.powf(params.log2_p_fail);

        let t1 = n * (k + 1.0) * l * N * beta * A_bk;
        let t2 = n * (1.0 + k * N) * epsilon;
        let t3 = n * 2f64.powf(-t - 1.0);
        let t4 = t * n * A_ksk;
        let max_error = t1 + t2 + t3 + t4;

        (max_error * (1u128 << 63) as f64).round() as u64
    }

    // cargo test tests::test_simulation --release -- --nocapture
    #[test]
    fn test_simulation() {
        let mut params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;

        let message = 1u64;

        let (cks, sks) = simulate_gen_keys(params);

        let ct = simulate_encryption(params, &cks, message);

        assert_eq!(cks.decrypt(&ct) as u64, message);
    }

    fn simulate_compression_decompression(
        comp_key: &CompressionKey,
        decomp_key: &DecompressionKey,
        list: &[Ciphertext],
    ) -> Vec<Ciphertext> {
        let compressed_list = comp_key.compress_ciphertexts_into_list(&list);
        let decompressed_list = decompress_shortint(&decomp_key, &compressed_list);

        decompressed_list
    }

    fn simulate_encryption(
        params: ClassicPBSParameters,
        cks: &ClientKey,
        message: u64,
    ) -> Ciphertext {
        let lwe_dim = find_lwe_dim(&params);

        let mut seeder = new_seeder();
        let seeder = seeder.as_mut();

        let mask_seed = seeder.seed();
        let noise_seed = seeder.seed();

        let delta = (1_u64 << 63) / (params.message_modulus.0 * params.carry_modulus.0) as u64;
        let encoded_message = (message % params.message_modulus.0 as u64) * delta;

        let noise = simulate_generate_noise(cks, lwe_dim.0, noise_seed);
        let lwe_secret_key = cks.encryption_key_and_noise().0;
        let secret_key: &[u64] = lwe_secret_key.into_container();

        let masks = simulate_generate_masks(mask_seed, lwe_dim.0);

        let mask_key_dot_product =
            slice_wrapping_dot_product(&masks, secret_key).wrapping_add(noise);

        let body = mask_key_dot_product.wrapping_add(encoded_message);

        let ciphertext = create_shortint_ciphertext_from_raw_parts(&params, &masks, body);

        ciphertext
    }

    fn compute_error(cks: &ClientKey, expected: u64, ciphertext: &Ciphertext) -> u64 {
        let lwe_secret_key = cks.encryption_key_and_noise().0;

        let delta = (1_u64 << 63)
            / (cks.parameters.message_modulus().0 * cks.parameters.carry_modulus().0) as u64;
        let message = expected % cks.parameters.message_modulus().0 as u64;
        let shifted_message = message * delta;
        let encoded = Plaintext(shifted_message);

        let (mask, body) = ciphertext.ct.get_mask_and_body();

        // sum(mask_i * sk_i)
        let mask_key_dot_product =
            slice_wrapping_dot_product(mask.as_ref(), lwe_secret_key.as_ref());

        // delta * m + e
        let plaintext = Plaintext((*body.data).wrapping_sub(mask_key_dot_product)).0;

        // e
        let error = plaintext.wrapping_sub(encoded.0);

        error
    }

    fn simulate_generate_masks(seed: Seed, lwe_dim: usize) -> Vec<u64> {
        let mut generator: RandomGenerator<ActivatedRandomGenerator> = RandomGenerator::new(seed);

        let mut masks: Vec<u64> = vec![0u64; lwe_dim];
        generator.fill_slice_with_random_uniform(&mut masks);

        // let max = (1u64 << 63) - 1;
        // let mut masks: Vec<u64> = vec![max; lwe_dim];

        masks
    }

    fn simulate_generate_noise(cks: &ClientKey, lwe_dim: usize, seed: Seed) -> u64 {
        let mut generator: RandomGenerator<ActivatedRandomGenerator> = RandomGenerator::new(seed);

        let distribution = cks.encryption_key_and_noise().1;
        let noise: u64 = generator.random_from_distribution(distribution);

        // let std = distribution.gaussian_std_dev().0;
        // let encoded_std = (std * (1u128 << 63) as f64).round() as u64;
        // let noise = 4 * encoded_std;

        // let delta = (1_u64 << 63)
        //     / (cks.parameters.message_modulus().0 * cks.parameters.carry_modulus().0) as u64;
        // let noise = delta / 2 - 1;

        noise
    }

    fn simulate_gen_keys<P>(params: P) -> (ClientKey, ServerKey)
    where
        P: TryInto<ShortintParameterSet>,
        <P as TryInto<ShortintParameterSet>>::Error: std::fmt::Debug,
    {
        let params: ShortintParameterSet = params.try_into().unwrap();

        let mut seeder = new_seeder();
        let seeder = seeder.as_mut();

        let secret_seed = seeder.seed();

        let mut generator: RandomGenerator<ActivatedRandomGenerator> =
            RandomGenerator::new(secret_seed);

        // short key
        let mut lwe_secret_key = LweSecretKeyOwned::new_empty_key(0u64, params.lwe_dimension());
        generator.fill_slice_with_random_uniform_binary(lwe_secret_key.as_mut());

        // let mut lwe_secret_key = LweSecretKeyOwned::new_empty_key(1u64, params.lwe_dimension()); // worst

        // big key
        let mut glwe_secret_key = GlweSecretKeyOwned::new_empty_key(
            0u64,
            params.glwe_dimension(),
            params.polynomial_size(),
        );
        generator.fill_slice_with_random_uniform_binary(glwe_secret_key.as_mut());

        // let mut glwe_secret_key = GlweSecretKeyOwned::new_empty_key(
        //     1u64,
        //     params.glwe_dimension(),
        //     params.polynomial_size(),
        // ); // worst

        let cks = ClientKey::from_raw_parts(glwe_secret_key, lwe_secret_key, params);
        let sks = ServerKey::new(&cks);

        (cks, sks)
    }

    // cargo test tests::test_arithmetic --release -- --nocapture
    #[test]
    fn test_arithmetic() {
        let dim = 1;

        let p = 4 as u64;
        let c = 4 as u64;
        let delta = (1 << 63) / (p * c) as u64;

        // println!("delta: {}", delta);

        let message = 1 as u64;

        println!("message: {}", message);

        let encoded = message * delta as u64;

        // println!("encoded: {}", encoded);
        // println!("encoded in binary: {:b}", encoded);

        let mut seeder = new_seeder();
        let seeder = seeder.as_mut();

        let seed = seeder.seed();
        let mut generator: RandomGenerator<ActivatedRandomGenerator> = RandomGenerator::new(seed);

        let mut masks: Vec<u64> = vec![0u64; dim];
        generator.fill_slice_with_random_uniform(&mut masks);

        // println!("masks: {:?}", masks[0]);
        // println!("masks in binary: {:b}", masks[0]);

        let distribution =
            DynamicDistribution::new_gaussian_from_std_dev(StandardDev(3.5539902359442825e-06));
        let noise: u64 = generator.random_from_distribution(distribution);

        let std: f64 = 3.5539902359442825e-06;
        let encoded_std = (3f64 * std * (1u128 << 63) as f64).round() as u64;

        // println!("std_u64: {}", encoded_std);
        // println!("std_u64 in binary: {:b}", encoded_std);

        // println!("delta / 2: {}", delta / 2);

        let noise = delta / 2; // max possible noise

        println!("noise: {}", noise);

        // println!("noise: {:?}", noise);
        // println!("noise (signed): {:?}", noise as i64);
        // println!("noise in binary: {:b}", noise);

        // let absolute_error = (noise as i64).abs() as u64;
        // assert!(absolute_error < delta / 2);
        // assert!(encoded_std < delta / 2);
        // assert!(absolute_error < encoded_std);

        let sk: Vec<u64> = vec![1];

        // println!("sk: {:?}", sk);
        // println!("sk in binary: {:b}", sk[0]);

        let mask_key_dot_product = slice_wrapping_dot_product(&masks, &sk);

        // println!("mask_key_dot_product: {:?}", mask_key_dot_product);
        // println!("mask_key_dot_product in binary: {:b}", mask_key_dot_product);

        let body_minus_noise: u64 = mask_key_dot_product.wrapping_add(encoded);

        // println!("body_minus_noise: {}", body_minus_noise);
        // println!("body_minus_noise in binary: {:b}", body_minus_noise);

        let body = body_minus_noise.wrapping_add(noise);

        // println!("body: {}", body);
        // println!("body in binary: {:b}", body);

        let plaintext = body.wrapping_sub(mask_key_dot_product);

        // println!("plaintext: {}", plaintext);
        // println!("plaintext in binary: {:b}", plaintext);

        let rounding_bit = delta >> 1;

        // println!("rounding_bit: {}", rounding_bit);

        let rounding = (plaintext & rounding_bit) << 1;

        // println!("rounding: {}", rounding);
        // println!("rounding in binary: {:b}", rounding);

        let decoded_message = plaintext.wrapping_add(rounding) / delta;

        println!("decoded_message: {}", decoded_message);

        assert!(message == decoded_message);
    }

    // cargo test tests::test_mul_cost --release -- --exact --nocapture
    #[test]
    fn test_mul_cost() {
        let params = tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2_KS_PBS;
        let (cks, sks) = gen_keys(params);

        let acc = cks.encrypt(1);
        let query = cks.encrypt(1);

        const NUM_RUNS: usize = 10;
        let mut times_sec = Vec::with_capacity(NUM_RUNS);

        for _ in 0..NUM_RUNS {
            let start = Instant::now();
            let _ = sks.mul(&acc, &query);
            times_sec.push(start.elapsed().as_secs_f64());
        }

        let avg_sec = average(&times_sec);

        println!(
            "Avg sks.mul cost ({} runs): {:.6} s ({:.3} ms)",
            NUM_RUNS,
            avg_sec,
            avg_sec * 1000.0
        );
    }
}

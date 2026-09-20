use crate::byte_fuse_filter::{Arity3ByteFusePosition, ByteFusePosition};
use crate::common::*;
use crate::utils::cwutils::*;
use bytebuffer::ByteBuffer;
use rayon::prelude::*;
use tfhe::core_crypto::commons::math::random::{Distribution, RandomGenerable, RandomGenerator};
use tfhe::core_crypto::prelude::slice_algorithms::slice_wrapping_dot_product;
use tfhe::core_crypto::prelude::ActivatedRandomGenerator;
use tfhe::shortint::ciphertext::CompressedCiphertext as ShortintCompressedCiphertext;
use tfhe::shortint::{ClassicPBSParameters, ClientKey as ShortintClientKey};
use tfhe::{
    prelude::*, ClientKey, CompressedCiphertextList, CompressedCiphertextListBuilder, FheUint8,
    Seed,
};

/// Encodes the query to a constant-weight codeword.
pub fn encode_to_cw_codeword(query: u64, cw_params: &ConstantWeightParameters) -> Vec<u8> {
    let encoded_query: Vec<u8> = get_perfect_constant_weight_codeword(
        query,
        cw_params.codeword_bitlen,
        cw_params.hamming_weight,
    );

    encoded_query
}

/// Encrypt and compress the encoded query (constant-weight codeword).
pub fn encrypt_and_compress_query(query: &[u8], ck: &ClientKey) -> CompressedCiphertextList {
    let encrypted_query: Vec<FheUint8> = query.iter().map(|&x| FheUint8::encrypt(x, ck)).collect();

    let compressed_query: CompressedCiphertextList = CompressedCiphertextListBuilder::new()
        .extend(encrypted_query.into_iter())
        .build()
        .unwrap();

    compressed_query
}

pub fn encrypt_and_compress_query_shortint(
    query: &[u8],
    cks: &ShortintClientKey,
) -> Vec<ShortintCompressedCiphertext> {
    let encrypted_query: Vec<ShortintCompressedCiphertext> = query
        .iter()
        .map(|&x| cks.encrypt_compressed(x as u64))
        .collect();

    encrypted_query
}

pub fn generate_raw_noise<D>(seed: Seed, dist: D, size: usize) -> Vec<u64>
where
    D: Distribution,
    u64: RandomGenerable<D>,
{
    let mut generator: RandomGenerator<ActivatedRandomGenerator> = RandomGenerator::new(seed);

    let mut noise: Vec<u64> = vec![0; size];

    generator.fill_slice_with_random_from_distribution(&mut noise, dist);

    noise
}

pub fn generate_raw_bodies(
    masks: &[Vec<u64>],
    secret: &[u64],
    noise: &[u64],
    size: usize,
) -> Vec<u64> {
    (0..size)
        .map(|i| slice_wrapping_dot_product(&masks[i], secret).wrapping_add(noise[i]))
        .collect()
}

pub fn generate_selection_vector(
    query: u64,
    bodies: &[u64],
    enc_params: &ClassicPBSParameters,
    bff_params: &ByteFuseFilterParameters,
    db_params: &DatabaseParameters,
) -> Vec<u64> {
    let fuse_position = Some(Arity3ByteFusePosition::new(
        db_params.num_keywords,
        bff_params.digest_byte_len + db_params.num_partitions(),
        bff_params.filter_seed.clone(),
    ));

    let key_buffer: ByteBuffer = ByteBuffer::from_vec(query.to_be_bytes().to_vec());
    let indices = fuse_position
        .unwrap()
        .abs_arity3_byte_fuse_pos
        .positions(key_buffer);

    // 1 x filter_len
    let mut iv: Vec<u64> = vec![0; bff_params.filter_len];
    let delta = (1_u64 << 63) / (enc_params.message_modulus.0 * enc_params.carry_modulus.0) as u64;

    for idx in indices {
        iv[idx] = delta;
    }

    // 1 x filter_len
    let mut sv = Vec::new();
    for i in 0..bff_params.filter_len {
        sv.push(bodies[i].wrapping_add(iv[i]));
    }

    sv
}

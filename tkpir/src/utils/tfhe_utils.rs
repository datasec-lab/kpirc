use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use tfhe::boolean::prelude::LweDimension;
use tfhe::core_crypto::commons::math::random::RandomGenerator;
use tfhe::core_crypto::prelude::ActivatedRandomGenerator;
use tfhe::core_crypto::prelude::DummyCreateFrom;
use tfhe::core_crypto::prelude::LweCiphertext;
use tfhe::integer::compression_keys::CompressionPrivateKeys;
use tfhe::integer::IntegerCiphertext;
use tfhe::integer::RadixCiphertext;
use tfhe::set_server_key;
use tfhe::shortint::ciphertext::Ciphertext as ShortintCiphertext;
use tfhe::shortint::ciphertext::CompressedCiphertext as ShortintCompressedCiphertext;
use tfhe::shortint::ciphertext::CompressedCiphertextList as ShortintCompressedCiphertextList;
use tfhe::shortint::list_compression::CompressionKey as ShortintCompressionKey;
use tfhe::shortint::list_compression::DecompressionKey as ShortintDecompressionKey;
use tfhe::shortint::parameters::Degree;
use tfhe::shortint::parameters::NoiseLevel;
use tfhe::shortint::parameters::COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64;
use tfhe::shortint::ClassicPBSParameters;
use tfhe::shortint::ClientKey as ShortintClientKey;
use tfhe::shortint::EncryptionKeyChoice;
use tfhe::shortint::PBSOrder;
use tfhe::shortint::ServerKey as ShortintServerKey;
use tfhe::ClientKey;
use tfhe::FheBool;
use tfhe::FheUint2;
use tfhe::FheUint128;
use tfhe::FheUint256;
use tfhe::FheUint512;
use tfhe::FheUint1024;
use tfhe::FheUint64;
use tfhe::Seed;
use tfhe::ServerKey;
use tfhe::Tag;
use tfhe::{prelude::*, CompressedCiphertextList, FheUint8};

pub fn decompress(compressed_cipher: &CompressedCiphertextList) -> Vec<FheUint8> {
    let mut compressed_cipher_list: Vec<FheUint8> = Vec::new();

    for idx in 0..(compressed_cipher.len()) {
        compressed_cipher_list.push(compressed_cipher.get(idx).unwrap().unwrap());
    }

    compressed_cipher_list
}

pub fn compress_shortint(
    comp_key: &ShortintCompressionKey,
    ciphertexts: &[ShortintCiphertext],
) -> ShortintCompressedCiphertextList {
    comp_key.compress_ciphertexts_into_list(&ciphertexts)
}

pub fn decompress_shortint(
    decomp_key: &ShortintDecompressionKey,
    compressed_ciphertexts: &ShortintCompressedCiphertextList,
) -> Vec<ShortintCiphertext> {
    (0..compressed_ciphertexts.count.0)
        .into_par_iter()
        .map(|idx| decomp_key.unpack(compressed_ciphertexts, idx).unwrap())
        .collect()
}

pub fn decompress_shortint_from_slice(
    compressed_ciphertexts: &[ShortintCompressedCiphertext],
) -> Vec<ShortintCiphertext> {
    (0..compressed_ciphertexts.len())
        .into_par_iter()
        .map(|idx| compressed_ciphertexts.get(idx).unwrap().decompress())
        .collect()
}

pub fn generate_raw_masks(seed: Seed, lwe_dim: usize, size: usize) -> Vec<Vec<u64>> {
    let mut generator: RandomGenerator<ActivatedRandomGenerator> = RandomGenerator::new(seed);

    let mut masks = Vec::new();

    for _ in 0..size {
        let mut mask: Vec<u64> = vec![0; lwe_dim];
        generator.fill_slice_with_random_uniform(&mut mask);
        masks.push(mask);
    }

    masks
}

pub fn create_shortint_ciphertexts_from_raw_parts(
    params: &ClassicPBSParameters,
    masks: &[Vec<u64>],
    bodies: &[u64],
    size: usize,
) -> Vec<ShortintCiphertext> {
    let pbs_order: PBSOrder = params.encryption_key_choice.into();

    let message_modulus = params.message_modulus;
    let carry_modulus = params.carry_modulus;
    let lwe_dim = find_lwe_dim(params);

    let mut ciphertexts = Vec::new();
    for i in 0..size {
        let mut ct = LweCiphertext::new(0u64, lwe_dim.to_lwe_size(), params.ciphertext_modulus);
        let (mut mask, body) = ct.get_mut_mask_and_body();
        for (j, a) in mask.as_mut().iter_mut().enumerate() {
            *a = masks[i][j];
        }
        *body.data = bodies[i];

        let ciphertext = ShortintCiphertext::new(
            ct,
            Degree::new(message_modulus.0 - 1),
            NoiseLevel::NOMINAL,
            message_modulus,
            carry_modulus,
            pbs_order,
        );
        ciphertexts.push(ciphertext);
    }

    ciphertexts
}

pub fn create_shortint_ciphertext_from_raw_parts(
    params: &ClassicPBSParameters,
    masks: &[u64],
    b: u64,
) -> ShortintCiphertext {
    let pbs_order: PBSOrder = params.encryption_key_choice.into();

    let message_modulus = params.message_modulus;
    let carry_modulus = params.carry_modulus;
    let lwe_dim = find_lwe_dim(params);

    let mut ct = LweCiphertext::new(0u64, lwe_dim.to_lwe_size(), params.ciphertext_modulus);
    let (mut mask, body) = ct.get_mut_mask_and_body();
    for (i, a) in mask.as_mut().iter_mut().enumerate() {
        *a = masks[i];
    }
    *body.data = b;

    let ciphertext = ShortintCiphertext::new(
        ct,
        Degree::new(message_modulus.0 - 1),
        NoiseLevel::NOMINAL,
        message_modulus,
        carry_modulus,
        pbs_order,
    );

    ciphertext
}

pub fn find_lwe_dim(params: &ClassicPBSParameters) -> LweDimension {
    let lwe_dim = match params.encryption_key_choice {
        EncryptionKeyChoice::Big => params
            .glwe_dimension
            .to_equivalent_lwe_dimension(params.polynomial_size),
        EncryptionKeyChoice::Small => params.lwe_dimension,
    };

    lwe_dim
}

fn shortint_ciphertexts_to_radix(
    params: &ClassicPBSParameters,
    ciphers: &[ShortintCiphertext],
    bit_width: usize,
) -> RadixCiphertext {
    let message_bits = params.message_modulus.0.ilog2() as usize;
    let num_blocks = bit_width / message_bits;
    assert!(
        bit_width % message_bits == 0,
        "bit_width {} must be divisible by message_bits {}",
        bit_width,
        message_bits
    );
    assert!(
        ciphers.len() <= num_blocks,
        "payload has {} limbs but width {} only holds {} (entry too large for Evaluate)",
        ciphers.len(),
        bit_width,
        num_blocks
    );

    let mut ciphertexts: Vec<ShortintCiphertext> = Vec::with_capacity(num_blocks);
    ciphertexts.extend_from_slice(ciphers);

    let pbs_order: PBSOrder = params.encryption_key_choice.into();
    let lwe_dim = find_lwe_dim(params);

    for _ in ciphers.len()..num_blocks {
        let dummy = LweCiphertext::new(0u64, lwe_dim.to_lwe_size(), params.ciphertext_modulus);
        let dummy: ShortintCiphertext = ShortintCiphertext::new(
            dummy,
            Degree::new(0),
            NoiseLevel::ZERO,
            params.message_modulus,
            params.carry_modulus,
            pbs_order,
        );
        ciphertexts.push(dummy);
    }

    RadixCiphertext::from_blocks(ciphertexts)
}

pub fn shortint_ciphertexts_to_fheuint64(
    params: &ClassicPBSParameters,
    ciphers: &[ShortintCiphertext],
) -> FheUint64 {
    FheUint64::try_from(shortint_ciphertexts_to_radix(params, ciphers, 64)).unwrap()
}

pub fn shortint_ciphertexts_to_fheuint128(
    params: &ClassicPBSParameters,
    ciphers: &[ShortintCiphertext],
) -> FheUint128 {
    FheUint128::try_from(shortint_ciphertexts_to_radix(params, ciphers, 128)).unwrap()
}

pub fn shortint_ciphertexts_to_fheuint256(
    params: &ClassicPBSParameters,
    ciphers: &[ShortintCiphertext],
) -> FheUint256 {
    FheUint256::try_from(shortint_ciphertexts_to_radix(params, ciphers, 256)).unwrap()
}

pub fn shortint_ciphertexts_to_fheuint512(
    params: &ClassicPBSParameters,
    ciphers: &[ShortintCiphertext],
) -> FheUint512 {
    FheUint512::try_from(shortint_ciphertexts_to_radix(params, ciphers, 512)).unwrap()
}

pub fn shortint_ciphertexts_to_fheuint1024(
    params: &ClassicPBSParameters,
    ciphers: &[ShortintCiphertext],
) -> FheUint1024 {
    FheUint1024::try_from(shortint_ciphertexts_to_radix(params, ciphers, 1024)).unwrap()
}

pub fn shortint_key_to_client_key(
    params: &ClassicPBSParameters,
    cks: ShortintClientKey,
) -> (ClientKey, ServerKey) {
    // integer compression key
    let comp_key = CompressionPrivateKeys::from_raw_parts(
        cks.new_compression_private_key(COMP_PARAM_MESSAGE_2_CARRY_2_KS_PBS_TUNIFORM_2M64),
    );

    // integer client key
    let ck = tfhe::integer::ClientKey::from_raw_parts(cks);

    // client key
    let ck = ClientKey::from_raw_parts(ck, None, Some(comp_key), Tag::default());

    // set server key
    let sk = ServerKey::new(&ck);
    set_server_key(sk.clone());

    (ck, sk)
}

pub fn encrypt_trivial_container(
    sks: &ShortintServerKey,
    container: &[u64],
) -> Vec<ShortintCiphertext> {
    container.iter().map(|&x| sks.create_trivial(x)).collect()
}

pub fn decrypt_trivial_ciphertexts(ciphertexts: &[ShortintCiphertext]) -> Vec<u64> {
    ciphertexts
        .iter()
        .map(|x| x.decrypt_trivial().unwrap())
        .collect()
}

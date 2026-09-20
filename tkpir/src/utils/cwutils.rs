pub fn choose(n: u64, k: u64) -> u64 {
    if k > n {
        return 0;
    }

    let mut r: u64 = 1;
    let mut n_tmp = n;

    for d in 1..(k + 1) {
        r *= n_tmp;
        r /= d;
        n_tmp -= 1;
    }

    r
}

/// Generates a perfect constant-weight codeword of the specified length and Hamming weight.
///
/// # Panics
/// Panics if `choose(codeword_bitlen, hamming_weight) < x`, which indicates that the
/// provided `x` is out of range for the given `codeword_length` and `hamming_weight`.
pub fn get_perfect_constant_weight_codeword(
    x: u64,
    codeword_bitlen: u64,
    hamming_weight: u64,
) -> Vec<u8> {
    let mut codeword = vec![0u8; codeword_bitlen as usize];
    let codeword_size = choose(codeword_bitlen, hamming_weight);

    assert!(
        codeword_size >= x,
        "Assertion failed: code_size ({}) is less than the number ({}). Choose a different code length?",
        codeword_size,
        x
    );

    let mut r = x;
    let mut h = hamming_weight;

    for l in (0..codeword_bitlen).rev() {
        let c = choose(l, h);
        if r >= c {
            codeword[l as usize] = 1;
            r -= c;
            h -= 1;
            if h == 0 {
                break;
            }
        }
    }

    codeword
}

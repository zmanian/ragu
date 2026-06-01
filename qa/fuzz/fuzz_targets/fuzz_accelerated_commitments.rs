//! Accelerated sparse-commitment fuzz target.
//!
//! Generates sparse-ish coefficient vectors, commits with the default CPU
//! path, then commits through the explicit acceleration API while forcing an
//! unavailable CUDA backend. The commitment must match, and the acceleration
//! counters must show CPU fallback instead of a facade result.

#![no_main]

use arbitrary::Arbitrary;
use ff::Field;
use libfuzzer_sys::fuzz_target;
use ragu_arithmetic::{
    AccelBackend, AccelMsmConfig, Cycle, accel_msm_stats, reset_accel_msm_stats,
};
use ragu_circuits::polynomials::{Rank, TestRank, sparse::Polynomial};
use ragu_pasta::{Fp, Pasta};

const MAX_COEFFS: usize = 128;

#[derive(Arbitrary, Debug)]
struct Input {
    coeffs: Vec<CoeffSeed>,
    min_msm_size: u16,
}

#[derive(Arbitrary, Debug)]
enum CoeffSeed {
    Zero,
    Small(u8),
    Large(u64),
    Negated(u64),
}

fuzz_target!(|input: Input| {
    if std::env::var("DEBUG_INPUT").is_ok() {
        eprintln!("{input:#?}");
        return;
    }

    let coeffs = input
        .coeffs
        .into_iter()
        .take(MAX_COEFFS.min(TestRank::num_coeffs()))
        .map(coeff_from_seed)
        .collect::<Vec<_>>();

    let poly = Polynomial::<Fp, TestRank>::from_coeffs(coeffs);
    let pasta = Pasta::baked();
    let generators = Pasta::host_generators(pasta);

    let expected = poly.commit_to_affine(generators);

    reset_accel_msm_stats();
    let actual = poly.commit_to_affine_with_accel_config(
        generators,
        AccelMsmConfig {
            backend: AccelBackend::Cuda,
            min_msm_size: input.min_msm_size as usize,
        },
    );

    assert_eq!(actual, expected);

    let stats = accel_msm_stats();
    assert!(stats.candidates <= 1);
    assert_eq!(stats.facade_results, 0);
    assert_eq!(stats.fallbacks, stats.candidates);
});

fn coeff_from_seed(seed: CoeffSeed) -> Fp {
    match seed {
        CoeffSeed::Zero => Fp::ZERO,
        CoeffSeed::Small(value) => Fp::from(value as u64),
        CoeffSeed::Large(value) => Fp::from(value),
        CoeffSeed::Negated(value) => -Fp::from(value),
    }
}

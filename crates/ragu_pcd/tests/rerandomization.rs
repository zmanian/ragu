use ff::Field;
use ragu_arithmetic::Cycle;
use ragu_circuits::polynomials::ProductionRank;
use ragu_core::{
    Result,
    drivers::{Driver, DriverValue},
    gadgets::{Bound, Kind},
    maybe::Maybe,
};
use ragu_pasta::{Fp, Pasta};
use ragu_pcd::{
    ApplicationBuilder,
    header::{Header, Suffix},
    step::{Encoded, Index, Step},
};
use ragu_primitives::{
    Element,
    allocator::{Allocator, Standard},
};
use rand::{SeedableRng, rngs::StdRng};

#[cfg(feature = "accel-msm")]
use ragu_arithmetic::{AccelBackend, accel_msm_stats, reset_accel_msm_stats};
#[cfg(feature = "accel-msm")]
use ragu_pcd::ProverAccelConfig;

// Header A (suffix 0) - unit data
struct HeaderA;

impl<F: Field> Header<F> for HeaderA {
    const SUFFIX: Suffix = Suffix::new(0);
    type Data = ();
    type Output = ();
    fn encode<'dr, D: Driver<'dr, F = F>, A: Allocator<'dr, D>>(
        _: &mut D,
        _: &mut A,
        _: DriverValue<D, Self::Data>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        Ok(())
    }
}

// Header with real data (suffix 2) - carries a field element
struct HeaderWithData;

impl Header<Fp> for HeaderWithData {
    const SUFFIX: Suffix = Suffix::new(2);
    type Data = Fp;
    type Output = Kind![Fp; Element<'_, _>];
    fn encode<'dr, D: Driver<'dr, F = Fp>, A: Allocator<'dr, D>>(
        dr: &mut D,
        allocator: &mut A,
        witness: DriverValue<D, Self::Data>,
    ) -> Result<Bound<'dr, D, Self::Output>> {
        Element::alloc(dr, allocator, witness)
    }
}

// Step that produces HeaderWithData from trivial inputs
struct StepWithData;
impl Step<Pasta> for StepWithData {
    const INDEX: Index = Index::new(0);
    type Witness<'source> = Fp;
    type Aux<'source> = ();
    type Left = ();
    type Right = ();
    type Output = HeaderWithData;
    fn witness<'dr, 'source: 'dr, D: Driver<'dr, F = Fp>, const HEADER_SIZE: usize>(
        &self,
        dr: &mut D,
        witness: DriverValue<D, Self::Witness<'source>>,
        left: DriverValue<D, ()>,
        right: DriverValue<D, ()>,
    ) -> Result<(
        (
            Encoded<'dr, D, Self::Left, HEADER_SIZE>,
            Encoded<'dr, D, Self::Right, HEADER_SIZE>,
            Encoded<'dr, D, Self::Output, HEADER_SIZE>,
        ),
        DriverValue<D, <Self::Output as Header<Fp>>::Data>,
        DriverValue<D, Self::Aux<'source>>,
    )> {
        let allocator = &mut Standard::new();
        let left = Encoded::new(dr, allocator, left)?;
        let right = Encoded::new(dr, allocator, right)?;
        let output = Encoded::new(dr, allocator, witness.clone())?;
        Ok(((left, right, output), witness, D::unit()))
    }
}

// Step0: () , ()  -> HeaderA
struct Step0;
impl<C: Cycle> Step<C> for Step0 {
    const INDEX: Index = Index::new(0);
    type Witness<'source> = ();
    type Aux<'source> = ();
    type Left = ();
    type Right = ();
    type Output = HeaderA;
    fn witness<'dr, 'source: 'dr, D: Driver<'dr, F = C::CircuitField>, const HEADER_SIZE: usize>(
        &self,
        dr: &mut D,
        _: DriverValue<D, Self::Witness<'source>>,
        left: DriverValue<D, ()>,
        right: DriverValue<D, ()>,
    ) -> Result<(
        (
            Encoded<'dr, D, Self::Left, HEADER_SIZE>,
            Encoded<'dr, D, Self::Right, HEADER_SIZE>,
            Encoded<'dr, D, Self::Output, HEADER_SIZE>,
        ),
        DriverValue<D, <Self::Output as Header<C::CircuitField>>::Data>,
        DriverValue<D, Self::Aux<'source>>,
    )> {
        let allocator = &mut Standard::new();
        let left = Encoded::new(dr, allocator, left)?;
        let right = Encoded::new(dr, allocator, right)?;
        let output = Encoded::from_gadget(());
        Ok(((left, right, output), D::unit(), D::unit()))
    }
}

struct Step1;
impl<C: Cycle> Step<C> for Step1 {
    const INDEX: Index = Index::new(1);
    type Witness<'source> = ();
    type Aux<'source> = ();
    type Left = HeaderA;
    type Right = HeaderA;
    type Output = HeaderA;
    fn witness<'dr, 'source: 'dr, D: Driver<'dr, F = C::CircuitField>, const HEADER_SIZE: usize>(
        &self,
        dr: &mut D,
        _: DriverValue<D, Self::Witness<'source>>,
        left: DriverValue<D, ()>,
        right: DriverValue<D, ()>,
    ) -> Result<(
        (
            Encoded<'dr, D, Self::Left, HEADER_SIZE>,
            Encoded<'dr, D, Self::Right, HEADER_SIZE>,
            Encoded<'dr, D, Self::Output, HEADER_SIZE>,
        ),
        DriverValue<D, <Self::Output as Header<C::CircuitField>>::Data>,
        DriverValue<D, Self::Aux<'source>>,
    )> {
        let allocator = &mut Standard::new();
        let left = Encoded::new(dr, allocator, left)?;
        let right = Encoded::new(dr, allocator, right)?;
        let output = Encoded::from_gadget(());
        Ok(((left, right, output), D::unit(), D::unit()))
    }
}

#[test]
fn rerandomization_flow() {
    let pasta = Pasta::baked();
    let app = ApplicationBuilder::<Pasta, ProductionRank, 4>::new()
        .register(Step0)
        .unwrap()
        .register(Step1)
        .unwrap()
        .finalize(pasta)
        .unwrap();

    let mut rng = StdRng::seed_from_u64(1234);

    let (seeded, _) = app.seed(&mut rng, Step0, ()).unwrap();
    assert!(app.verify(&seeded, &mut rng).unwrap());

    // Rerandomize
    let seeded = app.rerandomize(seeded, &mut rng).unwrap();
    assert!(app.verify(&seeded, &mut rng).unwrap());

    let (fused, _) = app
        .fuse(&mut rng, Step1, (), seeded.clone(), seeded)
        .unwrap();
    assert!(app.verify(&fused, &mut rng).unwrap());

    let fused = app.rerandomize(fused, &mut rng).unwrap();
    assert!(app.verify(&fused, &mut rng).unwrap());
}

#[test]
fn multiple_rerandomizations_all_verify() {
    let pasta = Pasta::baked();
    let app = ApplicationBuilder::<Pasta, ProductionRank, 4>::new()
        .register(Step0)
        .unwrap()
        .finalize(pasta)
        .unwrap();

    let mut rng = StdRng::seed_from_u64(9999);

    let (original, _) = app.seed(&mut rng, Step0, ()).unwrap();
    assert!(app.verify(&original, &mut rng).unwrap());

    // Rerandomize multiple times - each should verify
    let rerand1 = app.rerandomize(original.clone(), &mut rng).unwrap();
    assert!(app.verify(&rerand1, &mut rng).unwrap());

    let rerand2 = app.rerandomize(original.clone(), &mut rng).unwrap();
    assert!(app.verify(&rerand2, &mut rng).unwrap());

    // Rerandomize an already rerandomized proof
    let rerand3 = app.rerandomize(rerand1, &mut rng).unwrap();
    assert!(app.verify(&rerand3, &mut rng).unwrap());
}

#[test]
fn rerandomization_preserves_header_data() {
    let pasta = Pasta::baked();
    let app = ApplicationBuilder::<Pasta, ProductionRank, 4>::new()
        .register(StepWithData)
        .unwrap()
        .finalize(pasta)
        .unwrap();

    let mut rng = StdRng::seed_from_u64(4321);

    // Use a non-trivial data value
    let test_data = Fp::from(123456789u64);

    let (original, _) = app.seed(&mut rng, StepWithData, test_data).unwrap();
    assert!(app.verify(&original, &mut rng).unwrap());

    let rerandomized = app.rerandomize(original.clone(), &mut rng).unwrap();
    assert!(app.verify(&rerandomized, &mut rng).unwrap());

    // Header data should be preserved (non-unit comparison)
    assert_eq!(
        *original.data(),
        *rerandomized.data(),
        "rerandomization should preserve header data"
    );
    assert_eq!(
        *rerandomized.data(),
        Fp::from(123456789u64),
        "header data should match original value"
    );
}

#[test]
fn rerandomized_fused_proof_verifies() {
    let pasta = Pasta::baked();
    let app = ApplicationBuilder::<Pasta, ProductionRank, 4>::new()
        .register(Step0)
        .unwrap()
        .register(Step1)
        .unwrap()
        .finalize(pasta)
        .unwrap();

    let mut rng = StdRng::seed_from_u64(7777);

    // Create two seeded proofs
    let (left, _) = app.seed(&mut rng, Step0, ()).unwrap();
    let (right, _) = app.seed(&mut rng, Step0, ()).unwrap();

    // Fuse them
    let (fused, _) = app.fuse(&mut rng, Step1, (), left, right).unwrap();
    assert!(app.verify(&fused, &mut rng).unwrap());

    // Rerandomize the fused proof
    let rerandomized = app.rerandomize(fused, &mut rng).unwrap();
    assert!(
        app.verify(&rerandomized, &mut rng).unwrap(),
        "rerandomized fused proof should verify"
    );
}

#[cfg(feature = "accel-msm")]
#[test]
fn seed_with_accel_config_falls_back_and_verifies() {
    let pasta = Pasta::baked();
    let app = ApplicationBuilder::<Pasta, ProductionRank, 4>::new()
        .register(Step0)
        .unwrap()
        .finalize(pasta)
        .unwrap();

    let mut rng = StdRng::seed_from_u64(4242);
    reset_accel_msm_stats();

    let (seeded, _) = app
        .seed_with_accel_config(&mut rng, Step0, (), forced_unavailable_accel_config())
        .unwrap();

    assert_forced_backend_fell_back();

    reset_accel_msm_stats();
    assert!(app.verify(&seeded, &mut rng).unwrap());
}

#[cfg(feature = "accel-msm")]
#[test]
fn seed_with_cuda_accel_config_dispatches_or_falls_back_and_verifies() {
    let pasta = Pasta::baked();
    let app = ApplicationBuilder::<Pasta, ProductionRank, 4>::new()
        .register(Step0)
        .unwrap()
        .finalize(pasta)
        .unwrap();

    let mut rng = StdRng::seed_from_u64(4343);
    reset_accel_msm_stats();

    let (seeded, _) = app
        .seed_with_accel_config(&mut rng, Step0, (), cuda_accel_config())
        .unwrap();

    assert_requested_backend_dispatched_or_fell_back();

    reset_accel_msm_stats();
    assert!(app.verify(&seeded, &mut rng).unwrap());
}

#[cfg(feature = "accel-msm")]
#[test]
#[ignore = "full PCD proof-path accel fallback regression; run manually when changing prover acceleration"]
fn fuse_with_accel_config_falls_back_and_verifies() {
    let pasta = Pasta::baked();
    let app = ApplicationBuilder::<Pasta, ProductionRank, 4>::new()
        .register(Step0)
        .unwrap()
        .register(Step1)
        .unwrap()
        .finalize(pasta)
        .unwrap();

    let mut rng = StdRng::seed_from_u64(5252);
    let (left, _) = app.seed(&mut rng, Step0, ()).unwrap();
    let (right, _) = app.seed(&mut rng, Step0, ()).unwrap();

    reset_accel_msm_stats();
    let (fused, _) = app
        .fuse_with_accel_config(
            &mut rng,
            Step1,
            (),
            left,
            right,
            forced_unavailable_accel_config(),
        )
        .unwrap();

    assert_forced_backend_fell_back();

    reset_accel_msm_stats();
    assert!(app.verify(&fused, &mut rng).unwrap());
}

#[cfg(feature = "accel-msm")]
#[test]
#[ignore = "full PCD proof-path accel fallback regression; run manually when changing prover acceleration"]
fn rerandomize_with_accel_config_falls_back_and_preserves_data() {
    let pasta = Pasta::baked();
    let app = ApplicationBuilder::<Pasta, ProductionRank, 4>::new()
        .register(StepWithData)
        .unwrap()
        .finalize(pasta)
        .unwrap();

    let mut rng = StdRng::seed_from_u64(6262);
    let test_data = Fp::from(987654321u64);
    let (original, _) = app.seed(&mut rng, StepWithData, test_data).unwrap();

    reset_accel_msm_stats();
    let rerandomized = app
        .rerandomize_with_accel_config(
            original.clone(),
            &mut rng,
            forced_unavailable_accel_config(),
        )
        .unwrap();

    assert_forced_backend_fell_back();

    assert_eq!(original.data(), rerandomized.data());
    assert_eq!(*rerandomized.data(), test_data);

    reset_accel_msm_stats();
    assert!(app.verify(&rerandomized, &mut rng).unwrap());
}

#[cfg(feature = "accel-msm")]
fn forced_unavailable_accel_config() -> ProverAccelConfig {
    ProverAccelConfig {
        backend: AccelBackend::Avx512,
        min_msm_size: 1,
        min_fft_log2: 0,
        allow_gpu_witness_buffers: false,
    }
}

#[cfg(feature = "accel-msm")]
fn cuda_accel_config() -> ProverAccelConfig {
    ProverAccelConfig {
        backend: AccelBackend::Cuda,
        min_msm_size: 1,
        min_fft_log2: 0,
        allow_gpu_witness_buffers: false,
    }
}

#[cfg(feature = "accel-msm")]
fn assert_forced_backend_fell_back() {
    let stats = accel_msm_stats();
    assert!(stats.candidates > 0);
    assert!(stats.fallbacks > 0);
    assert_eq!(stats.facade_results, 0);
    assert_eq!(stats.fallbacks, stats.candidates);
}

#[cfg(feature = "accel-msm")]
fn assert_requested_backend_dispatched_or_fell_back() {
    let stats = accel_msm_stats();
    assert!(stats.candidates > 0);
    assert!(stats.facade_results > 0 || stats.fallbacks > 0);
    assert_eq!(stats.facade_results + stats.fallbacks, stats.candidates);
}

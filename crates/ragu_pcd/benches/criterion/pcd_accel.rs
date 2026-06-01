use criterion::{Criterion, criterion_group, criterion_main};

#[cfg(feature = "accel-msm")]
mod accel {
    use criterion::{BatchSize, Criterion};
    use ragu_arithmetic::{
        AccelBackend, Cycle, accel_msm_stats, format_accel_msm_stats, reset_accel_msm_stats,
    };
    #[cfg(feature = "accel-fft")]
    use ragu_arithmetic::{accel_fft_stats, format_accel_fft_stats, reset_accel_fft_stats};
    use ragu_circuits::polynomials::ProductionRank;
    use ragu_pasta::{Fp, Pasta};
    use ragu_pcd::{Application, ApplicationBuilder, Pcd, ProverAccelConfig};
    use ragu_testing::pcd::nontrivial;
    use rand::{SeedableRng, rngs::StdRng};

    type BenchApp = Application<'static, Pasta, ProductionRank, 4>;
    type BenchParams = &'static <Pasta as Cycle>::CircuitPoseidon;
    type Leaf = Pcd<Pasta, ProductionRank, nontrivial::LeafNode>;
    type Node = Pcd<Pasta, ProductionRank, nontrivial::InternalNode>;

    pub fn bench(c: &mut Criterion) {
        reset_accel_msm_stats();
        #[cfg(feature = "accel-fft")]
        reset_accel_fft_stats();

        let (app, poseidon_params) = setup_app();
        let accel_auto = accel_config(AccelBackend::Auto);
        let accel_forced_fallback = accel_config(AccelBackend::Cuda);

        let mut group = c.benchmark_group("pcd_accel");
        group.sample_size(10);

        group.bench_function("seed/cpu", |b| {
            b.iter_batched(
                || StdRng::seed_from_u64(0x51ed),
                |mut rng| {
                    app.seed(
                        &mut rng,
                        nontrivial::WitnessLeaf { poseidon_params },
                        Fp::from(42_u64),
                    )
                    .unwrap()
                },
                BatchSize::PerIteration,
            );
        });

        group.bench_function("seed/accel-auto", |b| {
            b.iter_batched(
                || StdRng::seed_from_u64(0x51ed),
                |mut rng| {
                    app.seed_with_accel_config(
                        &mut rng,
                        nontrivial::WitnessLeaf { poseidon_params },
                        Fp::from(42_u64),
                        accel_auto,
                    )
                    .unwrap()
                },
                BatchSize::PerIteration,
            );
        });

        group.bench_function("seed/forced-fallback", |b| {
            b.iter_batched(
                || StdRng::seed_from_u64(0x51ed),
                |mut rng| {
                    app.seed_with_accel_config(
                        &mut rng,
                        nontrivial::WitnessLeaf { poseidon_params },
                        Fp::from(42_u64),
                        accel_forced_fallback,
                    )
                    .unwrap()
                },
                BatchSize::PerIteration,
            );
        });

        let (leaf1, leaf2) = seed_leaves(&app, poseidon_params);
        group.bench_function("fuse/cpu", |b| {
            b.iter_batched(
                || (leaf1.clone(), leaf2.clone(), StdRng::seed_from_u64(0xf05e)),
                |(left, right, mut rng)| {
                    app.fuse(
                        &mut rng,
                        nontrivial::Hash2 { poseidon_params },
                        (),
                        left,
                        right,
                    )
                    .unwrap()
                },
                BatchSize::PerIteration,
            );
        });

        group.bench_function("fuse/accel-auto", |b| {
            b.iter_batched(
                || (leaf1.clone(), leaf2.clone(), StdRng::seed_from_u64(0xf05e)),
                |(left, right, mut rng)| {
                    app.fuse_with_accel_config(
                        &mut rng,
                        nontrivial::Hash2 { poseidon_params },
                        (),
                        left,
                        right,
                        accel_auto,
                    )
                    .unwrap()
                },
                BatchSize::PerIteration,
            );
        });

        let node = fuse_node(&app, poseidon_params, &leaf1, &leaf2);
        group.bench_function("rerandomize/cpu", |b| {
            b.iter_batched(
                || (node.clone(), StdRng::seed_from_u64(0xfeed)),
                |(node, mut rng)| app.rerandomize(node, &mut rng).unwrap(),
                BatchSize::PerIteration,
            );
        });

        group.bench_function("rerandomize/accel-auto", |b| {
            b.iter_batched(
                || (node.clone(), StdRng::seed_from_u64(0xfeed)),
                |(node, mut rng)| {
                    app.rerandomize_with_accel_config(node, &mut rng, accel_auto)
                        .unwrap()
                },
                BatchSize::PerIteration,
            );
        });

        group.bench_function("verify/node", |b| {
            b.iter_batched(
                || (node.clone(), StdRng::seed_from_u64(0x0b5e)),
                |(node, mut rng)| app.verify(&node, &mut rng).unwrap(),
                BatchSize::PerIteration,
            );
        });

        group.finish();

        let stats = accel_msm_stats();
        eprintln!("pcd accel-msm stats: {}", format_accel_msm_stats(&stats));

        #[cfg(feature = "accel-fft")]
        {
            let stats = accel_fft_stats();
            eprintln!("pcd accel-fft stats: {}", format_accel_fft_stats(&stats));
        }
    }

    fn setup_app() -> (BenchApp, BenchParams) {
        let pasta = Pasta::baked();
        let poseidon_params = Pasta::circuit_poseidon(pasta);
        let app = ApplicationBuilder::<Pasta, ProductionRank, 4>::new()
            .register(nontrivial::WitnessLeaf { poseidon_params })
            .unwrap()
            .register(nontrivial::Hash2 { poseidon_params })
            .unwrap()
            .finalize(pasta)
            .unwrap();

        (app, poseidon_params)
    }

    fn accel_config(backend: AccelBackend) -> ProverAccelConfig {
        ProverAccelConfig {
            backend,
            min_msm_size: 0,
            min_fft_log2: ProverAccelConfig::DEFAULT_MIN_FFT_LOG2,
            allow_gpu_witness_buffers: false,
        }
    }

    fn seed_leaves(app: &BenchApp, poseidon_params: BenchParams) -> (Leaf, Leaf) {
        let mut rng = StdRng::seed_from_u64(0x1eaf);
        let (leaf1, _) = app
            .seed(
                &mut rng,
                nontrivial::WitnessLeaf { poseidon_params },
                Fp::from(1_u64),
            )
            .unwrap();
        let (leaf2, _) = app
            .seed(
                &mut rng,
                nontrivial::WitnessLeaf { poseidon_params },
                Fp::from(2_u64),
            )
            .unwrap();

        (leaf1, leaf2)
    }

    fn fuse_node(app: &BenchApp, poseidon_params: BenchParams, leaf1: &Leaf, leaf2: &Leaf) -> Node {
        let mut rng = StdRng::seed_from_u64(0xf05e);
        app.fuse(
            &mut rng,
            nontrivial::Hash2 { poseidon_params },
            (),
            leaf1.clone(),
            leaf2.clone(),
        )
        .unwrap()
        .0
    }
}

#[cfg(feature = "accel-msm")]
fn pcd_accel_bench(c: &mut Criterion) {
    accel::bench(c);
}

#[cfg(not(feature = "accel-msm"))]
fn pcd_accel_bench(c: &mut Criterion) {
    c.bench_function("pcd_accel/enable-accel-msm-feature", |b| b.iter(|| ()));
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = pcd_accel_bench
}
criterion_main!(benches);

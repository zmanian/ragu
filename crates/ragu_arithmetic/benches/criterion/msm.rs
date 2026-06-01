use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ff::Field;
use pasta_curves::{EpAffine, Fq, group::CurveAffine};
use ragu_arithmetic::mul;
use rand::{SeedableRng, rngs::StdRng};

#[cfg(feature = "accel-msm")]
use ragu_arithmetic::{accel_msm_stats, format_accel_msm_stats, reset_accel_msm_stats};

fn msm_bench(c: &mut Criterion) {
    #[cfg(feature = "accel-msm")]
    reset_accel_msm_stats();

    let mut group = c.benchmark_group("msm");

    for size in [64, 256, 1024, 4096, 8192] {
        let mut rng = StdRng::seed_from_u64(1234);
        let coeffs: Vec<Fq> = (0..size).map(|_| Fq::random(&mut rng)).collect();
        let bases: Vec<EpAffine> = (0..size)
            .map(|_| (EpAffine::generator() * Fq::random(&mut rng)).into())
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| mul(coeffs.iter(), bases.iter()));
        });
    }

    group.finish();

    #[cfg(feature = "accel-msm")]
    {
        let stats = accel_msm_stats();
        eprintln!("accel-msm stats: {}", format_accel_msm_stats(&stats));
    }
}

criterion_group!(benches, msm_bench);
criterion_main!(benches);

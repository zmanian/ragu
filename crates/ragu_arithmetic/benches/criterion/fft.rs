use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ff::Field;
use pasta_curves::Fp;
use ragu_arithmetic::Domain;
use rand::{SeedableRng, rngs::StdRng};

#[cfg(feature = "accel-fft")]
use ragu_arithmetic::{accel_fft_stats, format_accel_fft_stats, reset_accel_fft_stats};

fn fft_bench(c: &mut Criterion) {
    #[cfg(feature = "accel-fft")]
    reset_accel_fft_stats();

    let mut group = c.benchmark_group("fft");

    for log2_n in [10, 14, 18] {
        let mut rng = StdRng::seed_from_u64(1234);
        let domain = Domain::<Fp>::new(log2_n);
        let data: Vec<Fp> = (0..domain.n()).map(|_| Fp::random(&mut rng)).collect();

        group.bench_with_input(BenchmarkId::from_parameter(log2_n), &log2_n, |b, _| {
            b.iter_batched(
                || data.clone(),
                |mut buf| domain.fft(&mut buf),
                criterion::BatchSize::LargeInput,
            );
        });
    }

    group.finish();

    #[cfg(feature = "accel-fft")]
    {
        let stats = accel_fft_stats();
        eprintln!("fft accel stats: {}", format_accel_fft_stats(&stats));
    }
}

fn ifft_bench(c: &mut Criterion) {
    #[cfg(feature = "accel-fft")]
    reset_accel_fft_stats();

    let mut group = c.benchmark_group("ifft");

    for log2_n in [10, 14, 18] {
        let mut rng = StdRng::seed_from_u64(1234);
        let domain = Domain::<Fp>::new(log2_n);
        let data: Vec<Fp> = (0..domain.n()).map(|_| Fp::random(&mut rng)).collect();

        group.bench_with_input(BenchmarkId::from_parameter(log2_n), &log2_n, |b, _| {
            b.iter_batched(
                || data.clone(),
                |mut buf| domain.ifft(&mut buf),
                criterion::BatchSize::LargeInput,
            );
        });
    }

    group.finish();

    #[cfg(feature = "accel-fft")]
    {
        let stats = accel_fft_stats();
        eprintln!("ifft accel stats: {}", format_accel_fft_stats(&stats));
    }
}

criterion_group!(benches, fft_bench, ifft_bench);
criterion_main!(benches);

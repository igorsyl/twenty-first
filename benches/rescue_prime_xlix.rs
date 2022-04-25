use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use twenty_first::shared_math::b_field_element::BFieldElement;
use twenty_first::shared_math::rescue_prime_xlix::{neptune_params, RescuePrimeXlix};
use twenty_first::shared_math::traits::GetRandomElements;
use twenty_first::util_types::simple_hasher::{Hasher, RescuePrimeProduction};

fn bench_single_elements(c: &mut Criterion) {
    let mut group = c.benchmark_group("rescue_prime_single_elements");

    let size = 1;
    group.sample_size(100);

    let hasher_rp = RescuePrimeProduction::new();
    let hasher_rp_xlix = RescuePrimeXlix::new();

    let mut rng = rand::thread_rng();
    let single_element = BFieldElement::random_elements(size, &mut rng);

    group.bench_function(BenchmarkId::new("RescuePrime", size), |bencher| {
        bencher.iter(|| hasher_rp.hash(&single_element));
    });

    group.bench_function(BenchmarkId::new("RescuePrimeXlix", size), |bencher| {
        bencher.iter(|| hasher_rp_xlix.hash(&single_element, 5));
    });
}

fn bench_many_elements(c: &mut Criterion) {
    let mut group = c.benchmark_group("rescue_prime_many_elements");

    let size = 8_192;
    group.sample_size(50);

    let hasher_rp = RescuePrimeProduction::new();
    let hasher_rp_xlix = RescuePrimeXlix::new();

    let mut rng = rand::thread_rng();
    let elements = BFieldElement::random_elements(size, &mut rng);

    group.bench_function(BenchmarkId::new("RescuePrime", size), |bencher| {
        bencher.iter(|| {
            let chunks: Vec<Vec<BFieldElement>> = elements.chunks(5).map(|s| s.to_vec()).collect();
            hasher_rp.hash_many(&chunks);
        });
    });

    group.bench_function(BenchmarkId::new("RescuePrimeXlix", size), |bencher| {
        bencher.iter(|| {
            hasher_rp_xlix.hash(&elements, 5);
        });
    });
}

fn rescue_prime_16384_rate_12_15(c: &mut Criterion) {
    let mut group = c.benchmark_group("rescue_prime_16384_rate_12_15");

    let size = 16_384;
    group.sample_size(20);

    let hasher_rp_xlix_12 = RescuePrimeXlix::new();
    let mut hasher_rp_xlix_15 = RescuePrimeXlix::new();
    hasher_rp_xlix_15.capacity = 1;

    let mut rng = rand::thread_rng();
    let elements = BFieldElement::random_elements(size, &mut rng);

    group.bench_function(BenchmarkId::new("RescuePrimeXlix-12", size), |bencher| {
        bencher.iter(|| {
            hasher_rp_xlix_12.hash(&elements, 5);
        });
    });

    group.bench_function(BenchmarkId::new("RescuePrimeXlix-15", size), |bencher| {
        bencher.iter(|| {
            hasher_rp_xlix_15.hash(&elements, 5);
        });
    });
}

fn bench_pairs(c: &mut Criterion) {
    let mut group = c.benchmark_group("rescue_prime_pairs");

    let size = 8_192;
    group.sample_size(50);

    let hasher_rp = RescuePrimeProduction::new();
    let hasher_rp_xlix = neptune_params();

    let mut rng = rand::thread_rng();
    let elements = BFieldElement::random_elements(size, &mut rng);

    // Hashing pairs

    group.bench_function(BenchmarkId::new("RescuePrime-hash_pair", size), |bencher| {
        let chunks: Vec<_> = elements.chunks_exact(10).collect();
        bencher.iter(|| {
            for chunk in chunks.iter() {
                let chunk_a = &chunk[0..5].to_vec();
                let chunk_b = &chunk[5..10].to_vec();
                hasher_rp.hash_pair(chunk_a, chunk_b);
            }
        });
    });

    group.bench_function(
        BenchmarkId::new("RescuePrimeXlix-hash_pair", size),
        |bencher| {
            let chunks: Vec<_> = elements.chunks_exact(10).collect();
            bencher.iter(|| {
                for chunk in chunks.iter() {
                    let chunk_a = &chunk[0..5].to_vec();
                    let chunk_b = &chunk[5..10].to_vec();
                    hasher_rp_xlix.hash_pair(chunk_a, chunk_b);
                }
            });
        },
    );
}

criterion_group!(
    benches,
    bench_single_elements,
    bench_many_elements,
    rescue_prime_16384_rate_12_15,
    bench_pairs,
);
criterion_main!(benches);
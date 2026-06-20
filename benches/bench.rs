use criterion::{black_box, criterion_group, criterion_main, Criterion};
use curvepress::{compress, Algo, Config};

fn fracture_series(n: usize) -> (Vec<i64>, Vec<f64>) {
    let ts: Vec<i64> = (0..n as i64).map(|i| i * 100_000).collect();
    let mut val = vec![0.0f64; n];
    let ramp_end = n * 3 / 5;
    let peak = ramp_end;
    let drop_end = peak + 10;
    for i in 0..ramp_end {
        val[i] = i as f64 / ramp_end as f64 * 100.0;
    }
    if peak < n { val[peak] = 150.0; }
    for i in (peak + 1)..drop_end.min(n) {
        val[i] = (100.0 - (i - peak) as f64 * 12.0).max(0.0);
    }
    (ts, val)
}

fn bench_rdp(c: &mut Criterion) {
    let (ts, val) = fracture_series(100_000);
    let cfg = Config { algo: Algo::Rdp, epsilon: 0.5, ..Default::default() };
    c.bench_function("rdp_100k", |b| {
        b.iter(|| compress(black_box(&ts), black_box(&val), black_box(&cfg)).unwrap())
    });
}

fn bench_vw(c: &mut Criterion) {
    let (ts, val) = fracture_series(100_000);
    let cfg = Config { algo: Algo::Vw, n_out: 1000, ..Default::default() };
    c.bench_function("vw_100k", |b| {
        b.iter(|| compress(black_box(&ts), black_box(&val), black_box(&cfg)).unwrap())
    });
}

fn bench_rdp_1m(c: &mut Criterion) {
    let (ts, val) = fracture_series(1_000_000);
    let cfg = Config { algo: Algo::Rdp, epsilon: 0.5, ..Default::default() };
    c.bench_function("rdp_1m", |b| {
        b.iter(|| compress(black_box(&ts), black_box(&val), black_box(&cfg)).unwrap())
    });
}

criterion_group!(benches, bench_rdp, bench_vw, bench_rdp_1m);
criterion_main!(benches);

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sixcy::codec::{Codec, ZstdCodec, Lz4Codec};
fn bench_compression(c: &mut Criterion) {
    let data = vec![0u8; 1024 * 1024]; 
    let zstd = ZstdCodec;
    let lz4 = Lz4Codec;
    c.bench_function("zstd_compress_1mb", |b| b.iter(|| zstd.compress(black_box(&data), 3)));
    c.bench_function("lz4_compress_1mb", |b| b.iter(|| lz4.compress(black_box(&data), 0)));
}
criterion_group!(benches, bench_compression);
criterion_main!(benches);

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sixcy::codec::{Codec, ZstdCodec, Lz4Codec};
use sixcy::io_stream::SixCyWriter;
use sixcy::CodecId;
use std::io::Cursor;

fn bench_compression(c: &mut Criterion) {
    let data = vec![0u8; 1024 * 1024];
    let zstd = ZstdCodec;
    let lz4 = Lz4Codec;

    c.bench_function("zstd_compress_1mb", |b| b.iter(|| zstd.compress(black_box(&data), 3)));
    c.bench_function("lz4_compress_1mb", |b| b.iter(|| lz4.compress(black_box(&data), 0)));
}

fn bench_pack_single_file(c: &mut Criterion) {
    let data = vec![42u8; 1024 * 1024];

    c.bench_function("pack_1mb_zstd", |b| {
        b.iter(|| {
            let buf = Cursor::new(Vec::new());
            let mut writer = SixCyWriter::new(buf).unwrap();
            writer.add_file("bench.bin".to_string(), black_box(&data), CodecId::Zstd).unwrap();
            writer.finalize().unwrap();
        })
    });

    c.bench_function("pack_1mb_lz4", |b| {
        b.iter(|| {
            let buf = Cursor::new(Vec::new());
            let mut writer = SixCyWriter::new(buf).unwrap();
            writer.add_file("bench.bin".to_string(), black_box(&data), CodecId::Lz4).unwrap();
            writer.finalize().unwrap();
        })
    });
}

fn bench_cas_dedup(c: &mut Criterion) {
    let data = vec![99u8; 512 * 1024];

    c.bench_function("cas_dedup_10x_identical", |b| {
        b.iter(|| {
            let buf = Cursor::new(Vec::new());
            let mut writer = SixCyWriter::new(buf).unwrap();
            for i in 0..10 {
                writer.add_file(format!("file_{}.bin", i), black_box(&data), CodecId::Zstd).unwrap();
            }
            writer.finalize().unwrap();
        })
    });
}

criterion_group!(benches, bench_compression, bench_pack_single_file, bench_cas_dedup);
criterion_main!(benches);

//! Benchmarks for TDS protocol encoding and decoding.

#![allow(clippy::unwrap_used, missing_docs)]

use bytes::{Bytes, BytesMut};
use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use tds_protocol::{
    encode_sql_batch,
    packet::{PACKET_HEADER_SIZE, PacketHeader, PacketStatus, PacketType},
    prelogin::{EncryptionLevel, PreLogin},
};

/// Benchmark packet header encoding.
fn bench_packet_header_encode(c: &mut Criterion) {
    let header = PacketHeader::new(PacketType::SqlBatch, PacketStatus::END_OF_MESSAGE, 1000);

    c.bench_function("packet_header_encode", |b| {
        b.iter(|| {
            let mut buf = BytesMut::with_capacity(PACKET_HEADER_SIZE);
            header.encode(&mut buf);
            black_box(buf)
        })
    });
}

/// Benchmark packet header decoding.
fn bench_packet_header_decode(c: &mut Criterion) {
    let header = PacketHeader::new(PacketType::SqlBatch, PacketStatus::END_OF_MESSAGE, 1000);
    let mut buf = BytesMut::with_capacity(PACKET_HEADER_SIZE);
    header.encode(&mut buf);
    let encoded = buf.freeze();

    c.bench_function("packet_header_decode", |b| {
        b.iter(|| {
            let mut cursor = encoded.clone();
            let decoded = PacketHeader::decode(&mut cursor).unwrap();
            black_box(decoded)
        })
    });
}

/// Benchmark PreLogin encoding.
fn bench_prelogin_encode(c: &mut Criterion) {
    let prelogin = PreLogin::new()
        .with_encryption(EncryptionLevel::On)
        .with_mars(true);

    c.bench_function("prelogin_encode", |b| {
        b.iter(|| {
            let encoded = prelogin.encode();
            black_box(encoded)
        })
    });
}

/// Benchmark PreLogin decoding.
fn bench_prelogin_decode(c: &mut Criterion) {
    let prelogin = PreLogin::new()
        .with_encryption(EncryptionLevel::On)
        .with_mars(true);
    let encoded = prelogin.encode();

    c.bench_function("prelogin_decode", |b| {
        b.iter(|| {
            let mut cursor = Bytes::copy_from_slice(&encoded);
            let decoded = PreLogin::decode(&mut cursor).unwrap();
            black_box(decoded)
        })
    });
}

/// Benchmark SQL batch encoding with various query sizes.
fn bench_sql_batch_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_batch_encode");

    // Simple query
    let simple_query = "SELECT 1";
    group.throughput(Throughput::Bytes(simple_query.len() as u64));
    group.bench_function("simple", |b| {
        b.iter(|| {
            let encoded = encode_sql_batch(black_box(simple_query));
            black_box(encoded)
        })
    });

    // Medium query (typical SELECT)
    let medium_query = "SELECT id, name, email, created_at, updated_at FROM users WHERE status = 'active' AND organization_id = 12345 ORDER BY created_at DESC LIMIT 100";
    group.throughput(Throughput::Bytes(medium_query.len() as u64));
    group.bench_function("medium", |b| {
        b.iter(|| {
            let encoded = encode_sql_batch(black_box(medium_query));
            black_box(encoded)
        })
    });

    // Large query (complex join)
    let large_query = "SELECT u.id, u.name, u.email, o.id as order_id, o.total, o.status, \
        p.name as product_name, p.price, oi.quantity \
        FROM users u \
        INNER JOIN orders o ON u.id = o.user_id \
        INNER JOIN order_items oi ON o.id = oi.order_id \
        INNER JOIN products p ON oi.product_id = p.id \
        WHERE u.organization_id = @p1 AND o.created_at >= @p2 AND o.status IN ('pending', 'processing', 'shipped') \
        ORDER BY o.created_at DESC, u.name ASC";
    group.throughput(Throughput::Bytes(large_query.len() as u64));
    group.bench_function("large", |b| {
        b.iter(|| {
            let encoded = encode_sql_batch(black_box(large_query));
            black_box(encoded)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_packet_header_encode,
    bench_packet_header_decode,
    bench_prelogin_encode,
    bench_prelogin_decode,
    bench_sql_batch_encode,
);

criterion_main!(benches);

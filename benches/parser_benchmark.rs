use bytes::BytesMut;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use stream_resp::parser::Parser;

fn benchmark_parser(c: &mut Criterion) {
    let mut parser = Parser::new(10, 1024);
    let mut buffer_simple_string = BytesMut::from("+OK\r\n");
    let mut buffer_error = BytesMut::from("-Error message\r\n");
    let mut buffer_integer = BytesMut::from(":1000\r\n");
    let mut buffer_bulk_string = BytesMut::from("$6\r\nfoobar\r\n");
    let mut buffer_array = BytesMut::from("*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n");

    c.bench_function("parse simple string", |b| {
        b.iter(|| {
            parser.read_buf(black_box(&mut buffer_simple_string));
            parser.try_parse().unwrap();
        })
    });

    c.bench_function("parse error", |b| {
        b.iter(|| {
            parser.read_buf(black_box(&mut buffer_error));
            parser.try_parse().unwrap();
        })
    });

    c.bench_function("parse integer", |b| {
        b.iter(|| {
            parser.read_buf(black_box(&mut buffer_integer));
            parser.try_parse().unwrap();
        })
    });

    c.bench_function("parse bulk string", |b| {
        b.iter(|| {
            parser.read_buf(black_box(&mut buffer_bulk_string));
            parser.try_parse().unwrap();
        })
    });

    c.bench_function("parse array", |b| {
        b.iter(|| {
            parser.read_buf(black_box(&mut buffer_array));
            parser.try_parse().unwrap();
        })
    });
}

criterion_group!(benches, benchmark_parser);
criterion_main!(benches);

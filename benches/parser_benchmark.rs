use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use stream_resp::parser::Parser;

fn benchmark_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("RESP Parser");

    // Configure benchmark parameters
    group.sample_size(100);
    group.measurement_time(std::time::Duration::from_secs(1));
    group.warm_up_time(std::time::Duration::from_secs(1));

    // Test data
    let simple_string = b"+OK\r\n";
    let error = b"-Error message\r\n";
    let integer = b":1000\r\n";
    let bulk_string = b"$6\r\nfoobar\r\n";
    let null_bulk_string = b"$-1\r\n";
    let empty_bulk_string = b"$0\r\n\r\n";
    let array = b"*2\r\n$3\r\nfoo\r\n$3\r\nbar\r\n";
    let nested_array = b"*2\r\n*2\r\n+a\r\n+b\r\n*2\r\n+c\r\n+d\r\n";
    let large_array = create_large_array(100);
    let large_bulk_string = create_large_bulk_string(1000);
    let mixed_types = b"*5\r\n:1\r\n+OK\r\n-Error\r\n$5\r\nhello\r\n*0\r\n";
    let real_command = b"*3\r\n$3\r\nSET\r\n$4\r\nkey1\r\n$6\r\nvalue1\r\n";

    // Benchmark scenarios
    bench_scenario(&mut group, "simple_string", simple_string);
    bench_scenario(&mut group, "error", error);
    bench_scenario(&mut group, "integer", integer);
    bench_scenario(&mut group, "bulk_string", bulk_string);
    bench_scenario(&mut group, "null_bulk_string", null_bulk_string);
    bench_scenario(&mut group, "empty_bulk_string", empty_bulk_string);
    bench_scenario(&mut group, "array", array);
    bench_scenario(&mut group, "nested_array", nested_array);
    bench_scenario(&mut group, "large_array", &large_array);
    bench_scenario(&mut group, "large_bulk_string", &large_bulk_string);
    bench_scenario(&mut group, "mixed_types", mixed_types);
    bench_scenario(&mut group, "real_command", real_command);

    // Benchmark batched commands
    let mut batched_commands = Vec::new();
    batched_commands.extend_from_slice(real_command);
    batched_commands.extend_from_slice(real_command);
    batched_commands.extend_from_slice(real_command);

    group.bench_function("batched_commands", |b| {
        b.iter(|| {
            let mut parser = Parser::new(10, 10000);
            parser.read_buf(&batched_commands);

            // Parse all commands in the batch
            let mut count = 0;
            while let Ok(Some(_cmd)) = parser.try_parse() {
                count += 1;
                if count >= 3 {
                    break;
                }
            }
        })
    });

    // Benchmark chunked parsing
    group.bench_function("chunked_parsing", |b| {
        b.iter(|| {
            let mut parser = Parser::new(100, 10000);

            // First chunk
            parser.read_buf(b"*3\r\n$3\r\nSET\r\n");
            let _ = parser.try_parse();

            // Second chunk
            parser.read_buf(b"$4\r\nkey1\r\n");
            let _ = parser.try_parse();

            // Third chunk
            parser.read_buf(b"$6\r\nvalue1\r\n");
            let _ = parser.try_parse().unwrap();
        })
    });

    group.finish();
}

fn bench_scenario(
    group: &mut criterion::BenchmarkGroup<criterion::measurement::WallTime>,
    name: &str,
    data: &[u8],
) {
    group.bench_with_input(BenchmarkId::new("parse", name), data, |b, data| {
        b.iter(|| {
            let mut parser = Parser::new(100, 10000);
            parser.read_buf(data);
            let _ = parser.try_parse().unwrap();
        })
    });
}

fn create_large_array(size: usize) -> Vec<u8> {
    let mut result = format!("*{}\r\n", size).into_bytes();
    for _ in 0..size {
        result.extend_from_slice(b":1\r\n");
    }
    result
}

fn create_large_bulk_string(size: usize) -> Vec<u8> {
    let data = "x".repeat(size);
    let mut result = format!("${}\r\n", size).into_bytes();
    result.extend_from_slice(data.as_bytes());
    result.extend_from_slice(b"\r\n");
    result
}

criterion_group!(benches, benchmark_parser);
criterion_main!(benches);

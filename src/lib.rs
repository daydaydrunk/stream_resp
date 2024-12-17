use jemallocator::Jemalloc;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

pub mod parser;
#[cfg(test)]
mod parser_test;
pub mod resp;
#[cfg(test)]
mod resp_test;

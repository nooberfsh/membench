use std::ops::Range;

use super::*;
use crate::latency;

/// 根据配置信息测量读取 延迟
///
/// 注意:又不不同的 group_size 可能会对应相同的 working set size, 为了测试的一致性
/// 我们会只分配一块内存,不同的 group_size 会用同一个 working set 进行测试.
/// 在最开始实现这个函数的时候,每一组测试都会分配自己的内存,导致在测试 L3 延迟的时候不同 group_size 之间的差距会变大
/// 出版实现可以参考 **read_latency_buggy**, 猜测可能和内存分配器有关.
pub fn read_latency(
    group_size_list: &[usize],
    working_set_range: Range<u32>,
    round: usize,
    pattern: latency::Pattern,
) -> anyhow::Result<ReadLatency> {
    let mut working_set = Vec::with_capacity(working_set_range.clone().count());
    for ws in working_set_range.clone() {
        let data = latency::alloc_working_set(2usize.pow(ws))?;
        working_set.push(data)
    }

    let mut latencies = vec![];
    let mut max_latency = 0;
    for group_size in group_size_list {
        println!("read_latency group_size: {group_size}");
        let mut avg_cycles = vec![];
        for data in &mut working_set {
            let res = latency::bench_with_data(round, *group_size, data, pattern)?;
            let cycle = res.avg_cycles();
            if cycle > max_latency {
                max_latency = cycle;
            }
            avg_cycles.push(cycle);
        }
        let latency = ReadLatencyLine {
            legend: *group_size,
            avg_cycles,
        };
        latencies.push(latency);
    }
    Ok(ReadLatency {
        working_set_range,
        latencies,
        max_latency,
    })
}

pub fn random_block_read_latency(
    block_count_list: &[usize],
    working_set_range: Range<u32>,
    group_size: usize,
    round: usize,
) -> anyhow::Result<ReadLatency> {
    let mut working_set = Vec::with_capacity(working_set_range.clone().count());
    for ws in working_set_range.clone() {
        let data = latency::alloc_working_set(2usize.pow(ws))?;
        working_set.push(data)
    }

    let mut latencies = vec![];
    let mut max_latency = 0;
    for block_count in block_count_list {
        println!("read_latency block_count: {block_count}");
        let pattern = latency::Pattern::RandomBlock(*block_count);
        let mut avg_cycles = vec![];
        for data in &mut working_set {
            let res = latency::bench_with_data(round, group_size, data, pattern)?;
            let cycle = res.avg_cycles();
            if cycle > max_latency {
                max_latency = cycle;
            }
            avg_cycles.push(cycle);
        }
        let latency = ReadLatencyLine {
            legend: *block_count,
            avg_cycles,
        };
        latencies.push(latency);
    }
    Ok(ReadLatency {
        working_set_range,
        latencies,
        max_latency,
    })
}

pub fn read_latency_pivot(
    group_size_list: &[usize],
    working_set_range: Range<u32>,
    round: usize,
    pattern: latency::Pattern,
) -> anyhow::Result<ReadLatency> {
    let mut max_latency = 0;
    let mut latencies = Vec::with_capacity(group_size_list.len());
    for group_size in group_size_list {
        latencies.push(ReadLatencyLine {
            legend: *group_size,
            avg_cycles: vec![],
        });
    }

    for ws in working_set_range.clone() {
        let mut data = latency::alloc_working_set(2usize.pow(ws))?;

        for line in &mut latencies {
            let res = latency::bench_with_data(round, line.legend, &mut data, pattern)?;
            let cycle = res.avg_cycles();
            if cycle > max_latency {
                max_latency = cycle;
            }

            line.avg_cycles.push(cycle);
        }
    }

    Ok(ReadLatency {
        working_set_range,
        latencies,
        max_latency,
    })
}

pub fn read_latency_buggy(
    group_size_list: &[usize],
    working_set_range: Range<u32>,
    round: usize,
    pattern: latency::Pattern,
) -> anyhow::Result<ReadLatency> {
    let mut latencies = vec![];
    let mut max_latency = 0;
    for group_size in group_size_list {
        let mut avg_cycles = vec![];
        for working_set in working_set_range.clone() {
            let res = latency::bench(round, *group_size, 2usize.pow(working_set), pattern)?;
            let cycle = res.avg_cycles();
            if cycle > max_latency {
                max_latency = cycle;
            }
            avg_cycles.push(cycle);
        }
        let latency = ReadLatencyLine {
            legend: *group_size,
            avg_cycles,
        };
        latencies.push(latency);
    }
    Ok(ReadLatency {
        working_set_range,
        latencies,
        max_latency,
    })
}

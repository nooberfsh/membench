use std::ops::Range;

use super::*;
use crate::latency2;

pub fn read_latency(
    group_size_list: &[usize],
    working_set_range: Range<u32>,
    round: usize,
    pattern: latency2::Pattern,
) -> anyhow::Result<ReadLatency> {
    let mut latencies = vec![];
    let mut max_latency = 0;
    for group_size in group_size_list {
        let mut avg_cycles = vec![];
        for working_set in working_set_range.clone() {
            let res = latency2::bench(round, *group_size, 2usize.pow(working_set), pattern)?;
            let cycle = res.avg_cycles();
            if cycle > max_latency {
                max_latency = cycle;
            }
            avg_cycles.push(cycle);
        }
        let latency = ReadLatencyLine {
            group_size: *group_size,
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

pub fn read_latency2(
    group_size_list: &[usize],
    working_set_range: Range<u32>,
    round: usize,
    pattern: latency2::Pattern,
) -> anyhow::Result<ReadLatency> {
    let mut working_set = Vec::with_capacity(working_set_range.clone().count());
    for ws in working_set_range.clone() {
        let data = latency2::alloc_working_set(2usize.pow(ws))?;
        working_set.push(data)
    }

    let mut latencies = vec![];
    let mut max_latency = 0;
    for group_size in group_size_list {
        let mut avg_cycles = vec![];
        for data in &mut working_set {
            let res = latency2::bench_with_data(round, *group_size, data, pattern)?;
            let cycle = res.avg_cycles();
            if cycle > max_latency {
                max_latency = cycle;
            }
            avg_cycles.push(cycle);
        }
        let latency = ReadLatencyLine {
            group_size: *group_size,
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

pub fn read_latency3(
    group_size_list: &[usize],
    working_set_range: Range<u32>,
    round: usize,
    pattern: latency2::Pattern
) -> anyhow::Result<ReadLatency> {
    let mut max_latency = 0;
    let mut latencies = Vec::with_capacity(group_size_list.len());
    for group_size in group_size_list {
        latencies.push(ReadLatencyLine {
            group_size: *group_size,
            avg_cycles: vec![],
        });
    }

    for ws in working_set_range.clone() {
        let mut data = latency2::alloc_working_set(2usize.pow(ws))?;

        for line in &mut latencies{
            let res = latency2::bench_with_data(round, line.group_size, &mut data, pattern)?;
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

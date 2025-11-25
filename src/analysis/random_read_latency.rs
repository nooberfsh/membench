use std::ops::Range;

use super::*;
use crate::latency2;

pub fn random_read_latency(
    group_size_list: &[usize],
    working_set_range: Range<u32>,
    round: usize,
) -> anyhow::Result<ReadLatency>{
    let mut latencies = vec![];
    let pattern = latency2::Pattern::Random;
    let mut max_latency = 0;
    for group_size in group_size_list {
        let mut avg_cycles = vec![];
        for working_set in working_set_range.clone() {
            let res =  latency2::bench(round, *group_size, 2usize.pow(working_set), pattern)?;
            let cycle = res.avg_cycles();
            if cycle > max_latency {
                max_latency = cycle;
            }
            avg_cycles.push(cycle);
        }
        let latency = ReadLatencyLine {
            group_size: *group_size,
            avg_cycles
        };
        latencies.push(latency);
    }
    Ok(ReadLatency {
        working_set_range,
        latencies,
        max_latency,
    })
}


use std::{
    mem::align_of,
    mem::size_of,
    time::{Duration, Instant},
};

use core_affinity::CoreId;
use perf_event::events::Hardware;
use perf_event::{Builder, Group};
use tabled::Table;

pub mod cache;
pub mod latency;
pub mod latency2;

#[allow(non_upper_case_globals)]
pub const SIZE_1GiB: usize = 1024 * 1024 * 1024;
pub const SIZE_1GB: usize = 1000 * 1000 * 1000;
pub const CACHE_LINE_SIZE: usize = 64;

#[derive(Copy, Clone)]
#[repr(align(64))]
pub struct CacheLine(#[allow(unused)] u64);

impl CacheLine {
    pub fn alloc(size: usize) -> Vec<Self> {
        assert_eq!(align_of::<CacheLine>(), CACHE_LINE_SIZE);
        assert_eq!(size_of::<CacheLine>(), CACHE_LINE_SIZE);

        assert_eq!(
            size % CACHE_LINE_SIZE,
            0,
            "分配的数据大小必须是 cache line 的倍数"
        );

        let mut rng = fastrand::Rng::new();
        let len = size / size_of::<CacheLine>();
        let mut data = Vec::with_capacity(len);
        for _ in 0..len {
            data.push(CacheLine(rng.u64(..)));
        }
        data
    }
}

#[derive(Clone, Debug, Copy, Default)]
struct BenchRes {
    core: Option<CoreId>,
    time: Duration,
    bytes: usize,
}

impl BenchRes {
    fn new() -> Self {
        Default::default()
    }

    fn desc(&self) {
        let total = self.bytes / SIZE_1GiB;
        let bw = total as f64 / self.time.as_secs_f64();
        let bw2 = (self.bytes / SIZE_1GB) as f64 / self.time.as_secs_f64();

        let msg = format!(
            "time: {:.2?}, size: {}GiB, bandwidth: {:.2}GiB/s, {:.2}GB/s",
            self.time, total, bw, bw2
        );
        let core_id = self
            .core
            .clone()
            .map(|x| x.id.to_string())
            .unwrap_or_else(|| "*".to_string());
        println!("core[{}] {msg}", core_id);
    }
}

fn main() -> anyhow::Result<()> {
    let caches = cache::get_cpu_cache(0)?;
    let table = Table::new(caches);
    println!("{table}");

    // for c in caches {
    //     println!("{}", c);
    // }
    //test2();
    Ok(())
}

fn test2() {
    let round = 100;
    let working_set_size = 4 * size::MiB;
    let res = latency::bench::<0>(round, working_set_size as usize).unwrap();
    res.report();

    //let working_set_size = 16* size::KiB;
    let res = latency::bench::<1>(round, working_set_size as usize).unwrap();
    res.report();

    //let working_set_size = 16 * size::KiB;
    let res = latency::bench::<3>(round, working_set_size as usize).unwrap();
    res.report();

    //let working_set_size = 32 * size::KiB;
    let res = latency::bench::<7>(round, working_set_size as usize).unwrap();
    res.report();

    let pattern = latency2::Pattern::Random;
    let res = latency2::bench(round, 1, working_set_size as usize, pattern).unwrap();
    res.report();
    let res = latency2::bench(round, 2, working_set_size as usize, pattern).unwrap();
    res.report();
    let res = latency2::bench(round, 4, working_set_size as usize, pattern).unwrap();
    res.report();
    let res = latency2::bench(round, 8, working_set_size as usize, pattern).unwrap();
    res.report();
}

fn test1() {
    let ids = vec![0];
    let core_ids: Vec<_> = core_affinity::get_core_ids()
        .unwrap()
        .into_iter()
        .filter(|x| ids.contains(&x.id))
        .collect();
    let round = 10;
    let size_per_core = SIZE_1GiB * 16;
    let size = size_per_core * core_ids.len();
    let data = CacheLine::alloc(size);

    assert_eq!(data.len() * size_of::<CacheLine>(), size);

    let mut res = BenchRes::new();
    res.bytes = size * round;

    let time = Instant::now();

    bench_bandwidth_read2(&data, round);

    res.time = time.elapsed();
    res.desc();
}

fn bench_bandwidth_read(data: &[CacheLine], round: usize, core_ids: Vec<CoreId>) {
    assert_eq!(data.len() % core_ids.len(), 0);
    let len_per_core = data.len() / core_ids.len();

    std::thread::scope(|scope| {
        for (i, id) in core_ids.into_iter().enumerate() {
            let data = &data[i * len_per_core..(i + 1) * len_per_core];

            scope.spawn(move || {
                // Pin this thread to a single CPU core.
                let ok = core_affinity::set_for_current(id);
                if !ok {
                    panic!("set affinity failed, coreid: {}", id.id)
                }
                let time = Instant::now();
                for _ in 0..round {
                    std::hint::black_box(read(&data));
                }
                let res = BenchRes {
                    core: Some(id),
                    time: time.elapsed(),
                    bytes: len_per_core * size_of::<CacheLine>() * round,
                };
                res.desc();
            });
        }
    })
}

fn bench_bandwidth_read2(data: &[CacheLine], round: usize) -> anyhow::Result<()> {
    let id = CoreId { id: 0 };

    let ok = core_affinity::set_for_current(id);
    if !ok {
        panic!("set affinity failed, coreid: {}", id.id)
    }
    let time = Instant::now();
    let mut group = Group::new()?;
    let cycles = group.add(&Builder::new(Hardware::CPU_CYCLES))?;

    group.enable()?;
    for _ in 0..round {
        std::hint::black_box(read(&data));
    }
    group.disable()?;

    let counts = group.read()?;

    let total_cycles: u64 = counts[&cycles];
    let avg_cycles = total_cycles / data.len() as u64 / round as u64;
    println!("avg cycles: {avg_cycles}");

    let res = BenchRes {
        core: Some(id),
        time: time.elapsed(),
        bytes: data.len() * size_of::<CacheLine>() * round,
    };
    res.desc();
    let avg_latency = res.time.as_nanos() as f64 / data.len() as f64 / round as f64;
    println!("avg latency: {avg_latency}");
    Ok(())
}

fn read(data: &[CacheLine]) -> u64 {
    let mut ret = 0;
    for cl in data {
        ret += cl.0;
    }
    ret
}

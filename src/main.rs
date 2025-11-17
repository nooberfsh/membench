use std::{
    mem::align_of,
    mem::size_of,
    time::{Duration, Instant},
};

use core_affinity::CoreId;

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

fn main() {
    let ids = vec![0, 8];
    let core_ids: Vec<_> = core_affinity::get_core_ids()
        .unwrap()
        .into_iter()
        .filter(|x| ids.contains(&x.id))
        .collect();
    let round = 10;
    let size_per_core = SIZE_1GiB * 32;
    let size = size_per_core * core_ids.len();
    let data = CacheLine::alloc(size);

    assert_eq!(data.len() * size_of::<CacheLine>(), size);

    let mut res = BenchRes::new();
    res.bytes = size * round;

    let time = Instant::now();

    bench_bandwidth_read(&data, round, core_ids);

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

// fn read(data: &[CacheLine]) {
//     use std::arch::asm;
//     for cl in data {
//         unsafe {
//             asm!(
//                 "mov {temp}, [{x}]",
//                 temp = out(reg) _,
//                 x = in(reg) cl,
//             )
//         }
//     }
// }

fn read(data: &[CacheLine]) -> u64 {
    let mut ret = 0;
    for cl in data {
        ret += cl.0;
    }
    ret
}

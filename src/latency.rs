use std::hint::black_box;
use std::ptr::null;

use anyhow::bail;
use core_affinity::CoreId;
use perf_event::events::{Cache, CacheId, CacheOp, CacheResult, Hardware};
use perf_event::{Builder, Group};

// TODO: 不同 cpu 可能有不同的 cacheline size
pub const CACHE_LINE_SIZE: usize = 64;

pub struct BenchResult {
    pub round: usize,
    pub working_set_size: usize,
    pub element_size: usize,
    pub element_len: usize,
    pub stats: BenchStats,
}

impl BenchResult {
    pub fn report(&self) {
        let avg_cycles = self.stats.cycles / self.element_len as u64 / self.round as u64;
        let miss_rate = self.stats.l1_miss as f64 / self.stats.l1_access as f64;
        println!(
            "round: {}, working_set_size: {}, element size: {}, element len: {}",
            self.round,
            size::Size::from_bytes(self.working_set_size),
            self.element_size,
            self.element_len
        );
        println!("avg cycles: {avg_cycles}, miss: {}, access: {}, miss_rate: {miss_rate:.2}", self.stats.l1_miss, self.stats.l1_access);
    }
}

pub struct BenchStats {
    pub cycles: u64,
    pub l1_access: u64,
    pub l1_miss: u64,
}

pub struct Element<const PAD: usize> {
    pub n: *const Element<PAD>,
    _pad: [usize; PAD],
}

fn prepare_working_set<const PAD: usize>(len: usize) -> anyhow::Result<Vec<Element<PAD>>> {
    assert!(len > 0);
    let mut ret = Vec::with_capacity(len);
    for i in 0..len {
        ret.push(Element {
            n: null(),
            _pad: [i; PAD],
        });
    }

    for i in 0..len {
        if i != len - 1 {
            ret[i].n = ret[i + 1..].as_ptr();
        }
    }

    Ok(ret)
}

fn prepare_working_set_rand<const PAD: usize>(len: usize) -> anyhow::Result<Vec<Element<PAD>>> {
    assert!(len > 0);
    let mut ret = Vec::with_capacity(len);
    for i in 0..len {
        ret.push(Element {
            n: null(),
            _pad: [i; PAD],
        });
    }

    let mut idx: Vec<usize> = (1..len).collect();
    fastrand::shuffle(&mut idx);
    //println!("\n{:?}\n", idx);

    let mut current = 0;
    while let Some(next) = idx.pop() {
        let p = ret[next..].as_ptr();
        ret[current].n = p;
        current = next;
    }

    Ok(ret)
}

fn bench_impl<const PAD: usize>(data: &[Element<PAD>], round: usize) -> anyhow::Result<BenchStats> {
    const ACCESS: Cache = Cache {
        which: CacheId::L1D,
        operation: CacheOp::READ,
        result: CacheResult::ACCESS,
    };
    const MISS: Cache = Cache {
        result: CacheResult::MISS,
        ..ACCESS
    };
    

    let mut group = Group::new()?;
    let access_counter = group.add(&Builder::new(ACCESS))?;
    let miss_counter = group.add(&Builder::new(MISS))?;
    let cycles = group.add(&Builder::new(Hardware::CPU_CYCLES))?;

    group.enable()?;
    // 注意: 需要把所有 round 的统计只能在一个 perf counter 中统计.
    // 每个 round 单独分配 perf counter 的话, 由于每个 counter 初始化的时候会清空缓存,导致最终的统计失效
    for _ in 0..round {
        let _ = black_box(read(data));
    }
    group.disable()?;

    let counts = group.read()?;
    let ret = BenchStats {
        cycles: counts[&cycles],
        l1_access: counts[&access_counter],
        l1_miss: counts[&miss_counter],
    };
    Ok(ret)
}

fn read<const PAD: usize>(data: &[Element<PAD>]) -> usize {
    debug_assert!(!data.is_empty());
    unsafe {
        let mut cursor = data.get_unchecked(0);
        while let Some(r) = cursor.n.as_ref() {
            cursor = r;
        }
        // 把最后一个元素的地址读取出来,防止编译器把整个循环优化掉
        (cursor as *const Element<PAD>).addr()
    }
}

fn check<const PAD: usize>(data: &[Element<PAD>]) {
    debug_assert!(!data.is_empty());
    let mut refs = vec![];
    unsafe {
        let mut cursor = data.get_unchecked(0);
        refs.push((cursor as *const Element<PAD>).addr());
        while let Some(r) = cursor.n.as_ref() {
            cursor = r;
            refs.push((cursor as *const Element<PAD>).addr());
        }
    }
    refs.sort();
    refs.dedup();
    assert_eq!(refs.len(), data.len());

    for (d, p) in data.iter().zip(refs.iter()) {
        let origin = d as *const Element<PAD>;
        assert_eq!(origin.addr(), *p);
    }
    println!("check finish");
}

/// PAD 必须等于 0 或者 PAD+1 必须是8的倍数, Element size 可以是 usize 的大小或者是 cache line 大小的整数倍
/// working set size 必须是 Element size 的整数倍
pub fn bench<const PAD: usize>(
    round: usize,
    working_set_size: usize,
) -> anyhow::Result<BenchResult> {
    let element_size = size_of::<Element<PAD>>();

    let ok = core_affinity::set_for_current(CoreId{id: 0});
    if !ok {
        panic!("set affinity failed, coreid: {}", 0);
    }
    

    if PAD == 0 {
        assert_eq!(element_size, size_of::<usize>());
    } else if (PAD + 1) % 8 == 0 {
        assert_eq!(element_size, (PAD + 1) * size_of::<usize>());
        if element_size % CACHE_LINE_SIZE != 0 {
            bail!("Element size 必须是 size_of::<usize> 或者是 cache line 大小的整数倍")
        }
    } else {
        //bail!("PAD 必须等于 0 或者 PAD+1 必须是8的倍数")
    }

    if working_set_size % element_size != 0 {
        bail!("working set size 必须是 Element size 的整数倍")
    }

    let len = working_set_size / element_size;
    let data = prepare_working_set_rand::<PAD>(len)?;
    check(&data);
    assert_eq!(data.len(), len);
    assert_eq!(len * element_size, working_set_size);

    let stats = bench_impl(&data, round)?;
    let ret = BenchResult {
        round,
        working_set_size: working_set_size,
        element_size: size_of::<Element<PAD>>(),
        element_len: len,
        stats,
    };
    Ok(ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_working_set() -> anyhow::Result<()> {
        let data = prepare_working_set::<7>(8)?;

        let mut cursor = data.as_ptr();
        let mut accu = 0;
        while let Some(r) = unsafe { cursor.as_ref() } {
            accu += r._pad[0];
            cursor = r.n;
        }
        assert_eq!(accu, (0..8).sum());
        Ok(())
    }
}

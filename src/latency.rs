use std::hint::black_box;
use std::ptr::null;

use anyhow::bail;
use core_affinity::CoreId;
use perf_event::events::{Cache, CacheId, CacheOp, CacheResult, Hardware};
use perf_event::{Builder, Group};

// TODO: 不同 cpu 可能有不同的 cacheline size
pub const CACHE_LINE_SIZE: usize = 64;
pub const ELEMENTS_PER_CACHE_LINE: usize = 64 / size_of::<Element>();

pub struct BenchResult {
    pub round: usize,
    pub working_set_size: usize,
    pub group_size: usize,
    pub group_len: usize,
    pub stats: BenchStats,
}

impl BenchResult {
    pub fn avg_cycles(&self) -> u64 {
        self.stats.cycles / self.group_len as u64 / self.round as u64
    }

    pub fn report(&self) {
        let avg_cycles = self.avg_cycles();
        let miss_rate = self.stats.l1d_miss as f64 / self.stats.l1d_access as f64;
        println!(
            "round: {}, working_set_size: {}, group size: {}, group len: {}",
            self.round,
            size::Size::from_bytes(self.working_set_size),
            self.group_size,
            self.group_len
        );
        println!(
            "avg cycles: {avg_cycles}, miss: {}, access: {}, miss_rate: {miss_rate:.2}",
            self.stats.l1d_miss, self.stats.l1d_access
        );
    }
}

/// 注意: 硬件计数器的数量是有限的,所以这里最好不要超过4个指标,具体的数量和硬件相关
/// 具体请看[这里](https://docs.rs/perf-event2/latest/perf_event/struct.Group.html#limits-on-group-size)
pub struct BenchStats {
    pub cycles: u64,
    pub l1d_access: u64,
    pub l1d_miss: u64,
}

#[derive(Copy, Clone)]
pub struct Element(*const Element);

#[derive(Copy, Clone)]
#[repr(align(64))]
pub struct CacheLine {
    data: [Element; ELEMENTS_PER_CACHE_LINE],
}

impl CacheLine {
    pub fn empty() -> Self {
        CacheLine {
            data: [Element(null()); ELEMENTS_PER_CACHE_LINE],
        }
    }
}

#[derive(Copy, Clone)]
pub enum Pattern {
    Seq,
    Random,
    RandomBlock(usize),
}

// working set 中数据是以 group 为基本单位, 每次访问只读取group 的第一个 Element
pub struct WorkingSetData<'a> {
    pub data: &'a mut [CacheLine],
    // 一个 group 中 Element 的数量
    pub group_size: usize,
    // working set 中 group 的数量,
    pub group_len: usize,
    // 工作集字节数
    pub working_set_size: usize,
    // 访问模式
    pattern: Pattern,
}

pub fn alloc_working_set(working_set_size: usize) -> anyhow::Result<Vec<CacheLine>> {
    if working_set_size % CACHE_LINE_SIZE != 0 {
        bail!("working set size 必须是 cache line size 的整数倍")
    }

    let cache_line_len = working_set_size / CACHE_LINE_SIZE;
    let data = vec![CacheLine::empty(); cache_line_len];
    Ok(data)
}

impl<'a> WorkingSetData<'a> {
    pub fn new(
        data: &'a mut [CacheLine],
        group_size: usize,
        pattern: Pattern,
    ) -> anyhow::Result<Self> {
        let working_set_size = data.len() * CACHE_LINE_SIZE;

        let element_size = size_of::<Element>();

        // 保证 Element size 等于系统字长
        assert_eq!(element_size, size_of::<usize>());
        assert_eq!(element_size * ELEMENTS_PER_CACHE_LINE, CACHE_LINE_SIZE);
        assert_eq!(size_of::<CacheLine>(), CACHE_LINE_SIZE);
        assert_eq!(align_of::<CacheLine>(), CACHE_LINE_SIZE);

        if working_set_size == 0 {
            bail!("working set size 必须 >0")
        }

        if group_size == 0 {
            bail!("group size 必须 >0")
        }

        if working_set_size % group_size != 0 {
            bail!("working set size 必须是 group size 的整数倍")
        }
        if working_set_size < group_size * element_size {
            bail!("working set size 不能小于一个 group 字节数, group_size: {group_size}")
        }

        let group_len = working_set_size / group_size / element_size;
        let mut block_group_len = group_len;
        if let Pattern::RandomBlock(block_count) = pattern {
            let block_size = working_set_size / block_count;
            if working_set_size % block_count != 0 {
                bail!("working set size 必须是 block count 的整数倍")
            }
            if block_size % CACHE_LINE_SIZE != 0 {
                bail!("block size 必须是 cache line size 的整数倍")
            }

            if block_size % group_size != 0 {
                bail!("block size 必须是 group size 的整数倍")
            }

            block_group_len = group_len / block_count;
            if group_len % block_count != 0 {
                bail!("group len 必须是 block count 的整数倍, group_len: {group_len}, block_count: {block_count}")
            }
        }


        // 重新初始化 working set
        for cl in &mut *data {
            *cl = CacheLine::empty();
        }

        // 根据 pattern 计算出元素的访问的顺序.
        // 第一个访问的元素始终是 data[0].data[0]
        let group_idx: Vec<usize> = match pattern {
            Pattern::Seq => (0..group_len).collect(),
            Pattern::Random => {
                let mut idx: Vec<usize> = (0..group_len).collect();
                fastrand::shuffle(&mut idx[1..]);
                idx
            }
            Pattern::RandomBlock(block_count) => {
                assert_eq!(block_count * block_group_len, group_len);
                let mut idx: Vec<usize> = (0..group_len).collect();
                for i in 0..block_count {
                    let start = i * block_group_len + 1;
                    let end = (i + 1) * block_group_len;
                    fastrand::shuffle(&mut idx[start..end]);
                }

                todo!()
            }
        };
        assert_eq!(group_idx.len(), group_len);
        assert_eq!(group_idx.get(0), Some(&0), "遍历从第一个元素开始");

        let location = |group_idx: usize| {
            let element_idx = group_idx * group_size;
            let cache_line_idx = element_idx / ELEMENTS_PER_CACHE_LINE;
            let cache_line_offset = element_idx % ELEMENTS_PER_CACHE_LINE;
            (cache_line_idx, cache_line_offset)
        };

        // 第一个元素作为起始元素
        let mut current = 0;
        // 把所有 group 按照 group_idx 串联起来.
        for next in &group_idx[1..] {
            let (current_cl_idx, current_cl_offset) = location(current);
            let (next_cl_idx, next_cl_offset) = location(*next);
            let ptr = data[next_cl_idx].data[next_cl_offset..].as_ptr();
            data[current_cl_idx].data[current_cl_offset].0 = ptr;
            current = *next;
        }

        let ret = Self {
            data,
            group_size,
            group_len,
            working_set_size,
            pattern,
        };

        ret.check(&group_idx);
        Ok(ret)
    }

    fn check(&self, expect_group_idx: &[usize]) {
        assert!(!self.data.is_empty());
        assert_eq!(self.data.len() * CACHE_LINE_SIZE, self.working_set_size);
        assert_eq!(
            self.working_set_size,
            self.group_size * self.group_len * size_of::<Element>()
        );

        // 遍历 data 记录访问的元素的 group idx
        let mut group_idx = vec![0];
        let base = self.data[0].data[0..].as_ptr();
        unsafe {
            let mut cursor = &self.data[0].data[0];
            while let Some(r) = cursor.0.as_ref() {
                // 计算下一个元素到第一个元素的距离
                let element_idx = cursor.0.offset_from(base);
                assert!(element_idx > 0);
                let g_idx = element_idx as usize / self.group_size;
                assert_eq!(element_idx as usize % self.group_size, 0);
                group_idx.push(g_idx);
                cursor = r;
            }
        }

        assert_eq!(group_idx, expect_group_idx);
    }
}

// 测量数据读取延迟的核心函数,函数编译后必须保证没有多余的指令来保证测量的准确性.
// 可以在[godbolt](https://rust.godbolt.org/z/cj87KxcKd) 查看对应的汇编
fn read(data: &[CacheLine]) -> usize {
    debug_assert!(!data.is_empty());
    unsafe {
        let mut cursor = data.get_unchecked(0).data.get_unchecked(0);
        while let Some(r) = cursor.0.as_ref() {
            cursor = r;
        }
        // 把最后一个元素的地址读取出来,防止编译器把整个循环优化掉
        (cursor as *const Element).addr()
    }
}

fn bench_impl(data: &[CacheLine], round: usize) -> anyhow::Result<BenchStats> {
    let l1d_access: Cache = Cache {
        which: CacheId::L1D,
        operation: CacheOp::READ,
        result: CacheResult::ACCESS,
    };
    let l1d_miss: Cache = Cache {
        result: CacheResult::MISS,
        ..l1d_access
    };

    let mut group = Group::new()?;
    let l1d_access_counter = group.add(&Builder::new(l1d_access))?;
    let l1d_miss_counter = group.add(&Builder::new(l1d_miss))?;
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
        l1d_access: counts[&l1d_access_counter],
        l1d_miss: counts[&l1d_miss_counter],
    };
    Ok(ret)
}

pub fn bench(
    round: usize,
    group_size: usize,
    working_set_size: usize,
    pattern: Pattern,
) -> anyhow::Result<BenchResult> {
    let mut data = alloc_working_set(working_set_size)?;
    bench_with_data(round, group_size, &mut data, pattern)
}

pub fn bench_with_data(
    round: usize,
    group_size: usize,
    data: &mut [CacheLine],
    pattern: Pattern,
) -> anyhow::Result<BenchResult> {
    let working_set_size = data.len() * CACHE_LINE_SIZE;
    let data = WorkingSetData::new(data, group_size, pattern)?;

    let ok = core_affinity::set_for_current(CoreId { id: 0 });
    if !ok {
        panic!("set affinity failed, coreid: {}", 0);
    }

    let stats = bench_impl(&data.data, round)?;
    let ret = BenchResult {
        round,
        working_set_size,
        group_size: data.group_size,
        group_len: data.group_len,
        stats,
    };
    Ok(ret)
}

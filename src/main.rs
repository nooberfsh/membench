use tabled::Table;

pub mod analysis;
pub mod cache;
pub mod latency;
pub mod latency2;


fn main() -> anyhow::Result<()> {
    // let data = analysis::random_read_latency(&[1, 2, 4, 8], 10..25, 50)?;
    // analysis::plot_read_latency(data, "random_read")?;
    // let data = analysis::random_read_latency2(&[1, 4, 8, 16], 10..25, 100)?;
    // analysis::plot_read_latency(data, "random_read2")?;
    // let data = analysis::random_read_latency3(&[1, 4, 8, 16], 10..25, 50)?;
    // analysis::plot_read_latency(data, "random_read3")?;
    // for c in caches {
    //     println!("{}", c);
    // }
    test2();
    Ok(())
}

fn dump_cache_info(cpu: usize) -> anyhow::Result<()> {
    let caches = cache::get_cpu_cache(0)?;
    let table = Table::new(caches);
    println!("{table}");
    Ok(())
}

fn test2() {
    let round = 100;
    let working_set_size = 512 * size::KiB;
    // let res = latency::bench::<0>(round, working_set_size as usize).unwrap();
    // res.report();

    // //let working_set_size = 16* size::KiB;
    // let res = latency::bench::<1>(round, working_set_size as usize).unwrap();
    // res.report();

    // //let working_set_size = 16 * size::KiB;
    // let res = latency::bench::<3>(round, working_set_size as usize).unwrap();
    // res.report();

    // //let working_set_size = 32 * size::KiB;
    // let res = latency::bench::<7>(round, working_set_size as usize).unwrap();
    // res.report();

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

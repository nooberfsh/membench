use structopt::StructOpt;
use tabled::Table;

pub mod analysis;
pub mod cache;
pub mod latency;
pub mod latency2;

#[derive(Debug, StructOpt)]
#[structopt(name = "membench", about = "Memory analysis and benchmark tool")]
struct Opt {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    CacheInfo {
        /// 指定某个 cpu core 的缓存
        #[structopt(short, long, default_value = "0")]
        core: usize,
    },
    Analysis {
        /// 指定 profile 路径
        path: String,
    },
}

fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    match opt.cmd {
        Command::CacheInfo { core } => dump_cache_info(core)?,
        Command::Analysis { path } => {
            let profile = analysis::load(&path)?;
            analysis::analysis_with_profile(&profile)?;
        }
    }
    test2();
    Ok(())
}

fn dump_cache_info(core: usize) -> anyhow::Result<()> {
    let caches = cache::get_cpu_cache(core)?;
    let table = Table::new(caches);
    println!("{table}");
    Ok(())
}

fn test2() {
    let round = 100;
    let working_set_size = 4 * size::MiB;
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
    let res = latency2::bench(round, 4, working_set_size as usize, pattern).unwrap();
    res.report();
    let res = latency2::bench(round, 8, working_set_size as usize, pattern).unwrap();
    res.report();
    let res = latency2::bench(round, 16, working_set_size as usize, pattern).unwrap();
    res.report();
}

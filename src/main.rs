use structopt::StructOpt;
use tabled::Table;

pub mod analysis;
pub mod cache;
pub mod latency;

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
    Ok(())
}

fn dump_cache_info(core: usize) -> anyhow::Result<()> {
    let caches = cache::get_cpu_cache(core)?;
    let table = Table::new(caches);
    println!("{table}");
    Ok(())
}

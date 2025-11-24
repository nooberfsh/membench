use std::fmt::Display;
use std::fs::read_to_string;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::bail;

#[derive(Copy, Clone, Debug)]
pub enum CacheType {
    L1D,
    L1I,
    L2,
    L3,
}

impl CacheType {
    pub fn as_str(&self) -> &str {
        match self {
            CacheType::L1D => "L1D",
            CacheType::L1I => "L1I",
            CacheType::L2 => "L2",
            CacheType::L3 => "L3",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Cache {
    pub ty: CacheType,
    pub size: usize,
    pub coherency_line_size: usize,
    pub ways_of_associativity: usize,
}

impl Cache {
    pub fn sets(&self) -> usize {
        self.size / self.coherency_line_size / self.ways_of_associativity
    }
}

// TODO: support other platforms
pub fn get_cpu_cache(cpu: usize) -> anyhow::Result<Vec<Cache>> {
    let base: PathBuf = format!("/sys/bus/cpu/devices/cpu{cpu}/cache/").into();
    if !base.is_dir() {
        bail!("directory {} not exists", base.display())
    }

    let mut ret = vec![];
    for i in 0.. {
        let mut index = base.clone();
        index.push(format!("index{i}"));
        if index.exists() {
            let d = get_cache_info(&index)?;
            ret.push(d);
        } else {
            break;
        }
    }

    Ok(ret)
}

fn get_cache_info(dir: &Path) -> anyhow::Result<Cache> {
    let level = load_int(dir, "level")?;

    let ty = load_string(dir, "type")?;
    let ty = ty.trim();

    let cache_ty = match (level, ty) {
        (1, "Data") => CacheType::L1D,
        (1, "Instruction") => CacheType::L1I,
        (2, "Unified") => CacheType::L2,
        (3, "Unified") => CacheType::L3,
        _ => bail!("unkown cache type, level: {level}, type: {ty}"),
    };

    let mut size_path = dir.to_owned();
    size_path.push("size");
    let size = read_to_string(&size_path)?;
    let size = size.trim();
    let size = if let Some(s) = size.strip_suffix("K") {
        let s: usize = s.parse()?;
        s * 1024
    } else {
        bail!("invalid cache size: {size}")
    };

    let coherency_line_size = load_int(dir, "coherency_line_size")?;
    let ways_of_associativity = load_int(dir, "ways_of_associativity")?;

    Ok(Cache {
        ty: cache_ty,
        size,
        coherency_line_size,
        ways_of_associativity,
    })
}

fn load_string(dir: &Path, name: &str) -> anyhow::Result<String> {
    let mut path = dir.to_owned();
    path.push(name);
    let s = read_to_string(&path).with_context(|| format!("failed to load {}", path.display()))?;
    Ok(s)
}

fn load_int(dir: &Path, name: &str) -> anyhow::Result<usize> {
    let s = load_string(dir, name)?;
    let num = s.trim().parse()?;
    Ok(num)
}

impl Display for CacheType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Display for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let size = size::Size::from_const(self.size as i64);
        write!(
            f,
            "{}: size: {}, coherency_line_size: {}, ways_of_associativity: {}, sets: {}",
            self.ty.as_str(),
            size,
            self.coherency_line_size,
            self.ways_of_associativity,
            self.sets()
        )
    }
}

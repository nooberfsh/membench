use std::{fs::read_to_string, ops::Range};

use anyhow::Context;
use serde::Deserialize;
use serde::Deserializer;

#[derive(Deserialize, Debug)]
pub struct Profile {
    pub name: String,
    #[serde(deserialize_with = "deserialize_profile_kind")]
    pub kind: ProfileKind,
    pub group_size: Vec<usize>,
    pub working_set: Range<u32>,
    pub round: usize,
    pub block_count: Option<Vec<usize>>,
}

#[derive(Deserialize, Debug)]
pub enum ProfileKind {
    RandomReadLatency,
    RandomBlockReadLatency,
    SeqReadLatency,
}

fn deserialize_profile_kind<'de, D>(deserializer: D) -> Result<ProfileKind, D::Error>
where
    D: Deserializer<'de>,
{
    let buf = String::deserialize(deserializer)?;
    match buf.as_str() {
        "random_read_latency" => Ok(ProfileKind::RandomReadLatency),
        "random_block_read_latency" => Ok(ProfileKind::RandomBlockReadLatency),
        "seq_read_latency" => Ok(ProfileKind::SeqReadLatency),
        _ => Err(serde::de::Error::custom(format!(
            "invalid profile kind: {buf}"
        ))),
    }
}

pub fn load(s: &str) -> anyhow::Result<Profile> {
    let s = read_to_string(s).with_context(|| format!("failed to load profile from: {s}"))?;
    let p = toml::from_str(s.as_str())?;
    Ok(p)
}

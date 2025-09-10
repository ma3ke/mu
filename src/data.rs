use std::{collections::HashMap, str::FromStr};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::{ActiveUser, CpuUsage, Machine, Owner};

/// The structure stored in `machine_usage.dat`
#[derive(Debug)]
pub struct Data {
    pub timestamp: std::time::SystemTime,
    pub info: Info,
}

impl Data {
    /// Reads the complete contents of `machine_usage.dat`.
    pub fn parse(s: &str) -> Result<Self> {
        // Read the first line to get the timestamp.
        let Some((first, rest)) = s.split_once('\n') else {
            bail!("expected at least two lines: a timestamp followed by the machines info");
        };

        let timestamp = first.parse::<u64>().context("could not parse timestamp")?;
        let timestamp =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp);

        let info = serde_json::from_str(rest).context("could not parse info")?;

        Ok(Data { timestamp, info })
    }

    pub fn machines(&self) -> Vec<Machine> {
        self.info
            .0
            .iter()
            .map(|entry| {
                let InfoEntry {
                    hostname,
                    owner,
                    room,
                    cpu_usage,
                    usage,
                } = entry.clone();

                let ignore_users = ["sshuser", "root"]; // TODO: Reconsider and make configurable.
                let active_user = usage
                    .iter()
                    .max_by_key(|(_, cores)| {
                        cores.iter().map(|cu| cu.percentage as u64).sum::<u64>()
                    })
                    .map(|(user, cu)| ActiveUser {
                        user: user.to_string(),
                        cores: cu.len() as u32,
                        task: cu
                            .iter()
                            .max_by_key(|cu| cu.percentage as u64)
                            .map(|cu| cu.process_name.to_string())
                            .unwrap_or("?".to_string()),
                    })
                    .filter(|au| !ignore_users.contains(&au.user.as_str()));

                Machine {
                    hostname,
                    owner,
                    room,
                    cpu_usage,
                    active_user,
                }
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "RawInfo")]
pub struct Info(pub Vec<InfoEntry>);

#[derive(Debug, Clone)]
pub struct InfoEntry {
    pub hostname: String,
    pub owner: Owner,
    pub room: String,
    pub cpu_usage: CpuUsage,
    pub usage: CoreUsagePerUser,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CoreUsage {
    percentage: f32,
    load_average: f32,
    process_name: String,
}

type CoreUsagePerUser = HashMap<String, Vec<CoreUsage>>;

impl TryFrom<RawInfo> for Info {
    type Error = anyhow::Error;

    fn try_from(RawInfo(raw_info): RawInfo) -> std::result::Result<Self, Self::Error> {
        let entries = raw_info
            .into_iter()
            .map(|(key, inner)| InfoEntry {
                hostname: key,
                owner: Owner::from_str(&inner.owner).unwrap(), // Cannot fail.
                room: inner.room,
                cpu_usage: CpuUsage {
                    total: inner.cores_total,
                    used: inner.cores_used,
                },
                usage: inner.usage,
            })
            .collect();
        Ok(Self(entries))
    }
}

#[derive(Debug, Deserialize)]
pub struct RawInfo(HashMap<String, RawInnerInfo>);

#[derive(Debug, Deserialize)]
struct RawInnerInfo {
    idk: bool,
    room: String,
    cores_total: u32,
    owner: String,
    user_to_list_of_idk: HashMap<String, Vec<String>>,
    cores_used: u32,
    list_of_four_numbers_idk_1: [i64; 4],
    list_of_four_numbers_idk_2: [i64; 4],
    usage: CoreUsagePerUser,
}

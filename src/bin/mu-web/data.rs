use std::{collections::HashMap, str::FromStr};

use mu::info::PROCESS_USAGE_THRESHOLD_PERCENT;

use crate::{ActiveUser, CpuUsage, Machine, Owner};

pub trait DataView {
    /// Return a sorted list of [`Machine`]s.
    fn machines(&self) -> Box<[Machine]>;
    fn total_usage(&self) -> f64;
    /// Return a list of tuples with the username and its associated number of threads across
    /// machines.
    fn tpu(&self) -> Vec<(&String, usize)>;
    fn cpu_count(&self) -> u32;
}

impl DataView for mu::info::Data {
    // pub fn info(&self) -> &[InfoEntry] {
    //     &self.0.info
    // }

    fn machines(&self) -> Box<[Machine]> {
        let mut ms = self
            .info
            .iter()
            .map(|entry| {
                let active_user = entry
                    .info
                    .usage
                    .iter()
                    .max_by_key(|(_, cores)| cores.iter().map(|cu| cu.usage as u64).sum::<u64>())
                    .map(|(user, cu)| ActiveUser {
                        user: user.to_string(),
                        cores: cu.len() as u32,
                        task: cu
                            .iter()
                            .max_by_key(|cu| cu.usage as u64)
                            .map(|cu| cu.name.to_string())
                            .unwrap_or("?".to_string()),
                    });

                let cpu_usage = CpuUsage {
                    used: entry.info.load_avg.one.round() as u32,
                    total: entry.info.cpus.len() as u32,
                };

                let t = entry.info.load_avg.five / cpu_usage.total as f64;
                const N_COLORS: usize = 10; // Magic. Cf. style.css gradient colors.
                let hotness = ((t * (N_COLORS - 1) as f64) as usize).clamp(0, N_COLORS - 1) as u32;

                let owner =
                    Owner::from_str(entry.note.clone().unwrap_or_default().as_str()).unwrap(); // TODO: UGH
                let owner_mark = match owner {
                    Owner::Member(_) => "",
                    Owner::Visitor(_) => "v",
                    Owner::Student(_) => "s",
                    Owner::Reserve => "",
                    Owner::None => "",
                }
                .to_string();
                let owner = match owner {
                    Owner::Member(name) | Owner::Visitor(name) | Owner::Student(name) => name,
                    Owner::Reserve => "Reservation required".to_string(),
                    Owner::None => String::default(),
                };

                Machine {
                    hostname: entry.info.hostname.clone(),
                    hotness,
                    owner,
                    owner_mark,
                    room: entry.room.clone(),
                    cpu_usage,
                    load_avg: entry.info.load_avg.clone(),
                    active_user,
                }
            })
            .collect::<Vec<_>>();

        ms.sort_by_cached_key(|m| m.hostname.clone());
        ms.into_boxed_slice()
    }

    fn total_usage(&self) -> f64 {
        let info = &self.info;
        let total_cores_used: f64 = info
            .iter()
            .map(|entry| entry.info.cpus.iter().sum::<f32>() as f64)
            .sum();
        let total_cores: f64 = info
            .iter()
            .map(|entry| entry.info.cpus.len() as f64 * 100.0)
            .sum();
        total_cores_used / total_cores
    }

    /// Return a list of tuples with the username and its associated number of threads across
    /// machines.
    fn tpu(&self) -> Vec<(&String, usize)> {
        // TODO: Also rewrite this this sucks.
        let mut tpu = HashMap::<_, usize>::new();
        for entry in &self.info {
            for (user, cu) in &entry.info.usage {
                // TODO: I think this is a cursed way of counting total usage.
                *tpu.entry(user).or_default() += cu.len();
            }
        }

        let mut tpu: Vec<(&String, usize)> = tpu.into_iter().collect();
        // We sort by the number of threads, followed by the name to deteministically break ties.
        tpu.sort_by_key(|(name, tasks_sum)| (*tasks_sum, *name));
        tpu
    }

    fn cpu_count(&self) -> u32 {
        let mut cpu_count = 0;
        for entry in &self.info {
            cpu_count += entry.info.cpus.len();
        }
        cpu_count as u32
    }
}

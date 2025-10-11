use std::str::FromStr;

use crate::{ActiveUser, CpuUsage, Machine, Owner};

pub trait DataView {
    /// Return a sorted list of [`Machine`]s.
    fn machines(&self) -> Box<[Machine]>;
}

impl DataView for mu::model::Data {
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
}

use std::str::FromStr;

use anyhow::{Context, Result};
use sysinfo::System;

use mu::{config::Config, info::Info};

const DEFAULT_CONFIG_PATH: &str = "/martini/sshuser/mu/ignore.linus";

fn main() -> Result<()> {
    let config_path = std::env::args()
        .skip(1)
        .next()
        .unwrap_or(DEFAULT_CONFIG_PATH.to_string());
    let config = match std::fs::read_to_string(&config_path) {
        Ok(s) => Some(
            Config::from_str(&s).context(format!("could not parse config file {config_path:?}"))?,
        ),
        Err(_) => None,
    }
    .unwrap_or_default();

    let mut system = System::new_with_specifics(
        sysinfo::RefreshKind::nothing()
            .with_cpu(sysinfo::CpuRefreshKind::everything())
            .with_memory(sysinfo::MemoryRefreshKind::everything())
            .with_processes(
                sysinfo::ProcessRefreshKind::nothing()
                    .with_cpu()
                    .with_user(sysinfo::UpdateKind::OnlyIfNotSet),
            ),
    );

    // We need to wait until we have enough cpu sampling.
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_all(); // TODO: Consider being more surgical in what we update at this point.

    // Read the system state.
    let info = Info::new(&system, config);

    // Send the serialized system info over stdout.
    let stdout = std::io::stdout().lock();
    // TODO: Consider writing out some sort of version information first.
    // TODO: Remove the pretty printing once this works.
    serde_json::to_writer_pretty(stdout, &info)?;
    Ok(())
}

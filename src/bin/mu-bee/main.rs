use anyhow::Result;
use sysinfo::System;

use mu::info::Info;

fn main() -> Result<()> {
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
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    system.refresh_all(); // TODO: Consider being more surgical in what we update at this point.

    // Read the system state.
    let info = Info::new(&system);

    // Send the serialized system info over stdout.
    let stdout = std::io::stdout().lock();
    // TODO: Consider writing out some sort of version information first.
    // TODO: Remove the pretty printing once this works.
    serde_json::to_writer_pretty(stdout, &info)?;
    Ok(())
}

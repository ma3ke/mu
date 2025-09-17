use anyhow::Result;
use sysinfo::System;

pub mod info;

#[derive(Debug, Clone)]
pub struct HostInfo {
    pub hostname: String,
    pub user: String,
    pub os: String,
    pub os_version: String,
}

impl HostInfo {
    pub fn new() -> Result<Self> {
        Ok(Self {
            hostname: hostname::get()?.to_str().unwrap_or("?").to_string(),
            user: users::get_current_username()
                .map(|u| u.to_string_lossy().to_string())
                .unwrap_or("?".to_string()),
            os: System::name().unwrap_or("?".to_string()),
            os_version: System::os_version().unwrap_or("?".to_string()),
        })
    }
}

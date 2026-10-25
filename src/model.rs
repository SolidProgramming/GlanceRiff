use serde::Serialize;

/// One point-in-time view of the system, shared by the TUI and the web API.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Snapshot {
    pub timestamp: u64,
    pub host: HostInfo,
    pub cpu: CpuInfo,
    pub mem: MemInfo,
    pub swap: MemInfo,
    pub disks: Vec<DiskInfo>,
    pub nets: Vec<NetInfo>,
    pub processes: Vec<ProcessInfo>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct HostInfo {
    pub hostname: String,
    pub os: String,
    pub uptime: u64,
    /// `None` on platforms without a load average (Windows).
    pub load_avg: Option<[f64; 3]>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct CpuInfo {
    pub total: f32,
    pub cores: Vec<f32>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MemInfo {
    pub total: u64,
    pub used: u64,
    pub percent: f32,
}

impl MemInfo {
    pub fn new(total: u64, used: u64) -> Self {
        let percent = if total == 0 { 0.0 } else { used as f32 / total as f32 * 100.0 };
        Self { total, used, percent }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub mount: String,
    pub fs: String,
    pub total: u64,
    pub used: u64,
    pub percent: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetInfo {
    pub name: String,
    /// Bytes per second.
    pub rx_rate: u64,
    pub tx_rate: u64,
    pub rx_total: u64,
    pub tx_total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    /// Can exceed 100 on multi-core machines (same as glances' default).
    pub cpu: f32,
    pub mem: u64,
    pub mem_percent: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mem_percent_handles_zero_total() {
        assert_eq!(MemInfo::new(0, 0).percent, 0.0);
        assert_eq!(MemInfo::new(200, 50).percent, 25.0);
    }

    #[test]
    fn snapshot_serializes() {
        let json = serde_json::to_value(Snapshot::default()).unwrap();
        assert!(json.get("cpu").is_some());
        assert!(json["host"]["load_avg"].is_null());
    }
}

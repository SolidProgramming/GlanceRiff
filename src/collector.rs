use std::{
    collections::HashSet,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use sysinfo::{
    Components, Disks, Networks, ProcessesToUpdate, System, ThreadKind,
    MINIMUM_CPU_UPDATE_INTERVAL,
};

use crate::model::{
    CpuInfo, DiskInfo, DiskIo, HostInfo, MemInfo, NetInfo, ProcessInfo, SensorInfo, Snapshot,
};

/// Cumulative CPU time counters from the `cpu` line of `/proc/stat`, in jiffies.
#[derive(Debug, Clone, Copy, PartialEq)]
struct CpuTimes {
    iowait: u64,
    total: u64,
}

impl CpuTimes {
    /// I/O wait share of the time between `prev` and `self`, in percent.
    fn iowait_since(self, prev: CpuTimes) -> f32 {
        let total = self.total.saturating_sub(prev.total);
        if total == 0 {
            return 0.0;
        }
        self.iowait.saturating_sub(prev.iowait) as f32 / total as f32 * 100.0
    }
}

/// Parses the aggregate `cpu  user nice system idle iowait irq softirq steal ...` line.
fn parse_cpu_times(stat: &str) -> Option<CpuTimes> {
    let line = stat.lines().find(|l| l.starts_with("cpu "))?;
    let fields: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .take(8) // guest and guest_nice are already counted in user and nice.
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    (fields.len() >= 5).then(|| CpuTimes { iowait: fields[4], total: fields.iter().sum() })
}

/// Linux only: other platforms have no I/O wait counter.
fn read_cpu_times() -> Option<CpuTimes> {
    if cfg!(target_os = "linux") {
        parse_cpu_times(&std::fs::read_to_string("/proc/stat").ok()?)
    } else {
        None
    }
}

pub struct Collector {
    sys: System,
    last_cpu_times: Option<CpuTimes>,
    disks: Disks,
    nets: Networks,
    components: Components,
    last_refresh: Instant,
    hostname: String,
    os: String,
}

impl Collector {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        // CPU usage is a delta between two refreshes, so prime it once.
        std::thread::sleep(MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_cpu_usage();
        sys.refresh_processes(ProcessesToUpdate::All, true);

        Self {
            sys,
            last_cpu_times: read_cpu_times(),
            disks: Disks::new_with_refreshed_list(),
            nets: Networks::new_with_refreshed_list(),
            components: Components::new_with_refreshed_list(),
            last_refresh: Instant::now(),
            hostname: System::host_name().unwrap_or_else(|| "unknown".into()),
            os: System::long_os_version().unwrap_or_else(|| "unknown".into()),
        }
    }

    /// Refreshes all sources and returns a fresh snapshot. Blocking.
    pub fn collect(&mut self) -> Snapshot {
        let elapsed = self.last_refresh.elapsed().as_secs_f64().max(0.001);
        self.last_refresh = Instant::now();

        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.sys.refresh_processes(ProcessesToUpdate::All, true);
        self.disks.refresh(true);
        self.nets.refresh(true);
        self.components.refresh(true);

        let cpu_times = read_cpu_times();
        let iowait = cpu_times.zip(self.last_cpu_times).map(|(now, prev)| now.iowait_since(prev));
        self.last_cpu_times = cpu_times;

        let total_mem = self.sys.total_memory();
        let load = System::load_average();
        let load_avg = (!cfg!(windows)).then_some([load.one, load.five, load.fifteen]);

        let mut nets: Vec<NetInfo> = self
            .nets
            .list()
            .iter()
            .map(|(name, data)| NetInfo {
                name: name.clone(),
                rx_rate: (data.received() as f64 / elapsed) as u64,
                tx_rate: (data.transmitted() as f64 / elapsed) as u64,
                rx_total: data.total_received(),
                tx_total: data.total_transmitted(),
            })
            .collect();
        nets.sort_by(|a, b| a.name.cmp(&b.name));

        let rate = |bytes: u64| (bytes as f64 / elapsed) as u64;
        let mut disks = Vec::new();
        let mut disk_io = DiskIo::default();
        // Bind mounts (common in containers) repeat a device; count its I/O once.
        let mut seen_devices = HashSet::new();
        for d in self.disks.list() {
            let total = d.total_space();
            let used = total.saturating_sub(d.available_space());
            let usage = d.usage();
            let (read_rate, write_rate) = (rate(usage.read_bytes), rate(usage.written_bytes));
            let device = d.name().to_string_lossy().into_owned();
            if seen_devices.insert(device.clone()) {
                disk_io.read_rate = disk_io.read_rate.saturating_add(read_rate);
                disk_io.write_rate = disk_io.write_rate.saturating_add(write_rate);
            }
            disks.push(DiskInfo {
                mount: d.mount_point().to_string_lossy().into_owned(),
                fs: d.file_system().to_string_lossy().into_owned(),
                device,
                total,
                used,
                percent: MemInfo::new(total, used).percent,
                read_rate,
                write_rate,
            });
        }

        // Often empty on Windows, where sysinfo needs WMI access for sensors.
        let mut sensors: Vec<SensorInfo> = self
            .components
            .list()
            .iter()
            .filter_map(|c| {
                Some(SensorInfo {
                    label: c.label().to_string(),
                    temp: c.temperature().filter(|t| t.is_finite())?,
                    max: c.max().filter(|t| t.is_finite()),
                    critical: c.critical().filter(|t| t.is_finite() && *t > 0.0),
                })
            })
            .collect();
        sensors.sort_by(|a, b| a.label.cmp(&b.label));

        let mut processes: Vec<ProcessInfo> = self
            .sys
            .processes()
            .values()
            // On Linux, sysinfo also lists each process's threads; keep processes only.
            .filter(|p| p.thread_kind() != Some(ThreadKind::Userland))
            .map(|p| ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                cpu: p.cpu_usage(),
                mem: p.memory(),
                mem_percent: MemInfo::new(total_mem, p.memory()).percent,
            })
            .collect();
        processes.sort_by(|a, b| b.cpu.total_cmp(&a.cpu));

        Snapshot {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or_default(),
            host: HostInfo {
                hostname: self.hostname.clone(),
                os: self.os.clone(),
                uptime: System::uptime(),
                load_avg,
            },
            cpu: CpuInfo {
                total: self.sys.global_cpu_usage(),
                cores: self.sys.cpus().iter().map(|c| c.cpu_usage()).collect(),
                iowait,
            },
            mem: MemInfo::new(total_mem, self.sys.used_memory()),
            swap: MemInfo::new(self.sys.total_swap(), self.sys.used_swap()),
            disks,
            disk_io,
            nets,
            sensors,
            processes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STAT: &str = "cpu  4705 356 584 3699176 23060 0 277 0 10 20
cpu0 1393 280 284 925079 6064 0 254 0 0 0
intr 114930548 113199788 3 0 5 263 0 4 [...]
";

    #[test]
    fn parses_aggregate_line() {
        let t = parse_cpu_times(STAT).unwrap();
        assert_eq!(t.iowait, 23060);
        assert_eq!(t.total, 4705 + 356 + 584 + 3699176 + 23060 + 277);
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_cpu_times(""), None);
        assert_eq!(parse_cpu_times("cpu  1 2 x 4 5"), None);
        assert_eq!(parse_cpu_times("cpu  1 2 3"), None);
    }

    #[test]
    fn iowait_percent() {
        let prev = CpuTimes { iowait: 100, total: 1000 };
        assert_eq!(CpuTimes { iowait: 150, total: 1500 }.iowait_since(prev), 10.0);
        assert_eq!(prev.iowait_since(prev), 0.0);
    }
}

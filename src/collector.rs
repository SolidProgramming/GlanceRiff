use std::time::{Instant, SystemTime, UNIX_EPOCH};

use sysinfo::{
    Disks, Networks, ProcessesToUpdate, System, ThreadKind, MINIMUM_CPU_UPDATE_INTERVAL,
};

use crate::model::{CpuInfo, DiskInfo, HostInfo, MemInfo, NetInfo, ProcessInfo, Snapshot};

pub struct Collector {
    sys: System,
    disks: Disks,
    nets: Networks,
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
            disks: Disks::new_with_refreshed_list(),
            nets: Networks::new_with_refreshed_list(),
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

        let disks = self
            .disks
            .list()
            .iter()
            .map(|d| {
                let total = d.total_space();
                let used = total.saturating_sub(d.available_space());
                let m = MemInfo::new(total, used);
                DiskInfo {
                    mount: d.mount_point().to_string_lossy().into_owned(),
                    fs: d.file_system().to_string_lossy().into_owned(),
                    total,
                    used,
                    percent: m.percent,
                }
            })
            .collect();

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
            },
            mem: MemInfo::new(total_mem, self.sys.used_memory()),
            swap: MemInfo::new(self.sys.total_swap(), self.sys.used_swap()),
            disks,
            nets,
            processes,
        }
    }
}

# GlanceRiff

A lightweight, [glances](https://github.com/nicolargo/glances)-style system monitor written in Rust. Run it as a terminal UI, or serve a web dashboard and JSON API from one self-contained binary.

It shows:

- CPU: total and per-core usage
- Memory and swap
- Disks: mount point, file system, usage
- Network: per-interface rx/tx rates and totals
- Processes: sortable by CPU, memory or name
- Host: hostname, OS, uptime, load average (not available on Windows)

## Installation

Requires Rust 1.88 or newer.

```sh
git clone https://github.com/SolidProgramming/GlanceRiff.git
cd GlanceRiff
cargo build --release
```

The binary is at `target/release/glanceriff` (`glanceriff.exe` on Windows). Or install it straight into your Cargo bin directory:

```sh
cargo install --path .
```

## Usage

```sh
glanceriff                        # terminal UI
glanceriff --web                  # web dashboard on http://127.0.0.1:61208
glanceriff --web -b 0.0.0.0:61208 # make the dashboard reachable from the network
glanceriff -t 1 -n                # 1 s refresh, machine-normalized process CPU%
```

| Option | Default | Description |
| --- | --- | --- |
| `-w`, `--web` | off | Serve the web dashboard and JSON API instead of the terminal UI |
| `-b`, `--bind <ADDR>` | `127.0.0.1:61208` | Address for the web server |
| `-t`, `--interval <SECS>` | `2.0` | Refresh interval in seconds (minimum 0.5) |
| `-n`, `--normalize-cpu` | off | Show process CPU% relative to the whole machine (max 100%) instead of per core (up to 100% × cores) |

> **Note:** The web server has no authentication. If you bind to a non-loopback address, anyone who can reach the port can see your system and process information.

### Terminal UI keys

| Key | Action |
| --- | --- |
| `c` / `m` / `p` | Sort processes by CPU / memory / name |
| `n` | Toggle normalized process CPU% |
| `Space` | Pause or resume refreshing |
| `+` / `-` | Increase / decrease the refresh interval by 0.5 s |
| `q` / `Esc` | Quit |

## JSON API

With `--web`, the following endpoints return JSON:

| Endpoint | Content |
| --- | --- |
| `/api/snapshot` | Everything below in one object |
| `/api/host` | Hostname, OS, uptime, load average |
| `/api/cpu` | Total and per-core usage |
| `/api/mem` | Memory and swap |
| `/api/disks` | Disks |
| `/api/net` | Network interfaces (rates in bytes/s) |
| `/api/processes` | Processes, sorted by CPU |
| `/api/config` | Server settings (`normalize_cpu`) |

Data is collected in the background at the configured interval, so requests are cheap and return the latest snapshot.

```sh
curl -s http://127.0.0.1:61208/api/cpu
```

## Development

```sh
cargo run              # terminal UI
cargo run -- --web     # web dashboard
cargo test
```

The web frontend is a single file, [`static/index.html`](static/index.html), embedded into the binary at compile time.

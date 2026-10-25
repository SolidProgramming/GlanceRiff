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

## Container (Docker, Podman, Unraid)

A prebuilt image with the web dashboard is published to `ghcr.io/solidprogramming/glanceriff`:

| Tag | Source |
| --- | --- |
| `latest`, `X.Y.Z`, `X.Y` | Release tags (`vX.Y.Z`) |
| `edge` | Latest commit on `master` |

A container normally only sees its own processes and network. To monitor the host, share its PID, network and UTS namespaces:

```sh
podman run -d --name glanceriff --pid=host --network=host --uts=host --init \
  ghcr.io/solidprogramming/glanceriff:latest
```

The same command works with `docker`. The dashboard is then at `http://<host>:61208`. To pass different options, append them after the image name, including `--web`:

```sh
podman run ... ghcr.io/solidprogramming/glanceriff:latest --web --bind 0.0.0.0:61208 --interval 1
```

With Compose (`docker compose` or `podman-compose`):

```sh
docker compose up -d          # uses the prebuilt image
docker compose up -d --build  # builds the image locally
```

Disks only show up if they are mounted into the container, for example `-v /mnt/user:/mnt/user:ro`.

### Unraid

1. Download the [template](unraid/glanceriff.xml) to `/boot/config/plugins/dockerMan/templates-user/my-glanceriff.xml` on your server:

   ```sh
   wget -O /boot/config/plugins/dockerMan/templates-user/my-glanceriff.xml \
     https://raw.githubusercontent.com/SolidProgramming/GlanceRiff/master/unraid/glanceriff.xml
   ```

2. In the Unraid web UI, go to **Docker → Add Container** and select **GlanceRiff** under *Template*.
3. Click **Apply**. The web UI opens from the container's context menu.

The template uses host networking with `--pid=host --uts=host --init` and mounts `/mnt/user` read-only.

## Development

```sh
cargo run              # terminal UI
cargo run -- --web     # web dashboard
cargo test
```

The web frontend is a single file, [`static/index.html`](static/index.html), embedded into the binary at compile time.

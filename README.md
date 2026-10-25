# GlanceRiff [![CodeQL](https://github.com/SolidProgramming/GlanceRiff/actions/workflows/github-code-scanning/codeql/badge.svg)](https://github.com/SolidProgramming/GlanceRiff/actions/workflows/github-code-scanning/codeql) [![Container image](https://github.com/SolidProgramming/GlanceRiff/actions/workflows/container.yml/badge.svg)](https://github.com/SolidProgramming/GlanceRiff/actions/workflows/container.yml)

A lightweight, [glances](https://github.com/nicolargo/glances)-style system monitor written in Rust. Run it as a terminal UI, or serve a web dashboard and JSON API from one self-contained binary.

<img width="1920" height="1080" alt="brave_eJwjhRbPZl" src="https://github.com/user-attachments/assets/5ea911e1-159e-4870-9591-fbcd69bbc246" />
<img width="1655" height="936" alt="brave_0DdCSqqDti" src="https://github.com/user-attachments/assets/272c1776-b146-486e-a663-b58884e7f53a" />


It shows:

- CPU: total and per-core usage
- Memory and swap
- Disks: mount point, file system, usage
- Network: per-interface rx/tx rates and totals
- Temperatures: hardware sensors with their critical points (usually not available on Windows)
- History: graphs of CPU, memory and network over the last 10 minutes
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

The history covers 10 minutes at the interval the program was started with.

### Authentication

Credentials are set through environment variables only, so they don't show up in the process list:

| Variable | Description |
| --- | --- |
| `GLANCERIFF_USERNAME`, `GLANCERIFF_PASSWORD` | Require HTTP Basic auth. Set both, or neither |
| `GLANCERIFF_TOKEN` | Accept `Authorization: Bearer <TOKEN>`, for scripts and monitoring tools |

Without them, the web server is open. If you bind to a non-loopback address, anyone who can reach the port can see your system and process information.

With a user name and password, the browser asks for them, and every page and API endpoint requires them. If a token is also set, either one is accepted. An empty variable counts as unset, so a blank container field means no login.

```sh
GLANCERIFF_USERNAME=admin GLANCERIFF_PASSWORD=secret glanceriff --web -b 0.0.0.0:61208

curl -u admin:secret http://127.0.0.1:61208/api/cpu
curl -H "Authorization: Bearer my-token" http://127.0.0.1:61208/api/cpu
```

Basic auth sends the password unencrypted. Over an untrusted network, put GlanceRiff behind a reverse proxy with HTTPS.

### Terminal UI keys

| Key | Action |
| --- | --- |
| `c` / `m` / `p` | Sort processes by CPU / memory / name |
| `n` | Toggle normalized process CPU% |
| `g` | Show or hide the history graphs |
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
| `/api/sensors` | Temperature sensors in °C (`temp`, `max`, `critical`) |
| `/api/processes` | Processes, sorted by CPU |
| `/api/history` | CPU%, memory% and total network rates per sample, oldest first |
| `/api/config` | Server settings (`normalize_cpu`, `interval_ms`, `history_len`) |

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
podman run -d --name glanceriff --pid=host --network=host --uts=host \
  -v /etc/os-release:/etc/os-release:ro \
  ghcr.io/solidprogramming/glanceriff:latest
```

The `os-release` mount is optional; without it the dashboard shows the container's OS (Debian) instead of the host's. The same command works with `docker`. The dashboard is then at `http://<host>:61208`. To pass different options, append them after the image name, including `--web`:

```sh
podman run ... ghcr.io/solidprogramming/glanceriff:latest --web --bind 0.0.0.0:61208 --interval 1
```

With Compose (`docker compose` or `podman-compose`):

```sh
docker compose up -d          # uses the prebuilt image
docker compose up -d --build  # builds the image locally
```

Disks only show up if they are mounted into the container, for example `-v /mnt/user:/mnt/user:ro`.

To require a login, set `GLANCERIFF_USERNAME` and `GLANCERIFF_PASSWORD`, for example `-e GLANCERIFF_USERNAME=admin -e GLANCERIFF_PASSWORD=secret`. `GLANCERIFF_TOKEN` sets a token for scripts. See [Authentication](#authentication).

### Terminal UI in the container

Open the terminal UI inside the running web container:

```sh
podman exec -it glanceriff glanceriff
```

Or run it on its own, without the web server. Any arguments after the image name replace the default `--web`:

```sh
podman run -it --rm --pid=host --network=host --uts=host \
  ghcr.io/solidprogramming/glanceriff:latest --interval 2
```

### Unraid

1. Download the [template](unraid/glanceriff.xml) to `/boot/config/plugins/dockerMan/templates-user/my-glanceriff.xml` on your server:

   ```sh
   wget -O /boot/config/plugins/dockerMan/templates-user/my-glanceriff.xml \
     https://raw.githubusercontent.com/SolidProgramming/GlanceRiff/master/unraid/glanceriff.xml
   ```

2. In the Unraid web UI, go to **Docker → Add Container** and select **GlanceRiff** under *Template*.
3. Click **Apply**. The web UI opens from the container's context menu.

The template uses host networking with `--pid=host --uts=host` and mounts `/etc/os-release` and `/mnt/user` read-only. To protect the dashboard, fill in *Username* and *Password*. The *API token* field is under *Show more settings*.

For the terminal UI, open the Unraid terminal (the `>_` icon in the top bar) and run `docker exec -it GlanceRiff glanceriff`. The *Console* entry in the container menu doesn't work, because the image contains no shell.

## Development

```sh
cargo run              # terminal UI
cargo run -- --web     # web dashboard
cargo test
```

The web frontend is a single file, [`static/index.html`](static/index.html), embedded into the binary at compile time.

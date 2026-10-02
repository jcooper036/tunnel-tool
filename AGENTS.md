# Tunnel Tool

Tunnel tool is for managing tunnels to other services. For example, cloud sql proxies, or kubectl tunnels, ssh tunnels, etc. 

If you have a connection that you want to keep alive, this is the tool

A tunnel is an outbound connection from this machine to a remote service, exposed on a local port. Clients connect to `localhost:<port>`, and `tun status` is the lookup for which port reaches which service

It handles things like de-duping ports so that starting / stopping multiple tunnels 

The problem this solves:
- I run a bunch of cloudsql proxies, and kubectl, and I might want to ssh to different machines
- I'd like to evolve beyond just having a tmux session to manage each

## Scope
- yes :check: connect to services not on your machine
- no :x: manage services that you are hosting

## CMD interface
- `tun` (prefix for the tool)
    - running `tun` alone is equivalent to `tun --help`
- `--help, -h, help` displays commands
    - there should be a `-h` and `--help` at every command level too
- `update` rebuilds and reinstalls `tun` from the source checkout it was built from (`git pull --ff-only` first when a remote exists)
- `start` to start tunnels
    - `-a, --all` starts all that are marked for auto run on start
    - `<tunnel-name>` starts a specific tunnel
    - `-p, --port` specify a port for a tunnel (different than the default) 
- `stop` to stop tunnels
    - `-a, --all` stops all
    - `<tunnel-name>` stops specific tunnel by name
    - `<session-id>` stops a specific tunnel by session id
- `status` displays all current running tunnels in a table
    - `--json`, `--all` (include stopped sessions), `-q, --quiet` (exit 0 only if at least one tunnel is running and all are up)
    - their id, name, address/resource, port, liveliness
- `registry` lists registered tunnel specs: name, port range, auto-run, running address, command. `--json`
- `create` create a tunnel (see spec)
    - missing name, `--command`, or `--port-range` starts an interactive prompt (terminal only)
- `heal` attempts to restart any dead tunnels, without starting any new ones
- `restart <tunnel-name|session-id>` stops and relaunches a tunnel, preferring its previous port
- `remove <tunnel-name>` deletes a spec. Refuses while running unless `-f, --force`
- `log <tunnel-name|session-id>` prints the newest session's log (stopped sessions included)
    - `-n N` last N lines (default 100), `-f` follows
- `show <tunnel-name|session-id>` one-call diagnosis: spec, latest session, live health check, log tail. `--json`
- `port <tunnel-name>` prints only the running port. Non-zero exit if not running
- `ensure <tunnel-name>` starts or heals as needed, waits until up (and healthy), prints only the port. `--all` for auto-run tunnels (`name<TAB>port` lines). `--timeout`
- `check <tunnel-name>` runs the spec's health_check. Exit 0 healthy, 1 not
- `doctor` checks if tunnels can be created (basically make a quick version, shut them down). Runs the health_check when set. `--json`


### tunnel spec
Stored as a .toml file (easier to edit than the database)
- name: must be a unique name from other tunnels
- access_command: command the tunnel runs. Must contain a `{port}` placeholder, which the tool fills with the assigned port
- preferred_port_range: `[start, end]`, first free port in the range is used. `-p` overrides it
    - avoid standard ports (5432, 8000, 3000, ...) so tunnels stay clear of test services and default configurations. Use a non-standard range, and let `tun status` be the source of truth for where a service is
- health_check: optional http(s) URL containing `{port}`. A 2xx response means healthy. An alive tunnel whose health check fails shows as `unhealthy`
- auto_run_on_start : bool if `tun start -a` would start this tunnel


## database schema
- table for sessions
    - a session is an instance of a running tunnel
    - id (uuid7), tunnel_name, created_at, port, process_id, command, log_path, stopped_at
    - active sessions are rows with `stopped_at` null. `log_path` is where the tunnel's stdout / stderr goes
- migrations are named `<YYMMDDHH>_<title>` and tracked in `migrations`

## technical layout
- no server. CLI commands read and write SQLite directly and spawn detached tunnel processes
- `process`: spawns, signals, and checks liveness of a tunnel process (own process group)
- `db`: SQLite state (sessions)
- `spec`: tunnel .toml specs
- `ports`: free port selection
- liveness: process alive and the port accepts a TCP connection. `up`, `unhealthy` (up but health_check fails), `unreachable` (alive, port closed), `dead`
- state lives in `~/.tun` (`tunnels/`, `logs/`, `tun.db`), overridable with `--home` / `TUN_HOME`

## tech stack
- Rust for all code
- local SQLite database for session state

## agent use
- stdout is data, diagnostics go to stderr, exit codes are meaningful
- `PORT=$(tun ensure <name>)` is the one-call way to get a usable tunnel
- when something is wrong: `tun show <name>`, then `tun log <name>`

## random behavior notes
- if I did `tun start -a`, it should bring up any default tunnels that aren't already running


# Tunnel Tool

`tun` keeps tunnels to remote services alive and observable. Cloud SQL proxies, `kubectl port-forward`, ssh tunnels: anything that exposes a remote service on a local port.

It replaces a tmux pane per tunnel. Each tunnel is registered once, started by name, assigned a free port, and checked for liveness.

## Install

```bash
cd <path-to-tunnel-tool-clone>
cargo install --path .
```

If you haven't used cargo and/or rust before, add this to your shell profile (ex .bashrc  or .zshrc) 
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

`tun` installs to `~/.cargo/bin`. `tun update` rebuilds and reinstalls from the same checkout, pulling first when the checkout has a remote. It installs whatever is checked out there, so keep that checkout on the branch you want.

Run `tun --help` for the commands. Every subcommand has its own `--help`.

## Concepts

### A tunnel is an outbound connection exposed on a local port

Clients connect to `localhost:<port>`. `tun status` is the lookup for which port reaches which service.

### Specs live in `~/.tun/tunnels/`

A spec is a `.toml` file: a name, the command that opens the tunnel, a preferred port range, whether it auto-runs, and an optional health check. The command contains a `{port}` placeholder that `tun` fills in. `tun registry` lists them, and `tun create` writes one.

### Ports are assigned, not fixed

`tun` picks the first free port in the spec's range. Use ports outside the standard ones (5432, 8000, 3000) so tunnels stay clear of test services and default configurations. Anything that needs the port asks `tun` for it.

### A session is one running instance of a tunnel

Sessions are rows in `~/.tun/tun.db`. Each session's output goes to `~/.tun/logs/<session-id>.log`. Stopped sessions stay in the database, so the log of a tunnel that died is still readable.

### Liveness has four states

`up`: the process is alive, the port accepts connections, and the health check (if any) passes. `unhealthy`: up, but the health check fails. `unreachable`: the process is alive and the port is closed. `dead`: the process is gone.

## Scope

`tun` connects to services that are not on your machine. It does not manage services you host.

## Output for scripts and agents

Stdout is data, diagnostics go to stderr, and exit codes are meaningful. Commands that report state take `--json`.

## State location

`~/.tun` by default. Override with `--home` or `TUN_HOME`.

## Development

```bash
cargo test
```

Design notes are in `AGENTS.md`. Research on which tunnels to register is in `docs/`.

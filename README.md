# Tunnel Tool

`tun` keeps tunnels to remote services alive and observable. Cloud SQL proxies, `kubectl port-forward`, ssh tunnels: anything that exposes a remote service on a local port.

It replaces a tmux pane per tunnel. Each tunnel is registered once, started by name, assigned a free port, and checked for liveness.

## Install

```bash
git clone git@github.com:jcooper036/tunnel-tool.git
cd tunnel-tool
cargo install --path .
```

If you haven't used cargo and/or rust before, add this to your shell profile (ex .bashrc  or .zshrc) 
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```
`tun` installs to `~/.cargo/bin`
Run `tun --help` for the commands. Every subcommand has its own `--help`.

### Installing for agents
Part of the usefullness of tunnel tool is that it lets you designate tunnels that your agents are ok to run.

You can use whatever agent skill manager you'd like. I personally like to symlink the skill to my global agents skill folder, e.x.

```
ln -s ~/<path-to-tunnel-tool-root>/.agents/skills/tunnel-tool ~/<path-to-agent-config>/skills/tunnel-tool
```
That way, you pick up skill updates with `tun update`.

### updating
```bash
tun update
```
This pulls from remote and re-installs at the same place as your checkout of this repo

## Security

`tun` runs shell commands from spec files and exposes remote services on local ports. Misused, it can expose internal services or move data off this machine.

- Specs are arbitrary commands. Only register specs you have read. Agents with `tun create` authority can register any command
- `tun` does not authenticate local connections. Any local process can reach a tunnel port
- Tunnels keep running after the starting session ends. Check `tun status`
- Logs in `~/.tun/logs` may contain credentials from the tunnel command
- Keep access commands bound to `127.0.0.1`

Provided as is, without warranty. See LICENSE.

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

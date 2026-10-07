---
name: tunnel-tool
description: Use `tun` whenever you need to reach a remote service from this machine through a local port, such as a Cloud SQL proxy, a kubectl port-forward, or an ssh tunnel. Also use it when a service on localhost is unreachable and a tunnel may be down.
---

# Tunnel tool

`tun` manages tunnels to remote services. Run `tun --help` for the commands and `tun <command> --help` for their flags. The help is the source of truth. Do not rely on a remembered command list.

## When to use tun

### Reaching a remote service needs a tunnel

Any time a task needs a database, an internal API, or a UI that is only reachable through a tunnel, use `tun`. Do not run `kubectl port-forward`, `cloud-sql-proxy`, or `ssh -L` yourself, and do not background them with `&`. Those leave processes nobody can see or stop.

### A localhost connection fails

Connection refused, a timeout, or a failing health URL on a tunneled port means the tunnel may be dead. Check `tun` before debugging the service.

### You need to know where a service lives

Ports are assigned, not fixed. Ask `tun` for the port instead of assuming one from a README, a script, or an old session.

### Nothing in the registry fits

If the service has no registered tunnel, suggest creating one with `tun create`. Do not run the raw command. You are never to create a tunnel without the user's permission - but you should suggest tunnels that you might need but do not have.

## Rules

### Never hardcode a port

Read it from `tun` every time. A tunnel can land on a different port after a restart.

### Use `ensure` to get a usable tunnel

`ensure` starts the tunnel if needed, repairs it if dead, waits until it is up, and prints only the port. Use it instead of checking status and then starting.

### Read the log before guessing

When a tunnel is down or unhealthy, `show` and `log` say why. Read them before restarting or changing anything.

### Do not touch the state directory

Never edit `~/.tun/tun.db` or kill tunnel processes by hand. Use `stop`, `restart`, and `remove`.

### Stop what you started

Tunnels started for a task that are not auto-run should be stopped when the task is done. Leave auto-run tunnels alone.

### Creating tunnels is interactive unless every argument is given

`tun create` with missing arguments prompts and fails without a terminal. Pass the name, command, and port range as arguments. Use a non-standard port range, and a health check when the service has a health URL.

## Example workflows

### Query a database through a proxy

```bash
PORT=$(tun ensure my-db)
psql -h 127.0.0.1 -p "$PORT" -U <user> <database>
```

### Call a tunneled API

```bash
PORT=$(tun ensure my-api)
curl -sf "localhost:$PORT/healthz"
```

### Diagnose a connection that stopped working

```bash
tun status --json
tun show <name>
tun log <name> -n 50
tun restart <name>
```

Read `show` and `log` before the restart. If the log shows an auth or credentials error, a restart will not fix it.

### Bring up everything marked auto-run

```bash
tun start -a
tun status
```

### Register a new tunnel

```bash
tun create <name> --command '<command with {port}>' --port-range 28100-28109 --health-check 'http://127.0.0.1:{port}/<health-path>'
tun ensure <name>
```

Again, suggest these commands to the user, do not execute them yourself unless given explicit permission.

`doctor` checks every registered tunnel, so use `ensure` to test just the new one.

### Check that tunnels are healthy before a long job

```bash
tun status -q || tun heal
```

`status -q` prints nothing and exits non-zero unless every running tunnel is up.

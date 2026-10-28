# Managing the coordinator remotely (Tailscale)

The coordinator can run **headless** (`./scripts/agents --daemon`): it takes tasks from a file
inbox, and with nothing queued it keeps sending the agent "Continue the task in
docs/HANDOFF.md." (backing off 1 to 30 minutes after failed turns, switching Claude/Codex on a
usage limit as usual). You steer it from any shell with `./scripts/agents-remote`, over SSH on
your tailnet, or from a small web page published only to your tailnet.

## One-time setup on this machine (needs your sudo and your Tailscale login)

```sh
sudo systemctl enable --now tailscaled
sudo tailscale up --ssh          # prints a login URL; open it on your phone/laptop
tailscale status                 # note this machine's name / 100.x address
```

`--ssh` turns on Tailscale SSH: any device on your tailnet that your ACLs allow can
`ssh jansolo@<machine-name>` with no keys or open ports. Install Tailscale on your phone or
laptop and sign in to the same account.

## Run the coordinator headless

Stop the interactive coordinator first (`/quit` at its prompt); only one may hold the lock.

```sh
mkdir -p ~/.config/systemd/user
cp tools/agent-coordinator/unigis-agents.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now unigis-agents
sudo loginctl enable-linger $USER      # keep running when you are logged out
journalctl --user -u unigis-agents -f  # or: ./scripts/agents-remote follow
```

(Without systemd: `nohup ./scripts/agents --daemon >/dev/null 2>&1 &`. Add `--no-keepalive` to
make it wait for tasks instead of continuing the handoff by itself.)

## Steer it from anywhere on the tailnet

```sh
ssh jansolo@<machine-name>
cd /mnt/media/ArcGIS_Linux
./scripts/agents-remote status
./scripts/agents-remote log 80
./scripts/agents-remote submit "Implement Surface Parameters and add ArcGIS reference cases"
./scripts/agents-remote cmd /switch codex
./scripts/agents-remote pause      # the turn in progress finishes; nothing new starts
./scripts/agents-remote resume
./scripts/agents-remote stop
```

Queued tasks run in order before any keepalive "continue". Commands (`/switch`, `/auto`,
`/new`, `/session`) run even while paused.

## Keeping a prompt open while it runs headless

Only one process can *own* the coordinator, so the interactive `./scripts/agents` and the
daemon cannot run together. Run the daemon (it keeps working when you leave) and use
`./scripts/agents-remote chat` as your prompt at the machine: it looks like the normal
coordinator (type a task, `/switch codex`, `/pause`, `/status`), streams the daemon's log, and
you can quit and re-open it at any time without stopping the work. Over SSH from your phone it
is the same command. Only `/cli` (the native Claude/Codex TUI) needs the interactive
coordinator: `agents-remote stop`, run `./scripts/agents`, and start the service again after.

## Optional web page and progress dashboard (tailnet only)

```sh
./scripts/agents-remote web 8765 tailscale   # listens on the tailnet address only (no sudo); without `tailscale`: 127.0.0.1; /dashboard serves docs/dashboard.html
# or as a service:  cp tools/agent-coordinator/unigis-web.service ~/.config/systemd/user/ && systemctl --user enable --now unigis-web
tailscale serve --bg 8765                   # publishes it to your tailnet over HTTPS
tailscale serve status                      # shows the https://<machine>.<tailnet>.ts.net URL
```

The page shows status, queue and log, and has buttons to queue a task, pause/resume, switch
agent and stop. Because the server binds to localhost and Tailscale Serve is reachable only from
your tailnet, no one else can reach it. Do not use `tailscale funnel` (that is public).

## Notes

- State and the remote files live under `.git/agent-coordinator/remote/` (inbox, done,
  status.json, daemon.log, PAUSE, STOP), private to your user.
- `/cli` (native interactive Claude/Codex) is not available headless; use the interactive
  coordinator at the machine for that.
- The daemon runs with full access like the interactive coordinator (see README).
- The headless coordinator cannot log in to a provider for you: if Claude or Codex logs out, the
  turns fail and the daemon backs off; log in again in a terminal, then `resume` is automatic.

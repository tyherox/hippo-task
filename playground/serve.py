#!/usr/bin/env python3
"""Local UI server for the REAL `hippo-task` CLI (zero dependencies, stdlib only).

    python3 playground/serve.py            # builds the binary (cargo build --locked), opens the UI  (or: make ui)
    PORT=9000 python3 playground/serve.py  # custom port

Serves ui.html and exposes a tiny JSON API that shells out to the compiled
`hippo-task` binary against a scratch store. The UI has a Live mode (real CLI — the
board shows the CLI's own `list --json`, so it can't drift from the binary) and
a Simulate mode (in-browser). Nothing here touches a real project.
"""
import http.server, socketserver, subprocess, os, json, shutil, sys, webbrowser, threading, pathlib, urllib.parse

HERE = pathlib.Path(__file__).resolve().parent  # playground/
ROOT = HERE.parent                              # the crate
BIN = ROOT / "target" / "debug" / "hippo-task"
STORE = pathlib.Path(os.environ.get("HIPPO_UI_STORE", str(HERE / "ui-data")))
PORT = int(os.environ.get("PORT", "8787"))


def log(msg):
    print(msg, file=sys.stderr, flush=True)


def ensure_bin():
    # Always build so a stale binary can't shadow newly-added commands
    # (cargo is a no-op when nothing changed).
    print("building hippo-task…", flush=True)
    r = subprocess.run(["cargo", "build", "--locked"], cwd=str(ROOT))
    if r.returncode != 0 or not BIN.exists():
        print("build failed — is Rust installed and the code compiling?")
        raise SystemExit(1)


def run_cli(args, actor=None, node=None):
    env = dict(os.environ, HIPPO_ACTOR=actor or "human:you", HIPPO_NODE=node or "n0")
    env.pop("HIPPO_DIR", None)
    STORE.mkdir(parents=True, exist_ok=True)  # --dir must exist
    p = subprocess.run([str(BIN), "--dir", str(STORE), *[str(a) for a in args]],
                       capture_output=True, text=True, env=env)
    return {"ok": p.returncode == 0, "stdout": p.stdout.strip(), "stderr": p.stderr.strip(), "code": p.returncode}


def cli_json(args):
    """Run a read-only command with --json; return (value, error_text)."""
    r = run_cli([*args, "--json"])
    if not r["ok"]:
        return None, r["stderr"]
    try:
        return json.loads(r["stdout"] or "null"), None
    except json.JSONDecodeError as e:
        log(f"warning: `hippo-task {' '.join(args)} --json` printed invalid JSON: {e}")
        return None, str(e)


def read_ledger():
    f = STORE / ".hippotask" / "ledger.jsonl"
    if not f.exists():
        return []
    out = []
    for n, line in enumerate(f.read_text(errors="replace").splitlines(), 1):
        line = line.strip()
        if not line:
            continue
        try:
            out.append(json.loads(line))
        except json.JSONDecodeError as e:
            log(f"warning: {f}:{n}: skipped an unreadable line ({e})")
    return out


def state(sel):
    """What the Live UI renders: the CLI's own projection + the raw ledger."""
    tasks, err = cli_json(["list"])
    out = {"tasks": tasks or [], "events": read_ledger(), "detail": None}
    if err:
        out["error"] = err
    if sel:
        detail, _missing = cli_json(["show", sel])  # a stale selection (e.g. after reset) is fine
        out["detail"] = detail
    return out


def wipe_store():
    if STORE.exists():
        try:
            shutil.rmtree(STORE)
        except OSError as e:
            log(f"warning: couldn't fully remove {STORE}: {e}")


def seed():
    wipe_store()

    def add(title, *flags):
        r = run_cli(["add", title, *flags, "--json"])
        if not r["ok"]:
            raise RuntimeError(f"seed: add {title!r} failed: {r['stderr']}")
        return json.loads(r["stdout"])["id"]

    t1 = add("Ship auth", "--priority", "urgent", "--label", "backend",
             "--body", "Users get a 401 after ~1h — suspect a token-refresh race.")
    t2 = add("Write docs", "--priority", "high")
    add("Polish landing", "--priority", "med")
    run_cli(["start", t1], "agent:claude", "cc")
    run_cli(["note", t1, "repro'd with two tabs; looking at the refresh lock"], "agent:claude", "cc")
    run_cli(["lease", t2], "agent:codex", "cx")
    run_cli(["update", t2, "--block", t1])


class Handler(http.server.BaseHTTPRequestHandler):
    def _send(self, code, body, ctype="application/json"):
        b = body.encode() if isinstance(body, str) else body
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)

    def do_GET(self):
        url = urllib.parse.urlparse(self.path)
        if url.path in ("/", "/index.html"):
            self._send(200, (HERE / "ui.html").read_text(), "text/html; charset=utf-8")
        elif url.path == "/state":
            sel = urllib.parse.parse_qs(url.query).get("sel", [None])[0]
            self._send(200, json.dumps(state(sel)))
        elif url.path == "/ledger":
            self._send(200, json.dumps({"events": read_ledger()}))
        else:
            self._send(404, "{}")

    def do_POST(self):
        path = urllib.parse.urlparse(self.path).path
        n = int(self.headers.get("Content-Length", "0"))
        try:
            data = json.loads(self.rfile.read(n) or b"{}")
        except json.JSONDecodeError as e:
            log(f"warning: bad JSON body for {path}: {e}")
            self._send(400, json.dumps({"ok": False, "stderr": f"bad request body: {e}", "code": 2}))
            return
        if path == "/run":
            self._send(200, json.dumps(run_cli(data.get("args", []), data.get("actor"), data.get("node"))))
        elif path == "/reset":
            wipe_store()
            self._send(200, '{"ok":true}')
        elif path == "/seed":
            try:
                seed()
                self._send(200, '{"ok":true}')
            except (RuntimeError, json.JSONDecodeError, KeyError) as e:
                log(f"error: {e}")
                self._send(500, json.dumps({"ok": False, "error": str(e)}))
        else:
            self._send(404, "{}")

    def log_message(self, *a):
        pass  # keep the terminal quiet: one line per request is noise here


class Server(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


if __name__ == "__main__":
    ensure_bin()
    url = f"http://127.0.0.1:{PORT}/"
    print(f"hippo-task UI  →  {url}\nstore: {STORE}\n(ctrl-c to stop)", flush=True)
    if "--no-open" not in sys.argv:
        threading.Timer(0.6, lambda: webbrowser.open(url)).start()
    try:
        Server(("127.0.0.1", PORT), Handler).serve_forever()
    except KeyboardInterrupt:
        print("\nbye")

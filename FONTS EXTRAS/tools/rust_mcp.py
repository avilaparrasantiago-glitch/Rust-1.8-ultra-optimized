# /// script
# requires-python = ">=3.11"
# dependencies = ["mcp>=1.12,<2"]
# ///
"""Project-scoped Cargo diagnostics and rust-analyzer hover for Codex."""
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def hover(path, line, column):
    import os
    import time
    source = (ROOT / path).resolve()
    source.relative_to(ROOT)
    if source.suffix != ".rs":
        raise ValueError("Expected a Rust source file")
    proc = subprocess.Popen(["rust-analyzer"], cwd=ROOT, stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)

    def send(message):
        data = json.dumps({"jsonrpc": "2.0", **message}).encode()
        proc.stdin.write(f"Content-Length: {len(data)}\r\n\r\n".encode() + data)
        proc.stdin.flush()

    def read():
        headers = {}
        while True:
            row = proc.stdout.readline()
            if not row:
                raise EOFError("rust-analyzer exited")
            if row == b"\r\n":
                break
            k, v = row.decode().split(":", 1)
            headers[k.lower()] = v.strip()
        return json.loads(proc.stdout.read(int(headers["content-length"])))

    try:
        send(dict(id=1, method="initialize", params=dict(processId=os.getpid(), rootUri=ROOT.as_uri(),
             capabilities={"experimental": {"serverStatusNotification": True}},
             workspaceFolders=[{"uri": ROOT.as_uri(), "name": "SyRust"}])))
        while read().get("id") != 1:
            pass
        send(dict(method="initialized", params={}))
        uri = source.as_uri()
        send(dict(method="textDocument/didOpen", params={"textDocument": {
            "uri": uri, "languageId": "rust", "version": 1, "text": source.read_text(encoding="utf8")}}))
        while True:
            msg = read()
            if msg.get("method") == "experimental/serverStatus" and msg["params"].get("quiescent"):
                break
            if "method" in msg and "id" in msg:
                send(dict(id=msg["id"], result=None))
        send(dict(id=2, method="textDocument/hover", params={
            "textDocument": {"uri": uri}, "position": {"line": line-1, "character": column-1}}))
        while True:
            msg = read()
            if msg.get("id") == 2:
                return msg
    finally:
        proc.kill()
        proc.wait()


if __name__ == "__main__" and sys.argv[1:2] == ["hover"]:
    print(json.dumps(hover(sys.argv[2], int(sys.argv[3]), int(sys.argv[4]))))
else:
    from mcp.server.fastmcp import FastMCP
    server = FastMCP("coral-rust")

    @server.tool()
    def cargo_diagnostics(clippy: bool = False) -> str:
        """Check the workspace with Cargo or strict Clippy and return compiler diagnostics."""
        args = ["cargo", "clippy" if clippy else "check", "--workspace", "--locked"]
        if clippy:
            args += ["--", "-D", "warnings"]
        p = subprocess.run(args, cwd=ROOT, capture_output=True, text=True, timeout=300)
        return f"exit={p.returncode}\n{p.stderr[-24000:]}"

    @server.tool()
    def rust_hover(path: str, line: int, column: int) -> str:
        """Resolve a type or signature with rust-analyzer. Coordinates are 1-based UTF-16."""
        p = subprocess.run([sys.executable, __file__, "hover", path, str(line), str(column)],
                           capture_output=True, text=True, timeout=180)
        return p.stdout if p.returncode == 0 else p.stderr

    if __name__ == "__main__":
        server.run()

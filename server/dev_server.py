# dev_server.py — Python mirror of protocol-keyserver for local development.
# Identical API to the Rust version; runs under Smart App Control because
# python.exe is org-signed. Use the Rust build for real deployments.
#
#   python dev_server.py            -> http://127.0.0.1:8080
#
# First run seeds a demo key (printed to console) for user "demo".

import hashlib
import json
import os
import secrets
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

BASE = Path(__file__).resolve().parent
WEB = BASE.parent / "web"
KEY_FILE = BASE / "keys.json"

ALPHABET = "K7QWZX9MNP3RCTVH"
RENEW_SECONDS = 24 * 3600
ADMIN_CAP_SECONDS = 30 * 24 * 3600  # admin renewals capped at +30d from now
MAX_FAILS = 10
FAIL_WINDOW = 3600

# Full catalog shown in the "all apps" view; a key's record lists its subset.
ALL_PLATFORMS = [f"Platform {i}" for i in range(1, 13)]

ADMIN_TOKEN = os.environ.get("PROTOCOL_ADMIN_TOKEN", "")

MIME = {
    ".html": "text/html", ".css": "text/css", ".js": "application/javascript",
    ".png": "image/png", ".svg": "image/svg+xml", ".ico": "image/x-icon",
}


def key_hash(key: str) -> str:
    return hashlib.sha256(key.encode()).hexdigest()


def fmt_key(raw: bytes) -> str:
    nibbles = [n for b in raw for n in (b >> 4, b & 0xF)]
    groups = ["".join(ALPHABET[n] for n in nibbles[i:i + 4]) for i in range(0, 20, 4)]
    return "-".join(groups)


def load_keys() -> dict:
    if KEY_FILE.exists():
        try:
            return json.loads(KEY_FILE.read_text())
        except json.JSONDecodeError:
            pass
    return {}


def save_keys(keys: dict) -> None:
    KEY_FILE.write_text(json.dumps(keys, indent=2))


KEYS = load_keys()
FAILS: dict[str, tuple[float, int]] = {}


def seed_demo() -> None:
    if KEYS:
        return
    key = fmt_key(secrets.token_bytes(10))
    KEYS[key_hash(key)] = {
        "username": "demo",
        "platforms": ["Platform 1", "Platform 2", "Platform 3"],
        "expires_at": int(time.time()) + 24 * 3600,
    }
    save_keys(KEYS)
    print(f"*** demo key (valid 24h, user 'demo'): {key}")


def rate_limited(ip: str) -> bool:
    t = time.time()
    start, count = FAILS.get(ip, (t, 0))
    if t - start > FAIL_WINDOW:
        start, count = t, 0
    FAILS[ip] = (start, count + 1)
    return FAILS[ip][1] > MAX_FAILS


class Handler(BaseHTTPRequestHandler):
    def _json(self, code: int, obj: dict) -> None:
        body = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()
        self.wfile.write(body)

    def do_OPTIONS(self):
        self.send_response(204)
        self.send_header("Access-Control-Allow-Origin", "*")
        self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        self.send_header("Access-Control-Allow-Headers", "Content-Type, X-Admin-Token")
        self.end_headers()

    def _body(self) -> dict:
        n = int(self.headers.get("Content-Length") or 0)
        try:
            return json.loads(self.rfile.read(n) or b"{}")
        except json.JSONDecodeError:
            return {}

    def _lookup(self):
        key = self._body().get("key", "")
        return key, KEYS.get(key_hash(key))

    def log_message(self, fmt, *args):  # keep console quiet
        pass

    # ---------- API ----------

    def do_POST(self):
        if rate_limited(self.client_address[0]):
            return self._json(429, {"valid": False, "ok": False, "error": "rate limited"})

        if self.path == "/api/validate":
            key, rec = self._lookup()
            if not rec:
                return self._json(401, {"valid": False, "error": "invalid key"})
            left = rec["expires_at"] - int(time.time())
            if left <= 0:
                return self._json(401, {"valid": False, "error": "key expired"})
            return self._json(200, {
                "valid": True,
                "username": rec["username"],
                "platforms": rec["platforms"],
                "seconds_left": left,
            })

        if self.path == "/api/renew":
            key, rec = self._lookup()
            if not rec:
                return self._json(401, {"ok": False, "error": "invalid key"})
            if rec["expires_at"] <= time.time():
                return self._json(401, {"ok": False, "error": "key expired"})
            if not rec.get("admin"):
                # Normal keys renew through the ad-link checkpoint
                # (Linkvertise/LootLabs flow — to be built).
                return self._json(200, {"ok": False, "checkpoint_required": True})
            cap = int(time.time()) + ADMIN_CAP_SECONDS
            rec["expires_at"] = min(rec["expires_at"] + RENEW_SECONDS, cap)
            save_keys(KEYS)
            return self._json(200, {
                "ok": True,
                "seconds_left": rec["expires_at"] - int(time.time()),
                "capped": rec["expires_at"] >= cap,
            })

        if self.path == "/api/select":
            body = self._body()
            rec = KEYS.get(key_hash(body.get("key", "")))
            if not rec or rec["expires_at"] <= time.time():
                return self._json(401, {"ok": False, "error": "invalid or expired key"})
            if any(p.lower() == body.get("platform", "").lower() for p in rec["platforms"]):
                return self._json(200, {"ok": True, "platform": body["platform"]})
            return self._json(403, {"ok": False, "error": "platform not on key"})

        if self.path == "/api/admin/issue":
            got = self.headers.get("X-Admin-Token", "")
            if not ADMIN_TOKEN or not secrets.compare_digest(got, ADMIN_TOKEN):
                return self._json(403, {"ok": False, "error": "forbidden"})
            body = self._body()
            key = fmt_key(secrets.token_bytes(10))
            KEYS[key_hash(key)] = {
                "username": body.get("username", "user"),
                "platforms": body.get("platforms", []),
                "expires_at": int(time.time()) + min(body.get("hours", 24), 24 * 365) * 3600,
                "admin": bool(body.get("admin", False)),
            }
            save_keys(KEYS)
            return self._json(200, {"key": key})

        return self._json(404, {"error": "not found"})

    def do_GET(self):
        if self.path == "/api/status":
            return self._json(200, {"keys": len(KEYS)})

        if self.path == "/api/platforms":
            return self._json(200, {"platforms": ALL_PLATFORMS})

        # static files
        rel = self.path.split("?")[0].lstrip("/") or "index.html"
        target = (WEB / rel).resolve()
        if not str(target).startswith(str(WEB.resolve())) or not target.is_file():
            self.send_error(404)
            return
        body = target.read_bytes()
        self.send_response(200)
        self.send_header("Content-Type", MIME.get(target.suffix, "application/octet-stream"))
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)


if __name__ == "__main__":
    seed_demo()
    print("dev server on http://127.0.0.1:8080")
    ThreadingHTTPServer(("127.0.0.1", 8080), Handler).serve_forever()

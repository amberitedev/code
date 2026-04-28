#!/usr/bin/env python3
"""Amberite Core diagnostic — exercises every API endpoint."""
import argparse, io, sys, time
import requests

URL: str = "http://localhost:16662"
HDR: dict = {}
IID: str | None = None
PASS = FAIL = SKIP = 0

def _r(code, sym, tag, msg=""):
    global PASS, FAIL, SKIP
    print(f"  \033[{(32,31,33)[code]}m{sym}\033[0m {tag}" + (f": {msg}" if msg else ""))
    if code == 0:   PASS += 1
    elif code == 1: FAIL += 1
    else:           SKIP += 1

def ok(t):         _r(0, "✓", t)
def fail(t, m=""): _r(1, "✗", t, m)
def skip(t, r=""): _r(2, "~", t, r)

def req(method, path, *, auth=True, **kw):
    h = HDR.copy() if auth else {}
    try:
        return getattr(requests, method)(f"{URL}{path}", headers=h, timeout=15, **kw)
    except requests.RequestException as e:
        class E:
            status_code = 0; text = str(e); ok = False
            def json(self): return {}
        return E()

def chk(tag, r, code=200):
    if r.status_code == code: ok(tag); return True
    fail(tag, f"HTTP {r.status_code} {r.text[:100]}"); return False

def chk2(tag, r, *codes):
    if r.status_code in (codes or (200, 201)): ok(tag); return True
    fail(tag, f"HTTP {r.status_code} {r.text[:100]}"); return False

def p_system():
    print("\n── System ──")
    chk("GET /health",       req("get", "/health",       auth=False))
    chk("GET /version",      req("get", "/version",      auth=False))
    chk("GET /java",         req("get", "/java",         auth=False))
    chk("GET /setup/status", req("get", "/setup/status", auth=False))

def p_setup(code, supa_url, owner):
    print("\n── Setup ──")
    if not code:
        skip("POST /setup", "pass --pairing-code to test"); return
    chk("POST /setup", req("post", "/setup", auth=False,
        json={"code": code, "supabase_url": supa_url, "owner_user_id": owner}))

def p_instances():
    global IID
    print("\n── Instances ──")
    chk("GET /instances", req("get", "/instances"))
    r = req("post", "/instances", json={
        "name": "diag-test", "game_version": "1.20.1", "loader": "vanilla",
        "port": 25575, "memory": {"min_mb": 512, "max_mb": 1024},
    })
    if not chk("POST /instances", r, 201): return
    IID = r.json().get("id")
    chk("GET /instances/:id", req("get", f"/instances/{IID}"))
    print("    polling until install finishes (max 60 s) …")
    for _ in range(30):
        st = req("get", f"/instances/{IID}").json().get("status", "")
        if st != "installing": break
        time.sleep(2)

def p_lifecycle():
    if not IID: return
    print("\n── Lifecycle ──")
    chk2("POST .../start",   req("post", f"/instances/{IID}/start"),   200, 400)
    chk2("POST .../stop",    req("post", f"/instances/{IID}/stop"),    200, 400)
    chk2("POST .../restart", req("post", f"/instances/{IID}/restart"), 200, 400)
    chk2("POST .../kill",    req("post", f"/instances/{IID}/kill"),    200, 400)
    chk2("POST .../command", req("post", f"/instances/{IID}/command", json={"command": "list"}), 200, 400)
    chk2("POST /ws-token",   req("post", "/ws-token", json={"instance_id": IID}), 200, 400)

def p_mods():
    if not IID: return
    print("\n── Mods ──")
    chk("GET .../mods", req("get", f"/instances/{IID}/mods"))
    jar = io.BytesIO(b"PK\x05\x06" + b"\x00" * 18)
    r = req("post", f"/instances/{IID}/mods/upload",
            files={"file": ("dummy.jar", jar, "application/java-archive")})
    if chk2("POST .../mods/upload", r, 200, 201, 400):
        fn = "dummy.jar"
        chk2("PATCH .../mods/:f toggle",
             req("patch", f"/instances/{IID}/mods/{fn}", json={"enabled": False}), 200, 400, 404)
        chk2("PUT .../mods/:f/update",
             req("put", f"/instances/{IID}/mods/{fn}/update"), 200, 400, 404)
        chk2("DELETE .../mods/:f",
             req("delete", f"/instances/{IID}/mods/{fn}"), 200, 204, 404)
    chk2("POST .../mods (Modrinth)",
         req("post", f"/instances/{IID}/mods", json={"project_id": "fabric-api"}), 200, 201, 400, 422, 503)
    chk2("POST .../mods/update-all",
         req("post", f"/instances/{IID}/mods/update-all"), 200, 400)

def p_logs():
    if not IID: return
    print("\n── Logs ──")
    r = req("get", f"/instances/{IID}/logs")
    chk("GET .../logs", r)
    if logs := (r.json() if r.ok else []):
        chk("GET .../logs/:file", req("get", f"/instances/{IID}/logs/{logs[0]}"))
    else:
        skip("GET .../logs/:file", "no log files yet")
    rc = req("get", f"/instances/{IID}/crash-reports")
    chk("GET .../crash-reports", rc)
    if crs := (rc.json() if rc.ok else []):
        chk("GET .../crash-reports/:file", req("get", f"/instances/{IID}/crash-reports/{crs[0]}"))
    else:
        skip("GET .../crash-reports/:file", "no crash reports yet")

def p_properties():
    if not IID: return
    print("\n── Properties ──")
    chk2("GET  .../properties", req("get", f"/instances/{IID}/properties"), 200, 400, 404)
    chk2("PATCH .../properties", req("patch", f"/instances/{IID}/properties", json={"motd": "amberite-diag"}), 200, 400, 404)

def p_stats():
    if not IID: return
    print("\n── Stats ──")
    chk2("GET .../stats", req("get", f"/instances/{IID}/stats"), 200, 400)

def p_modpack():
    if not IID: return
    print("\n── Modpack ──")
    chk2("GET .../modpack", req("get", f"/instances/{IID}/modpack"), 200, 404)
    chk2("POST .../modpack install", req("post", f"/instances/{IID}/modpack",
         json={"modrinth_project_id": "adrenaserver", "modrinth_version_id": "latest"}), 200, 201, 400, 422, 503)
    chk2("GET  .../modpack/export", req("get", f"/instances/{IID}/modpack/export"), 200, 400, 404)
    chk2("DELETE .../modpack", req("delete", f"/instances/{IID}/modpack"), 200, 204, 400, 404)

def p_macros():
    if not IID: return
    print("\n── Macros ──")
    chk("GET .../macros", req("get", f"/instances/{IID}/macros"))
    r = req("post", f"/instances/{IID}/macros", json={"name": "nonexistent"})
    chk2("POST .../macros spawn", r, 200, 201, 400, 404)
    pid = (r.json() or {}).get("pid") if r.status_code in (200, 201) else None
    if pid:
        chk2(f"DELETE .../macros/{pid}", req("delete", f"/instances/{IID}/macros/{pid}"), 200, 204, 404)
    else:
        skip("DELETE .../macros/:pid", "no macro spawned")

def p_cleanup():
    if not IID: return
    print("\n── Cleanup ──")
    chk2("DELETE /instances/:id", req("delete", f"/instances/{IID}"), 200, 204)

def main():
    global URL, HDR
    ap = argparse.ArgumentParser(description="Amberite Core diagnostic")
    ap.add_argument("--url",          default="http://localhost:16662")
    ap.add_argument("--token",        default="",  help="Supabase JWT")
    ap.add_argument("--pairing-code", default="", dest="pairing_code")
    ap.add_argument("--supabase-url", default="", dest="supabase_url")
    ap.add_argument("--owner-id",     default="", dest="owner_id")
    args = ap.parse_args()
    URL = args.url.rstrip("/")
    HDR = {"Authorization": f"Bearer {args.token}"} if args.token else {}
    print(f"Amberite Core Diagnostic  →  {URL}")
    if not HDR: print("  ⚠  No --token provided; authenticated endpoints will likely 401.")
    p_system(); p_setup(args.pairing_code, args.supabase_url, args.owner_id)
    p_instances(); p_lifecycle(); p_mods(); p_logs()
    p_properties(); p_stats(); p_modpack(); p_macros(); p_cleanup()
    total = PASS + FAIL + SKIP
    print(f"\n{'─'*48}\n  PASS {PASS}/{total}   FAIL {FAIL}   SKIP {SKIP}")
    msg, col = ("● All checks passed", 32) if not FAIL else ("● Some checks failed", 31)
    print(f"  \033[{col}m{msg}\033[0m")
    if FAIL: sys.exit(1)

if __name__ == "__main__":
    main()

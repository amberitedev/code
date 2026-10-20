#!/usr/bin/env sh
# Download the copal binary for this machine from the newest Copal release on a channel, checked
# against the release manifest's SHA-256. Then run `sudo copal install` to set Copal up as a
# service. From there Core updates itself and checks every update's signature.
set -eu

repository="amberitedev/code"
channel="stable"
install_dir="${COPAL_INSTALL_DIR:-$HOME/.local/bin}"

while [ "$#" -gt 0 ]; do
	case "$1" in
		--channel) channel="$2"; shift 2 ;;
		--install-dir) install_dir="$2"; shift 2 ;;
		*) printf '%s\n' "Unknown option: $1" >&2; exit 2 ;;
	esac
done

case "$(uname -s)-$(uname -m)" in
	Linux-x86_64) asset="copal-linux-x64" ;;
	Linux-aarch64 | Linux-arm64) asset="copal-linux-arm64" ;;
	Darwin-arm64) asset="copal-macos-arm64" ;;
	*) printf '%s\n' "Copal is released for Linux x64, Linux arm64, macOS arm64, and Windows x64." >&2; exit 1 ;;
esac

command -v curl >/dev/null 2>&1 || { printf '%s\n' "curl is required." >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { printf '%s\n' "python3 is required." >&2; exit 1; }

temporary_dir="$(mktemp -d)"
cleanup() { rm -rf "$temporary_dir"; }
trap cleanup EXIT INT TERM

# The same rules as Core's updater: copal-v tags only, and each channel also gets the ones
# before it (beta gets stable releases, alpha gets both). The Modrinth App's releases share the
# list, so it is read page by page until a page holds a release on the channel.
cat > "$temporary_dir/resolve.py" <<'EOF'
import json, re, sys

releases = json.load(open(sys.argv[1]))
allowed = {"stable": ["stable"], "beta": ["stable", "beta"], "alpha": ["stable", "beta", "alpha"]}[sys.argv[2]]
pattern = re.compile(r"copal-v(\d+)\.(\d+)\.(\d+)(?:-(beta|alpha)(?:\.(\d+))?)?$")
order = {"alpha": 0, "beta": 1, None: 2}
best = None
for release in releases:
    match = pattern.match(release["tag_name"])
    if release.get("draft") or not match:
        continue
    major, minor, patch, pre, number = match.groups()
    if (pre or "stable") not in allowed:
        continue
    key = (int(major), int(minor), int(patch), order[pre], int(number or 0))
    if best is None or key > best[0]:
        best = (key, release["tag_name"])
print(best[1] if best else "")
EOF
tag=""
for page in 1 2 3 4 5; do
	curl --fail --silent --show-error "https://api.github.com/repos/$repository/releases?per_page=100&page=$page" -o "$temporary_dir/releases.json"
	tag="$(python3 "$temporary_dir/resolve.py" "$temporary_dir/releases.json" "$channel")"
	[ -n "$tag" ] && break
	grep -q '"tag_name"' "$temporary_dir/releases.json" || break
done
[ -n "$tag" ] || { printf '%s\n' "No Copal release is on the $channel channel yet." >&2; exit 1; }

base_url="https://github.com/$repository/releases/download/$tag"
curl --fail --location --silent --show-error "$base_url/$asset" -o "$temporary_dir/copal"
curl --fail --location --silent --show-error "$base_url/copal-manifest.json" -o "$temporary_dir/manifest.json"
python3 - "$temporary_dir/manifest.json" "$temporary_dir/copal" "$asset" <<'EOF'
import hashlib, json, sys

manifest = json.load(open(sys.argv[1]))
expected = next(file["sha256"] for file in manifest["files"] if file["name"] == sys.argv[3])
if hashlib.sha256(open(sys.argv[2], "rb").read()).hexdigest() != expected:
    sys.exit("The download does not match the release manifest.")
EOF

mkdir -p "$install_dir"
install -m 0755 "$temporary_dir/copal" "$install_dir/copal"
printf '%s\n' "Installed Copal ${tag#copal-v} to $install_dir/copal"
printf '%s\n' "Set it up as a service with: sudo $install_dir/copal install"

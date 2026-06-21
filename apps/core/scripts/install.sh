#!/usr/bin/env sh
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
	Linux-x86_64) asset="copal-linux-x64.tar.gz" ;;
	*) printf '%s\n' "Copal's curl installer currently supports Linux x64 only." >&2; exit 1 ;;
esac

command -v curl >/dev/null 2>&1 || { printf '%s\n' "curl is required." >&2; exit 1; }
command -v tar >/dev/null 2>&1 || { printf '%s\n' "tar is required." >&2; exit 1; }
command -v sha256sum >/dev/null 2>&1 || { printf '%s\n' "sha256sum is required." >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { printf '%s\n' "python3 is required." >&2; exit 1; }

if [ "$channel" = "stable" ]; then
	tag="$(curl --fail --silent --show-error "https://api.github.com/repos/$repository/releases/latest" | python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"])')"
else
	tag="$(curl --fail --silent --show-error "https://api.github.com/repos/$repository/releases?per_page=100" | python3 -c 'import json,sys; channel=sys.argv[1]; releases=json.load(sys.stdin); print(next((r["tag_name"] for r in releases if r["prerelease"] and channel in r["tag_name"]), ""))' "$channel")"
	[ -n "$tag" ] || { printf '%s\n' "No $channel Copal release is available." >&2; exit 1; }
fi

base_url="https://github.com/$repository/releases/download/$tag"
temporary_dir="$(mktemp -d)"
cleanup() { rm -rf "$temporary_dir"; }
trap cleanup EXIT INT TERM

curl --fail --location --silent --show-error "$base_url/$asset" -o "$temporary_dir/$asset"
curl --fail --location --silent --show-error "$base_url/SHA256SUMS" -o "$temporary_dir/SHA256SUMS"
(cd "$temporary_dir" && grep "  $asset$" SHA256SUMS | sha256sum --check --status)

tar -xzf "$temporary_dir/$asset" -C "$temporary_dir"
[ -f "$temporary_dir/copal" ] || { printf '%s\n' "Release archive does not contain the Copal binary." >&2; exit 1; }
mkdir -p "$install_dir"
install -m 0755 "$temporary_dir/copal" "$install_dir/copal"
printf '%s\n' "Installed Copal $tag to $install_dir/copal"
case ":$PATH:" in
	*":$install_dir:"*) ;;
	*) printf '%s\n' "Add $install_dir to PATH, then run: copal install --curl --env-file /path/to/copal.env" ;;
esac

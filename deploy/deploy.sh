#!/bin/sh
# Build and deploy to vttl.ttczonnebeke.be. Run from the repo root or anywhere:
#
#   sh deploy/deploy.sh
#
# The web client is built here (pnpm). The server binary is built on the server (ARM64)
# by the vttl-build user, then installed in /opt/vttl-league. The database is not touched.
set -eu
HOST="${VL_HOST:-westtoer-directus}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SSH="ssh -o BatchMode=yes -o LogLevel=ERROR"
cd "$ROOT"

REV=$(git rev-parse --short HEAD)
git diff --quiet HEAD || REV="$REV-dirty"
echo "== deploy $REV to $HOST"

echo "== build web client"
(cd web && pnpm install --frozen-lockfile --silent && pnpm build >/dev/null)

echo "== upload sources and web/dist"
rsync -az --delete -e "$SSH" Cargo.toml Cargo.lock src "$HOST:/home/vttl-build/src/"
rsync -az --delete -e "$SSH" web/dist/ "$HOST:/opt/vttl-league/web.new/"

echo "== build server binary on $HOST (first build takes a few minutes)"
$SSH "$HOST" '
set -e
chown -R vttl-build:vttl-build /home/vttl-build/src
cd /home/vttl-build/src
sudo -u vttl-build -H /home/vttl-build/.cargo/bin/cargo build --release --locked --quiet
'

echo "== install and restart"
$SSH "$HOST" "
set -e
cd /opt/vttl-league
# Keep the previous release for a quick rollback.
[ -f vttl-league ] && cp -p vttl-league vttl-league.prev
[ -d web ] && rm -rf web.prev && mv web web.prev
install -m 755 /home/vttl-build/src/target/release/vttl-league vttl-league.new
mv vttl-league.new vttl-league
chown -R root:root web.new && mv web.new web
echo $REV > REVISION
systemctl restart vttl-league
for i in 1 2 3 4 5 6 7 8 9 10; do
  curl -fs http://127.0.0.1:3100/api/health >/dev/null && break
  sleep 1
done
curl -fs http://127.0.0.1:3100/api/health >/dev/null || { journalctl -u vttl-league -n 30 --no-pager; exit 1; }
echo \"running $REV\"
"

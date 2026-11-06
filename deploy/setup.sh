#!/bin/sh
# One-time server setup for competitie.ttczonnebeke.be. Safe to run again.
#
#   sh deploy/setup.sh            # users, dirs, Rust, env file, systemd unit, nginx (HTTP)
#   sh deploy/setup.sh --tls      # also gets the Let's Encrypt certificate (DNS must point here)
#
# Server: westtoer-directus (Hetzner cax11, ARM64, Ubuntu 24.04), SSH via Tailscale.
# Shares the box with nginx, mosquitto and Directus: the app only listens on 127.0.0.1:3100.
set -eu
HOST="${VL_HOST:-westtoer-directus}"
DOMAIN=competitie.ttczonnebeke.be
DIR="$(cd "$(dirname "$0")" && pwd)"
SSH="ssh -o BatchMode=yes -o LogLevel=ERROR"

echo "== $HOST: users, directories, packages"
$SSH "$HOST" '
set -e
id vttl-league >/dev/null 2>&1 || useradd --system --home /var/lib/vttl-league --shell /usr/sbin/nologin vttl-league
id vttl-build  >/dev/null 2>&1 || useradd --system --create-home --home /home/vttl-build --shell /bin/bash vttl-build
install -d -o vttl-league -g vttl-league -m 750 /var/lib/vttl-league
install -d -o root -g root -m 755 /opt/vttl-league
install -d -o vttl-build -g vttl-build -m 755 /home/vttl-build/src
dpkg -s build-essential >/dev/null 2>&1 || DEBIAN_FRONTEND=noninteractive apt-get install -y -qq build-essential >/dev/null
dpkg -s sqlite3 >/dev/null 2>&1 || DEBIAN_FRONTEND=noninteractive apt-get install -y -qq sqlite3 >/dev/null
'

echo "== $HOST: Rust toolchain for vttl-build"
$SSH "$HOST" '
set -e
if [ ! -x /home/vttl-build/.cargo/bin/cargo ]; then
  sudo -u vttl-build -H sh -c "curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path" >/dev/null
fi
sudo -u vttl-build -H /home/vttl-build/.cargo/bin/rustup update stable >/dev/null 2>&1 || true
sudo -u vttl-build -H /home/vttl-build/.cargo/bin/cargo --version
'

echo "== $HOST: /etc/vttl-league.env (secret made on the server, never printed)"
$SSH "$HOST" '
set -e
if [ ! -f /etc/vttl-league.env ]; then
  umask 027
  cat > /etc/vttl-league.env <<EOF
BIND=127.0.0.1:3100
DATABASE=/var/lib/vttl-league/vttl-league.sqlite
WEB_DIR=/opt/vttl-league/web
SECRET_KEY=$(openssl rand -base64 32)
COOKIE_SECURE=true
# Real uploads to VTTL: keep false until the first upload is tested together.
UPLOAD_ENABLED=false
RUST_LOG=info
EOF
  chown root:vttl-league /etc/vttl-league.env
  chmod 640 /etc/vttl-league.env
  echo "created"
else
  echo "exists, not changed"
fi
'

echo "== $HOST: systemd unit"
$SSH "$HOST" 'cat > /etc/systemd/system/vttl-league.service' < "$DIR/vttl-league.service"
$SSH "$HOST" 'systemctl daemon-reload && systemctl enable vttl-league >/dev/null 2>&1 && echo enabled'

echo "== $HOST: nginx site"
if $SSH "$HOST" "grep -q ssl_certificate /etc/nginx/sites-available/$DOMAIN 2>/dev/null"; then
  echo "site already has TLS (certbot changed it): not overwritten"
else
  $SSH "$HOST" "install -d /var/www/letsencrypt && cat > /etc/nginx/sites-available/$DOMAIN" < "$DIR/nginx-$DOMAIN.conf"
  $SSH "$HOST" "ln -sf /etc/nginx/sites-available/$DOMAIN /etc/nginx/sites-enabled/$DOMAIN && nginx -t 2>&1 | tail -1 && systemctl reload nginx"
fi

if [ "${1:-}" = "--tls" ]; then
  echo "== $HOST: certificate"
  IP=$($SSH "$HOST" 'curl -4 -s https://ifconfig.me')
  # Ask a public resolver: the local one can keep an old empty answer in its cache.
  DNS=$(dig +short "$DOMAIN" @1.1.1.1 | tail -1)
  if [ "$DNS" != "$IP" ]; then
    echo "DNS for $DOMAIN is '$DNS', server is $IP. Add an A record (Cloudflare: DNS only) first." >&2
    exit 1
  fi
  $SSH "$HOST" "certbot --nginx -d $DOMAIN --redirect --non-interactive --agree-tos --keep-until-expiring -m info@urbanhunt.be"
fi
echo "== done"

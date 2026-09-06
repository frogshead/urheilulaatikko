#!/usr/bin/env bash
#
# End-to-end smoke test against the docker-compose environment.
#
#   docker compose up -d
#   ./scripts/e2e.sh
#
# Logs in to Keycloak as the admin test user, provisions a TOTP secret for the
# regular test user, then logs in as that user and requests a PIN.
set -euo pipefail

cd "$(dirname "$0")/.."

if [ -f .env ]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env
  set +a
fi

KEYCLOAK_PORT="${KEYCLOAK_PORT:-8080}"
TOTP_SERVER_PORT="${TOTP_SERVER_PORT:-3000}"
KEYCLOAK="http://localhost:${KEYCLOAK_PORT}/realms/lock-demo"
TOTP="http://localhost:${TOTP_SERVER_PORT}"
: "${TEST_USER_PASSWORD:?set TEST_USER_PASSWORD in .env}"
: "${TEST_ADMIN_PASSWORD:?set TEST_ADMIN_PASSWORD in .env}"

json() { python3 -c "import sys,json;print(json.load(sys.stdin)$1)"; }

step() { printf '\n\033[1m==> %s\033[0m\n' "$1"; }

login() {
  curl -sf -X POST "${KEYCLOAK}/protocol/openid-connect/token" \
    -d grant_type=password -d client_id=lock-cli \
    -d "username=$1" --data-urlencode "password=$2" | json "['access_token']"
}

step "Waiting for /healthz"
for _ in $(seq 1 60); do
  if curl -sf "${TOTP}/healthz" >/dev/null; then break; fi
  sleep 2
done
curl -sf "${TOTP}/healthz" | python3 -m json.tool

step "Logging in as testadmin"
ADMIN_TOKEN="$(login testadmin "${TEST_ADMIN_PASSWORD}")"
echo "got an access token (${#ADMIN_TOKEN} chars)"

step "Looking up testuser's subject"
USER_TOKEN="$(login testuser "${TEST_USER_PASSWORD}")"
SUBJECT="$(curl -sf -u "totp-server:dev-secret-change-me" \
  -X POST "${KEYCLOAK}/protocol/openid-connect/token/introspect" \
  -d "token=${USER_TOKEN}" | json "['sub']")"
echo "subject: ${SUBJECT}"

step "Provisioning a TOTP secret for testuser (admin only, returned once)"
curl -sf -X POST "${TOTP}/v1/totp/provision" \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  -H 'Content-Type: application/json' \
  -d "{\"subject\":\"${SUBJECT}\"}" | python3 -m json.tool

step "Requesting a PIN as testuser"
curl -sf -X POST "${TOTP}/v1/totp/request" \
  -H "Authorization: Bearer ${USER_TOKEN}" | python3 -m json.tool

step "A request with no token must be rejected"
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST "${TOTP}/v1/totp/request")"
[ "$code" = "401" ] && echo "401 as expected" || { echo "expected 401, got $code"; exit 1; }

step "A non-admin must not be able to provision"
code="$(curl -s -o /dev/null -w '%{http_code}' -X POST "${TOTP}/v1/totp/provision" \
  -H "Authorization: Bearer ${USER_TOKEN}" -H 'Content-Type: application/json' \
  -d "{\"subject\":\"${SUBJECT}\"}")"
[ "$code" = "403" ] && echo "403 as expected" || { echo "expected 403, got $code"; exit 1; }

printf '\n\033[1;32mEnd-to-end flow OK\033[0m\n'

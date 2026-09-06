#!/bin/sh
# Renders the realm template into the import volume, substituting the test
# passwords from the environment.
#
# Keycloak does not expand `${env.VAR}` inside an imported realm file — it stores
# the placeholder verbatim as the password — so the substitution has to happen
# before the import. Doing it here keeps `docker compose up` a single command and
# keeps test credentials out of the repository.
set -eu

: "${TEST_USER_PASSWORD:?TEST_USER_PASSWORD must be set}"
: "${TEST_ADMIN_PASSWORD:?TEST_ADMIN_PASSWORD must be set}"

sed \
  -e "s|__TEST_USER_PASSWORD__|${TEST_USER_PASSWORD}|g" \
  -e "s|__TEST_ADMIN_PASSWORD__|${TEST_ADMIN_PASSWORD}|g" \
  /template/realm-export.template.json > /import/realm-export.json

echo "rendered realm import file"

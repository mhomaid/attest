#!/bin/sh
# The Garage image has no shell. This runs after the server is healthy and
# creates the buckets Compose expects, then grants the dev key.

set -eu
base="http://garage:3903"
auth="Authorization: Bearer ${GARAGE_ADMIN_TOKEN}"
key="${GARAGE_DEFAULT_ACCESS_KEY}"

i=0
until curl -sf "${base}/health" >/dev/null; do
  i=$((i + 1))
  if [ "$i" -gt 60 ]; then
    echo "garage admin API did not become ready" >&2
    exit 1
  fi
  sleep 1
done

allow() {
  name="$1"
  curl -s -o /tmp/create.json -w "%{http_code}" -X POST "${base}/v2/CreateBucket" \
    -H "${auth}" -H "Content-Type: application/json" \
    -d "{\"globalAlias\":\"${name}\"}" >/tmp/create.code || true
  info="$(curl -sf -G "${base}/v2/GetBucketInfo" -H "${auth}" --data-urlencode "globalAlias=${name}")"
  id="$(printf '%s' "${info}" | sed -n 's/.*"id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"
  if [ -z "${id}" ]; then
    echo "no bucket id for ${name}" >&2
    printf '%s\n' "${info}" >&2
    exit 1
  fi
  curl -sf -X POST "${base}/v2/AllowBucketKey" \
    -H "${auth}" -H "Content-Type: application/json" \
    -d "{\"bucketId\":\"${id}\",\"accessKeyId\":\"${key}\",\"permissions\":{\"read\":true,\"write\":true,\"owner\":true}}" \
    >/dev/null
  echo "bucket ${name} ready"
}

allow "${GARAGE_DEFAULT_BUCKET}"
allow attest-attestations
allow attest-cloudtrail
allow risingwave
echo "Garage buckets ready"

#!/usr/bin/env bash
# T-040: proves the Helm chart is hardened by default and that the production
# overlay example wires external dependencies by reference (no secret material).
# Needs only `helm`. Run from anywhere: `make helm-template` calls it.

set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
CHART="$ROOT/infra/helm/attest"
CI_VALUES="$CHART/ci-values.yaml"
PROD_EXAMPLE="$CHART/values-production.example.yaml"
failures=0

fail() {
  echo "FAIL: $*" >&2
  failures=$((failures + 1))
}

render() {
  helm template attest "$CHART" -n attest "$@"
}

count() { # count <pattern> <text>
  printf '%s\n' "$2" | grep -c -- "$1" || true
}

assert_has() { # assert_has <label> <pattern> <text>
  printf '%s\n' "$3" | grep -q -- "$2" || fail "$1: missing '$2'"
}

assert_lacks() { # assert_lacks <label> <pattern> <text>
  if printf '%s\n' "$3" | grep -q -- "$2"; then fail "$1: unexpected '$2'"; fi
}

# ── default render ────────────────────────────────────────────────────────
default_out="$(render -f "$CI_VALUES")" || { echo "default render failed" >&2; exit 1; }

deployments="$(count '^kind: Deployment$' "$default_out")"
pdbs="$(count '^kind: PodDisruptionBudget$' "$default_out")"
[[ "$deployments" -ge 1 ]] || fail "default: no Deployments rendered"
[[ "$pdbs" -eq "$deployments" ]] || fail "default: $pdbs PDBs for $deployments Deployments"

ro="$(count 'readOnlyRootFilesystem: true' "$default_out")"
[[ "$ro" -eq "$deployments" ]] || fail "default: readOnlyRootFilesystem on $ro of $deployments app containers"
nonroot="$(count 'runAsNonRoot: true' "$default_out")"
[[ "$nonroot" -eq "$deployments" ]] || fail "default: runAsNonRoot on $nonroot of $deployments pods"
assert_lacks default 'runAsUser: 0$' "$default_out"
assert_lacks default 'readOnlyRootFilesystem: false' "$default_out"
assert_lacks default 'privileged: true' "$default_out"
assert_lacks default 'allowPrivilegeEscalation: true' "$default_out"

# every app container has requests and limits
limits="$(count '^              memory: 512Mi$' "$default_out")"
[[ "$limits" -ge "$deployments" ]] || fail "default: resource limits missing on some containers"

# default deny: one policy that selects every pod in the release and has no rules
assert_has default 'name: attest-default-deny' "$default_out"
deny="$(printf '%s\n' "$default_out" | awk '/name: attest-default-deny/{f=1} f{print} /^---/{if(f) exit}')"
assert_has default-deny '- Ingress' "$deny"
assert_has default-deny '- Egress' "$deny"
assert_lacks default-deny '^  ingress:' "$deny"
assert_lacks default-deny '^  egress:' "$deny"
# explicit allows exist
assert_has default 'name: attest-allow-dns' "$default_out"
assert_has default 'name: attest-allow-release-internal' "$default_out"
assert_has default 'name: attest-allow-external-egress' "$default_out"

# bundled dev dependencies are off by default
assert_lacks default 'dev-postgres' "$default_out"
assert_lacks default 'dev-clickhouse' "$default_out"
assert_lacks default 'dev-redpanda' "$default_out"

# service auth and tool credential hooks survive
sa_out="$(render -f "$CI_VALUES" --set serviceAuth.existingSecret=s1 --set toolCredentials.existingSecret=s2)"
assert_has hooks 'secretName: s2' "$sa_out"
assert_has hooks 'name: ATTEST_SERVICE_AUTH_SECRET' "$sa_out"

# toggles work
off="$(render -f "$CI_VALUES" --set networkPolicy.enabled=false --set podDisruptionBudget.enabled=false)"
assert_lacks toggles '^kind: NetworkPolicy$' "$off"
assert_lacks toggles '^kind: PodDisruptionBudget$' "$off"

# PDB shape follows replica count
assert_has pdb-single 'maxUnavailable: 1' "$default_out"
multi="$(render -f "$CI_VALUES" --set replicaCount=2)"
assert_has pdb-multi 'minAvailable: 1' "$multi"

# ── values.yaml carries no secret material ────────────────────────────────
values="$(grep -v '^ *#' "$CHART/values.yaml")"
for key in accessKey secretKey signingKey; do
  printf '%s\n' "$values" | grep -E "^ *$key:" | grep -vqE ': *""' && fail "values.yaml: $key is not empty"
done
# auth.password is the sign-in feature flag ("off"), not a credential.
without_flag="$(printf '%s\n' "$values" | grep -v 'password: "off"')"
assert_lacks values 'password:' "$without_flag"
assert_lacks values 'postgres://[^ ]*:[^ @]*@' "$values"

# ── production overlay example ────────────────────────────────────────────
[[ -f "$PROD_EXAMPLE" ]] || { fail "missing $PROD_EXAMPLE"; echo "$failures failure(s)" >&2; exit 1; }
prod_out="$(render -f "$PROD_EXAMPLE")" || { echo "production example render failed" >&2; exit 1; }
assert_lacks prod '^kind: Secret$' "$prod_out"
assert_has prod 'KAFKA_SECURITY_PROTOCOL: "SASL_SSL"' "$prod_out"
assert_has prod 'KAFKA_SASL_MECHANISM: "AWS_MSK_IAM"' "$prod_out"
assert_has prod 'KAFKA_AWS_REGION:' "$prod_out"
assert_has prod 'name: DATABASE_URL' "$prod_out"
assert_has prod 'name: CLICKHOUSE_PASSWORD' "$prod_out"
assert_has prod 'eks.amazonaws.com/role-arn: arn:aws:iam::123456789012:role/' "$prod_out"
assert_has prod 'automountServiceAccountToken: false' "$prod_out"
assert_lacks prod 'dev-postgres' "$prod_out"
assert_lacks prod 'S3_ACCESS_KEY' "$prod_out"

# SCRAM variant reads credentials from a Secret by reference
scram="$(render -f "$PROD_EXAMPLE" --set kafka.saslMechanism=SCRAM-SHA-512 --set kafka.existingSecret=kafka-sasl)"
assert_has scram 'name: KAFKA_SASL_USERNAME' "$scram"
assert_has scram 'name: KAFKA_SASL_PASSWORD' "$scram"
assert_lacks scram '^kind: Secret$' "$scram"

# ── bundled dev dependencies, when asked for ──────────────────────────────
dev="$(render -f "$CI_VALUES" \
  --set devDependencies.postgres.enabled=true \
  --set devDependencies.clickhouse.enabled=true \
  --set devDependencies.redpanda.enabled=true)"
assert_has dev 'name: attest-dev-postgres' "$dev"
assert_has dev 'name: attest-dev-clickhouse' "$dev"
assert_has dev 'name: attest-dev-redpanda' "$dev"
assert_has dev 'KAFKA_BROKERS: "attest-dev-redpanda:9092"' "$dev"

# negative: a hardened default never renders a root pod or writable root FS
assert_lacks dev 'runAsUser: 0$' "$dev"

if [[ "$failures" -gt 0 ]]; then
  echo "$failures failure(s)" >&2
  exit 1
fi
echo "ok: helm hardening checks passed (deployments=$deployments pdbs=$pdbs)"

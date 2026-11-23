# Overlay template

Copy this directory into a **private** repository. Replace every `example.com`
host, secret name, and placeholder id there. Do not copy those private values
back into the public Attest repo.

```bash
cp -R examples/overlay /path/to/private-overlay
cd /path/to/private-overlay
```

## Local proof, before any production host

From the public repo:

```bash
make dev-up-all
make dev-up-live
```

Then point the workbench at this directory's `.env.example` (copy it to `.env`
in the private repo and fill the blanks). That starts:

| Piece | Local process |
| --- | --- |
| Identity provider | Keycloak, realm `example`, http://localhost:8180 |
| SIEM | Graylog, http://localhost:9000 |
| Edge | `edge-logpush` posts one firewall event and one SIEM-shaped alert |

Create a Graylog API token and set `ATTEST_GRAYLOG_TOKEN` in the private `.env`.
The investigator search does nothing useful until that token exists.

## Production

1. Copy `infra/helm/attest/values-production.example.yaml` to `values.yaml` in
   the private repo. Replace the example hosts and secret names.
2. Edit `policy.yaml`. The placeholders below are not a real list. Put the
   users, accounts, IPs, ASNs, and tenants that agents must not touch.
3. Edit `field-map.yaml`. `from` is the field name in your logs. `to` is the
   Attest name. Anything else is kept under `unmapped.<prefix>`.
4. Create the Kubernetes Secrets named in `values.yaml` out of band.
   `ATTEST_AUTH_PASSWORD` stays `off`.
5. Follow `docs/17_Deploy_Single_Org.md` for shadow mode, go-live, and rollback.

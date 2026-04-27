# Attest

Attest is a streaming-first, agent-aware security operations platform. The docs in `docs/` are the source of truth for product, architecture, stack, build order, and repository layout.

## Start Here

- Product and architecture overview: `docs/README.md`
- Canonical tech stack: `docs/07_Stack_Revised.md`
- Implementation sequence: `docs/10_Build_Order.md`
- Repository layout: `docs/11_Repo_Structure.md`
- Workbench spec: `docs/12_Workbench.md`

## Local Development

The first foundation target is:

```sh
make dev-up && make smoke
```

That command is intentionally not complete yet. The initial scaffold creates the monorepo shape, workspace files, and service slots that the implementation phases will fill in.

The JavaScript workspace uses Bun, matching `docs/12_Workbench.md`. Python commands run through `uv` so tests and ML tooling do not depend on globally installed packages.

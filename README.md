# Exchequer

A self-hosted accounting app for households that bank in the US: import the CSV and
OFX/QFX/QBO files your banks export, label each transaction by who it was for, and settle up
between each person and the household's shared money every month.

- A Rust REST server over PostgreSQL. It runs no cron jobs and does nothing until a request
  arrives.
- Accounts hold several currencies and securities at once, with exact decimal arithmetic
  everywhere (no floating point).
- Clients are a Python CLI/TUI and a SvelteKit web app, both generated from the OpenAPI spec.

**Status:** design phase. See [`docs/requirements.md`](docs/requirements.md) and
[`docs/design.md`](docs/design.md).

## License

BSD-2-Clause. See [LICENSE](LICENSE).

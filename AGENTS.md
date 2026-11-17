# solana-programs

LI.FI's Solana programs and their TypeScript bindings, published to npm as `@lifi/solana-programs`.
Today there is one program, `lifi-solana-program-1`: its `TrackV1` instruction logs a LI.FI
transaction id on-chain (`LI.FI TX: 0x...`) so the backend can track transactions it generated.

## Ownership

API Expansion squad, reviewer Daniela (`@melianessa`). Review channel: `#dev-sc-review`.

## Layout

| Path                              | What lives there                                                                                                                 |
| --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `index.ts`                        | Package entry; re-exports each program's bindings (`lifiSolanaProgram1`)                                                         |
| `lifi-solana-program-1/program/`  | Rust program (`solana-program` 2.0.3, borsh): `src/processor.rs`, `src/instructions/track_v1.rs`, `src/utils.rs`, `src/error.rs` |
| `lifi-solana-program-1/ts/`       | TypeScript bindings: `PROGRAM_ADDRESS`, instruction data, `trackv1/` builder and PDA derivation, vitest tests                    |
| `lifi-solana-program-1/README.md` | Program docs: instruction inputs, checks, build and test commands                                                                |
| `.github/workflows/publish.yaml`  | On a `v*` tag: GitHub release, then build, test and `pnpm publish` (npm trusted publishing)                                      |

New programs follow the same split: `<program>/program` for Rust, `<program>/ts` for bindings, plus an
export line in `index.ts`.

## Commands

Node from `.nvmrc` (26), pnpm 9 (`packageManager` in package.json).

```bash
pnpm install
pnpm build          # clean, tsc to dist/_cjs, _esm, _types (gitignored); also runs oxlint --fix and oxfmt on the repo
pnpm test           # vitest (watch mode locally; `pnpm test:affected` for a single run since the last run)
pnpm lint           # oxlint
pnpm format:check   # oxfmt --check
```

Rust program (from `lifi-solana-program-1/`, needs the Solana CLI for `cargo build-sbf`):

```bash
cargo build-sbf --manifest-path=./program/Cargo.toml --sbf-out-dir=./program/target/so
cargo test --manifest-path=./program/Cargo.toml
solana-verify build ./program   # verifiable build
```

No env file: neither the bindings nor the program read environment variables.

## Conventions

- Releases use `standard-version` (`pnpm release`, `release:alpha`, `release:beta`), which bumps the
  version and `CHANGELOG.md`; pushing the tag triggers the publish workflow.
- Commit messages are conventional (`fix(deps): ...`, `chore: ...`), since `standard-version` builds the
  changelog from them.
- The program address in `lifi-solana-program-1/ts/index.ts` is the deployed mainnet and devnet program;
  a change to the Rust program means a redeploy, which the owner decides.
- Formatting is oxfmt and linting oxlint (no Prettier or ESLint).

## Deeper docs

- [README.md](README.md)
- [lifi-solana-program-1/README.md](lifi-solana-program-1/README.md)

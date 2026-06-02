# intent-factory-program

Solana program for intent-constrained execution from a PDA vault.

It exposes two instructions:

- `execute` (`variant = 0`): run one or more CPIs, enforce transfer outcomes, then close/sweep the source vault.
- `refund` (`variant = 1`): return vault funds to the funder and close the vault path.

Built with `solana-program` (no Anchor).

---

## Program overview

### Instruction summary

| Variant | Instruction | File                          | Purpose                                                                     |
| ------- | ----------- | ----------------------------- | --------------------------------------------------------------------------- |
| `0`     | `execute`   | `src/instructions/execute.rs` | Run CPI route, enforce `min` deltas, require source drained, cleanup vault. |
| `1`     | `refund`    | `src/instructions/refund.rs`  | Re-derive same intent PDA and return funds to funder.                       |

### Shared intent/PDA model

Both instructions bind to the same intent hash (see `src/intent_hash.rs`):

- PDA seeds: `[INTENT_PDA_SEED, intent_hash]`
- `INTENT_PDA_SEED = b"intent"`
- preimage domain separator: `b"isv1"`

Hashed fields:

- `funder`
- `mint` (`SystemProgram` for SOL, or `ta.mint` from canonical source ATA for SPL)
- `amount_in`
- `salt`
- `executor`
- `transfer_nb`
- `transfer_destinations[]`
- `transfer_min_amounts[]`

CPI payload bytes are **not** hashed.

---

## `execute` (`variant = 0`)

### High-level flow

1. Parse data prefix (`amount_in`, `salt`, `transfer_nb`, `mins[]`, `cpi_count`).
2. Parse account header and source type (SOL or SPL ATA source).
3. Rebuild intent hash and verify PDA.
4. Snapshot transfer-check destination balances.
5. Execute `cpi_count` CPIs via `invoke_signed`.
6. Enforce postconditions:
   - source drained (`SOL`: PDA at rent minimum, `SPL`: source ATA amount `0`)
   - each destination delta `>= min[i]`
7. Cleanup:
   - `SPL`: close source ATA to executor
   - `SOL`: sweep PDA lamports to executor

Postcondition caveat: see [Known limitation: dust griefing on `execute`](#known-limitation-dust-griefing-on-execute).

### `execute` instruction data layout (`rest` after variant byte)

| Offset | Size              | Field                    | Notes                      |
| ------ | ----------------- | ------------------------ | -------------------------- |
| `0`    | `8`               | `amount_in`              | `u64` LE                   |
| `8`    | `32`              | `salt`                   | bytes                      |
| `40`   | `1`               | `transfer_nb`            | `1..=8`                    |
| `41`   | `8 * transfer_nb` | `transfer_min_amounts[]` | `u64` LE each              |
| next   | `1`               | `cpi_count`              | `1..=16`                   |
| next   | variable          | CPI blocks               | repeated `cpi_count` times |

No destination pubkeys in `execute` data; transfer-check destinations are in accounts.

#### One CPI block

| Field            | Size                 | Notes                                                        |
| ---------------- | -------------------- | ------------------------------------------------------------ |
| `acc_count`      | `1`                  | `1..=128`; CPI slice length (`0` index is callee program id) |
| `override_count` | `1`                  | `0..=32`                                                     |
| overrides        | `2 * override_count` | repeated `(pos, flags)`                                      |
| `inner_data_len` | `2`                  | `u16` LE                                                     |
| `inner_data`     | `inner_data_len`     | opaque bytes for callee                                      |

Trailing bytes after last CPI block are rejected (`InvalidInstructionData`).

### CPI safety model

- No CPI to this program (`CpiToSelfNotAllowed`).
- Denylist blocks legacy and upgradeable BPF loaders (`CpiProgramDenied`).
- Inner signer override allowed only for `executor` or `pda` (`InnerSignerNotAllowed`).
- Writable escalation above outer metas is forbidden (`WritableEscalationNotAllowed`).

---

## `refund` (`variant = 1`)

### High-level flow

1. Parse full intent preimage from data (`amount_in`, `salt`, `transfer_nb`, dest+min pairs).
2. Parse account header and source type (SOL or SPL ATA source).
3. Rebuild intent hash and verify PDA.
4. Return funds:
   - `SOL`: close/sweep PDA to `funder`
   - `SPL`: ensure canonical `funder_ata`, create idempotently if needed, transfer full source ATA balance to `funder_ata`
5. Cleanup:
   - `SPL`: close source ATA (rent recipient = `executor`)

### `refund` instruction data layout (`rest` after variant byte)

| Offset | Size     | Field                       | Notes                      |
| ------ | -------- | --------------------------- | -------------------------- |
| `0`    | `8`      | `amount_in`                 | `u64` LE                   |
| `8`    | `32`     | `salt`                      | bytes                      |
| `40`   | `1`      | `transfer_nb`               | `1..=8`                    |
| `41`   | repeated | `(dest_pubkey, min_amount)` | each pair = `32 + 8` bytes |

Exact-length parsing: trailing bytes are rejected (`InvalidInstructionData`).

### Refund accounts

Header:

1. `executor` (signer)
2. `pda` (writable)
3. `funder` (SOL path writable; SPL path readonly owner of destination ATA)
4. `from_program` (`SystemProgram` or `spl_token` / `spl_token_2022`)

SPL-only tail:

5. `pda_ata` (writable source ATA)
6. `funder_ata` (writable destination ATA)
7. `mint` (readonly)
8. `system_program` (readonly, ATA create CPI)
9. `associated_token_program` (readonly)

---

## Errors

Custom errors (`ProgramError::Custom(N)`):

| `N`  | Variant                        | Meaning                                                                        |
| ---- | ------------------------------ | ------------------------------------------------------------------------------ |
| `0`  | `InvalidPda`                   | Derived PDA mismatch                                                           |
| `1`  | `MinAmountNotMet`              | `execute`: destination delta below min                                         |
| `2`  | `WrongTokenProgram`            | `from_program` is not System / SPL Token / SPL Token-2022                      |
| `3`  | `InvalidTransferNb`            | `transfer_nb == 0` or `> 8`                                                    |
| `4`  | `InsufficientData`             | Truncated instruction data                                                     |
| `5`  | `NotEnoughAccounts`            | Account list too short                                                         |
| `6`  | `FromAtaNotEmpty`              | `execute`: SPL source ATA not fully drained after CPIs                         |
| `8`  | `InvalidCpiCount`              | `execute`: `cpi_count == 0` or `> 16`                                          |
| `9`  | `InvalidCpiAccountSpec`        | `execute`: invalid CPI `acc_count` / `override_count` / override position      |
| `10` | `CpiToSelfNotAllowed`          | `execute`: inner CPI targets this program                                      |
| `11` | `CpiProgramDenied`             | `execute`: inner CPI target is on denylist                                     |
| `12` | `InnerSignerNotAllowed`        | `execute`: unauthorized inner signer escalation                                |
| `13` | `WritableEscalationNotAllowed` | `execute`: unauthorized writable escalation                                    |
| `14` | `SolSourcePdaNotDrained`       | `execute`: SOL PDA lamports after CPIs not equal to `Rent::minimum_balance(0)` |
| `15` | `InvalidSourceAta`             | SPL source ATA is not canonical/valid for `(pda, mint, token_program)`         |
| `16` | `InvalidDestinationAta`        | `refund`: destination ATA/mint mismatch for funder                             |

Code `7` is reserved.

Non-custom Solana errors used:

- `ProgramError::MissingRequiredSignature` (executor must sign)
- `ProgramError::InvalidInstructionData` (invalid variant, trailing bytes, malformed fixed layout)

---

## Known limitation: dust griefing on `execute`

`execute` is fail-closed on strict source-drain checks:

- SOL source: PDA must end at exactly `Rent::minimum_balance(0)`.
- SPL source: source ATA token amount must end at exactly `0`.

Because PDA/ATA addresses are deterministic and public, third parties can send dust to the source before `execute`, causing otherwise-valid execution to fail these invariants.

This is a liveness/griefing tradeoff, not a direct fund-loss vector. Funds remain recoverable through `refund`:

- SOL refund sweeps full current PDA lamports to funder.
- SPL refund transfers full current source ATA token balance to `funder_ata` (source ATA close rent follows configured recipient policy).

---

## Build

### a) What you need

This crate targets the **Solana 2.3** dependency line (`solana-program = "2.3"`, `solana-program-test = "2.3"`, edition 2021). Two independent toolchains are involved, each used for a different task:

| Toolchain                              | Used by                                                          | Required version                                                                  |
| -------------------------------------- | ---------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Host Rust (`rustc` / `cargo`)          | `cargo test` (host-side `solana-program-test` integration tests) | `1.84+` (validated on `1.86.0`)                                                   |
| Agave/Solana CLI (`cargo build-sbf`)   | building the on-chain `.so`                                      | `solana-cli 2.1+` (validated with `solana-cargo-build-sbf 2.1.21`, platform-tools `v1.43`, rustc `1.79`) |

Key points:

- The integration tests run the program **in-process** (`processor!`), so `cargo test` only needs **host Rust** — no `.so` build, no Agave CLI.
- `cargo build-sbf` does **not** use your host Rust toolchain — it uses the `rustc` bundled in its platform-tools. Verify with `cargo build-sbf --version`.
- Do not upgrade this crate to the Solana 3.x/4.x crate line without also moving the CLI to a platform-tools build whose `rustc` supports those crates (the 3.0 line pulls `edition2024` transitive deps that need `rustc ≥ 1.85`).

### b) How to get there

**From scratch.** Install the two toolchains:

```bash
# Host Rust (rustup)
rustup toolchain install 1.86.0

# Agave/Solana CLI (provides cargo build-sbf)
agave-install init 2.1.21          # or: solana-install init 2.1.21
```

**If you already have other versions installed.** Host Rust and the Agave CLI are selected independently:

```bash
# Host Rust (affects `cargo test`). Prefer a folder-local pin so other repos are untouched:
#   Option A — create rust-toolchain.toml in this crate:
#     [toolchain]
#     channel = "1.86.0"
#   Option B — one-off override for this directory:
rustup override set 1.86.0         # rustup override unset  # to remove later

# Agave/Solana CLI (affects `cargo build-sbf`). This sets the *globally active*
# release for your machine, so re-init it when you come back to this crate:
agave-install init 2.1.21          # or: solana-install init 2.1.21
```

Verify both before building:

```bash
rustc --version                    # expect 1.84+ (e.g. 1.86.0)
cargo build-sbf --version          # expect solana-cargo-build-sbf 2.1.21 / platform-tools v1.43 / rustc 1.79
```

### Commands

```bash
# Build the on-chain program
cargo build-sbf

# Run unit + integration tests (host Rust only)
cargo test
```

Note: keep `Cargo.lock` compatible with the Solana SBF toolchain; updating transitive crates blindly can break SBF builds due to Rust/toolchain version drift.

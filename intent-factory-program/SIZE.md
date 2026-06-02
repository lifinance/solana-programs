# Size Notes — `intent-factory-program`

This document describes transaction-byte sizing for **this program only**:

- `execute` (`variant = 0`)
- `refund` (`variant = 1`)

Primary focus is `execute` because CPI routing makes it the tight path.

Solana v0 packet hard limit: **1232 bytes**.

---

## Constants that bound size

From `src/const.rs`:

- `MAX_TRANSFER_NB = 8`
- `MAX_CPI_NB = 16`
- `MAX_ACC_PER_CPI = 128`
- `MAX_OVERRIDE_PER_CPI = 32`

---

## `execute` instruction bytes

### Exact `ix.data` layout

Instruction data is `variant(1)` + `rest`:

- `amount_in`: `8`
- `salt`: `32`
- `transfer_nb`: `1`
- `transfer_min_amounts`: `8 * transfer_nb`
- `cpi_count`: `1`
- for each CPI:
  - `acc_count`: `1`
  - `override_count`: `1`
  - overrides: `2 * override_count` (`pos`, `flags`)
  - `inner_data_len`: `2`
  - `inner_data`: `inner_data_len`

So:

`execute_ix_data_len = 43 + 8T + Σ(4 + 2Vj + Dj)`

where:

- `T = transfer_nb`
- `Vj = override_count` of CPI `j`
- `Dj = inner_data_len` of CPI `j`

Single-CPI form:

`execute_ix_data_len = 47 + 8T + 2V + D`

Typical baseline (`T=1`, `V=1`, one swap CPI):

`execute_ix_data_len = 57 + D`

### `execute` account-metas count

For one `execute` instruction:

`N_accounts = 4 + I_spl + T + Σ(acc_count_j)`

- header `4` = `executor`, `pda`, `funder`, `from_program`
- `I_spl = 1` for SPL (`pda_ata`), else `0` for SOL
- `T` transfer-check destination accounts
- each CPI contributes `acc_count_j` contiguous accounts (`0` index is inner callee program id)

### Swap-CPI budget framing

For one swap CPI (`j=1`) with:

- `K = acc_count_1` (swap program id + swap accounts)
- `V = override_count_1`
- `D = inner_data_len_1`

the `execute`-owned bytes that scale with swap routing are:

- account indices in outer `ix.accounts`: `+K`
- CPI wrapper fields in `ix.data`: `+(4 + 2V + D)`
- plus key-storage cost for any unique swap pubkeys not already present:
  - static key path: `~32` bytes per new key
  - lookup-table path: `~1` lookup index byte per key (+ ALT entry overhead)

Rule of thumb:

- without per-tx ALT for swap keys, each new swap key usually costs ~`33` tx bytes (`32` key + `1` account index)
- with per-tx ALT, each new swap key is usually ~`2` tx bytes (`1` lookup index + `1` account index), plus one-time ALT overhead

---

## `refund` instruction bytes

### Exact `ix.data` layout

Instruction data is `variant(1)` +:

- `amount_in`: `8`
- `salt`: `32`
- `transfer_nb`: `1`
- repeated `transfer_nb` times:
  - `dest_pubkey`: `32`
  - `min_amount`: `8`

So:

`refund_ix_data_len = 42 + 40T`

where `T = transfer_nb`.

Typical baseline (`T=1`):

`refund_ix_data_len = 82`

### `refund` account-metas count

- SOL path: `4` accounts
  - `executor`, `pda`, `funder`, `from_program`
- SPL path: `9` accounts
  - SOL header `4` + `pda_ata`, `funder_ata`, `mint`, `system_program`, `associated_token_program`

`refund` does not carry CPI route bytes, so sizing is much more stable.

---

## Tx-envelope bytes (independent of program logic)

For the common single-signer v0 flow with both compute-budget ixs:

- signatures shortvec + one signature: `65`
- v0 marker + message header: `4`
- recent blockhash: `32`
- shortvec headers for account keys / instructions / ALTs: usually `3`
- `SetComputeUnitLimit`: `8`
- `SetComputeUnitPrice`: `12`
- `ComputeBudget` program static key: `32`

Baseline envelope subtotal: about **156 bytes** before counting your program id, program accounts, lookup-table entries, and your instruction data.

---

## Worked baseline (execute, one swap CPI)

Assume:

- one signer (executor/fee payer)
- one transfer check destination (`T=1`)
- one CPI (`cpi_count=1`)
- one signer override for PDA (`V=1`)
- swap payload size `D`

Then:

- `execute_ix_data_len = 57 + D`
- account metas contributed by `execute`:
  - SPL: `6 + K` (`4` header + `pda_ata` + `dest` + CPI slice `K`)
  - SOL: `5 + K`

This is the shape to plug into your serializer/binary-search script when validating real routes against the `1232`-byte limit.

---

## Practical guidance

1. Always size with actual serialized v0 transactions (`VersionedTransaction.serialize()`).
2. For medium/large routes, assume a per-tx ALT for swap keys; no-ALT routes run out of bytes quickly.
3. Keep `transfer_nb` and `cpi_count` as low as possible; both increase account and/or data bytes.
4. `refund` is rarely the byte bottleneck; optimize `execute` first.

---

## Methodology

For production sizing, use binary search over swap payload bytes and account count with your exact account list + ALT layout, then assert serialized length `< 1232`.

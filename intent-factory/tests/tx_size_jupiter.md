# Jupiter Transaction Size Measurements

These tests measure how large production-shaped intent factory execute transactions become when the inner calls are built from fresh Jupiter swap instructions. The goal is to understand which Jupiter routes fit inside Solana's transaction size limit, and how much ALTs and peripheral instructions affect the result.

## Usage

Set the required environment values in `.env`:

```bash
JUPITER_API_KEY=...
SOLANA_RPC_URL=...
```

Run all Jupiter measurement scenarios:

```bash
npx vitest run tests/tx_size_jupiter.spec.ts
```

Run only selected scenarios with `JUPITER_SCENARIOS`:

```bash
JUPITER_SCENARIOS=no_alt npx vitest run tests/tx_size_jupiter.spec.ts
JUPITER_SCENARIOS=with_alt npx vitest run tests/tx_size_jupiter.spec.ts
JUPITER_SCENARIOS=no_alt,with_alt_peripherals npx vitest run tests/tx_size_jupiter.spec.ts
```

Valid scenario keys:

- `no_alt`
- `no_alt_peripherals`
- `with_alt`
- `with_alt_peripherals`
- `jito_no_alt`
- `jito_no_alt_peripherals`
- `jito_with_alt`
- `jito_with_alt_peripherals`

The `jito_*` variants drop the `SetComputeUnitPrice` instruction from the measured transaction to model a Jito-routed flow; the Jito tip is paid as a separate `SystemProgram.transfer` composed in a setup instruction outside this test, so it is intentionally not part of the measured size.

When `JUPITER_SCENARIOS` is empty, all scenarios run. Results are written to `tests/fixtures/tx_size_jupiter.json`.

## Structure

Each test run fetches fresh Jupiter quotes for a matrix of `maxAccounts` values. This lets us compare route sizes under different account constraints.

For each quote, the test requests Jupiter swap instructions and separates them into outer and inner instructions. This split is measurement/integrator logic, not SDK logic:

- Compute budget instructions stay as outer transaction instructions because they configure the whole transaction.
- Setup and cleanup instructions are treated as peripherals because they often require wallet-owned signer behavior or token account lifecycle work. These can be measured either outside the intent execution path or included in the same transaction.
- Swap and other core Jupiter instructions become inner CPI calls executed by the intent factory program.

The test maps Jupiter placeholder accounts to intent factory fixed slots, then uses the SDK to convert inner `TransactionInstruction`s into symbolic instructions. The SDK also builds the core `execute_intent` instruction via `buildExecuteIntentIx()`, including PDA derivation, source ATA and receiver token derivation, header/call encoding, and account ordering. The test then composes the outer instructions plus the SDK-built execute instruction and serializes a v0 transaction for measurement.

ALT scenarios load the real Jupiter lookup tables from the configured Solana RPC, then add the common intent factory ALT used by the measurement transaction.

## Mocks And Differences

The measurements serialize real Solana v0 transactions, but they do not submit transactions on-chain.

Some accounts are generated with `PublicKey.unique()` instead of live user accounts, and the program id is a test-only public key. The recent blockhash is also a deterministic placeholder. These values preserve serialized byte shape, but they are not executable on mainnet.

Jupiter quotes, Jupiter instructions, and Jupiter ALT contents are production data when the live API path is enabled. The intent factory instruction construction is delegated to the SDK, matching the production client flow, but the tests are focused on transaction size, not execution success.

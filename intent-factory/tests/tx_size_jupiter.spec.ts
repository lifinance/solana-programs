import { describe, it, expect, beforeAll, afterAll } from "vitest"
import {
  PublicKey,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  AddressLookupTableAccount,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
  Connection,
} from "@solana/web3.js"
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token"
import { writeFileSync, readFileSync } from "fs"
import { resolve } from "path"

import {
  FixedSlot,
  buildExecuteIntentIx,
  convertToSymbolicIx,
} from "../ts-client/src/index.js"
import type { IntentHeader, SymbolicInstruction } from "../ts-client/src/index.js"

import { loadTestEnv, requireJupiterApiKey, requireSolanaRpcUrl } from "./env.js"
import type { TestEnv } from "./env.js"

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

const LOAD_JUPITER_FROM_API = true

const SOLANA_TX_LIMIT = 1232

const PROGRAM_ID = PublicKey.unique()
const PAYER = PublicKey.unique()

const USDC_MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
const USDT_MINT = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB"
const SOL_MINT = "So11111111111111111111111111111111111111112"

const COMPUTE_BUDGET_PROGRAM_ID = new PublicKey(
  "ComputeBudget111111111111111111111111111111",
)
const SET_COMPUTE_UNIT_PRICE_DISC = 0x03

const MAX_ACCOUNTS_MATRIX = [64, 48, 40, 32, 24, 16]

const ALL_SCENARIOS = [
  "no_alt",
  "no_alt_peripherals",
  "with_alt",
  "with_alt_peripherals",
  "jito_no_alt",
  "jito_no_alt_peripherals",
  "jito_with_alt",
  "jito_with_alt_peripherals",
] as const

type ScenarioKey = (typeof ALL_SCENARIOS)[number]

function activeScenarios(env: TestEnv): Set<ScenarioKey> {
  if (env.jupiterScenarios.length === 0) return new Set(ALL_SCENARIOS)
  return new Set(
    env.jupiterScenarios.filter((s): s is ScenarioKey =>
      (ALL_SCENARIOS as readonly string[]).includes(s),
    ),
  )
}

// ---------------------------------------------------------------------------
// Jupiter API types (local mirror of response shape)
// ---------------------------------------------------------------------------

interface JupiterAccountMeta {
  pubkey: string
  isSigner: boolean
  isWritable: boolean
}

interface JupiterInstruction {
  programId: string
  accounts: JupiterAccountMeta[]
  data: string
}

interface JupiterSwapInstructionsResponse {
  computeBudgetInstructions: JupiterInstruction[]
  setupInstructions: JupiterInstruction[]
  swapInstruction: JupiterInstruction
  cleanupInstruction?: JupiterInstruction | null
  otherInstructions?: JupiterInstruction[]
  tokenLedgerInstruction?: JupiterInstruction | null
  addressLookupTableAddresses: string[]
}

interface JupiterQuoteResponse {
  inputMint: string
  inAmount: string
  outputMint: string
  outAmount: string
  slippageBps: number
  routePlan: unknown[]
  [key: string]: unknown
}

interface JupiterRouteFixture {
  quote: JupiterQuoteResponse
  instructions: JupiterSwapInstructionsResponse
  placeholderTaker: string
  inputMint: string
  outputMint: string
  amount: string
  maxAccounts: number
}

type JupiterFixtureMap = Record<string, JupiterRouteFixture | null>

// ---------------------------------------------------------------------------
// Partitioned instruction groups
// ---------------------------------------------------------------------------

interface PartitionedIxs {
  innerCpiIxs: SymbolicInstruction[]
  peripheralIxs: TransactionInstruction[]
  setupIxCount: number
  cleanupIxCount: number
  computeBudgetIxs: TransactionInstruction[]
  jupiterAltAddresses: string[]
}

// ---------------------------------------------------------------------------
// Scenario result type (extends deterministic suite's shape)
// ---------------------------------------------------------------------------

interface JupiterScenarioResult {
  name: string
  kind: "execute"
  maxAccounts: number
  calls: number
  namedAccounts: number
  tailAccounts: number
  signatures: number
  headerBytes: number
  callsBytes: number
  messageBytes: number
  transactionBytes: number
  usesAlt: boolean
  altAddressCount: number
  setupIxCount: number
  cleanupIxCount: number
  computeBudgetIxCount: number
  jupiterAltTableCount: number
  outerIxCount: number
  jitoMode: boolean
  fits: boolean
  classification: "fits" | "phase-2 pressure" | "blocker"
}

const results: JupiterScenarioResult[] = []

// ---------------------------------------------------------------------------
// Jupiter API client
// ---------------------------------------------------------------------------

async function fetchJupiterQuote(
  apiUrl: string,
  apiKey: string,
  inputMint: string,
  outputMint: string,
  amount: string,
  maxAccounts: number,
): Promise<JupiterQuoteResponse | null> {
  const params = new URLSearchParams({
    inputMint,
    outputMint,
    amount,
    slippageBps: "50",
    maxAccounts: String(maxAccounts),
  })

  const res = await fetch(`${apiUrl}/swap/v1/quote?${params}`, {
    headers: { "x-api-key": apiKey },
  })

  if (!res.ok) {
    if (res.status === 400 || res.status === 404) return null
    throw new Error(`Jupiter quote failed: ${res.status} ${await res.text()}`)
  }

  return res.json() as Promise<JupiterQuoteResponse>
}

async function fetchJupiterSwapInstructions(
  apiUrl: string,
  apiKey: string,
  userPublicKey: string,
  quote: JupiterQuoteResponse,
): Promise<JupiterSwapInstructionsResponse> {
  const res = await fetch(`${apiUrl}/swap/v1/swap-instructions`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-api-key": apiKey,
    },
    body: JSON.stringify({
      userPublicKey,
      quoteResponse: quote,
      dynamicComputeUnitLimit: true,
      prioritizationFeeLamports: { autoMultiplier: 2 },
    }),
  })

  if (!res.ok) {
    throw new Error(`Jupiter swap-instructions failed: ${res.status} ${await res.text()}`)
  }

  return res.json() as Promise<JupiterSwapInstructionsResponse>
}

// ---------------------------------------------------------------------------
// Route loader (API or fixture)
// ---------------------------------------------------------------------------

async function loadJupiterRoute(
  maxAccounts: number,
  apiUrl: string,
  apiKey: string,
  placeholderTaker: string,
): Promise<JupiterRouteFixture | null> {
  const quote = await fetchJupiterQuote(
    apiUrl,
    apiKey,
    USDC_MINT,
    USDT_MINT,
    "1000000",
    maxAccounts,
  )

  if (!quote) return null

  const instructions = await fetchJupiterSwapInstructions(
    apiUrl,
    apiKey,
    placeholderTaker,
    quote,
  )

  return {
    quote,
    instructions,
    placeholderTaker,
    inputMint: USDC_MINT,
    outputMint: USDT_MINT,
    amount: "1000000",
    maxAccounts,
  }
}

function loadFixtureMap(): JupiterFixtureMap {
  const fixturePath = resolve(__dirname, "fixtures/jupiter_route.json")
  const raw = readFileSync(fixturePath, "utf-8")
  return JSON.parse(raw) as JupiterFixtureMap
}

// ---------------------------------------------------------------------------
// Jupiter IX → TransactionInstruction
// ---------------------------------------------------------------------------

function jupiterIxToTransactionIx(jix: JupiterInstruction): TransactionInstruction {
  return new TransactionInstruction({
    programId: new PublicKey(jix.programId),
    keys: jix.accounts.map((a) => ({
      pubkey: new PublicKey(a.pubkey),
      isSigner: a.isSigner,
      isWritable: a.isWritable,
    })),
    data: Buffer.from(jix.data, "base64"),
  })
}

/**
 * Drop SetComputeUnitPrice (discriminator 0x03) from a list of compute-budget
 * instructions. Models Jito-routed transactions, which use a separate tip
 * transfer instead of priority fees.
 */
function stripCuPrice(ixs: TransactionInstruction[]): TransactionInstruction[] {
  return ixs.filter((ix) => {
    if (!ix.programId.equals(COMPUTE_BUDGET_PROGRAM_ID)) return true
    if (ix.data.length === 0) return true
    return ix.data[0] !== SET_COMPUTE_UNIT_PRICE_DISC
  })
}

// ---------------------------------------------------------------------------
// Jupiter account map for SDK convertToSymbolicIx
// ---------------------------------------------------------------------------

function buildJupiterAccountMap(
  placeholderTaker: string,
  sourceAtaPubkey: string,
  receiverTokenPubkey: string,
  receiverPubkey: string,
): Map<string, PublicKey> {
  return new Map([
    [placeholderTaker, FixedSlot.IntentPda],
    [sourceAtaPubkey, FixedSlot.SourceAta],
    [receiverTokenPubkey, FixedSlot.ReceiverToken],
    [receiverPubkey, FixedSlot.Receiver],
  ])
}

function jupiterInnerToSymbolicIx(
  jix: JupiterInstruction,
  accountMap: Map<string, PublicKey>,
) {
  return convertToSymbolicIx(jupiterIxToTransactionIx(jix), {
    accountMap,
    signerPolicy: "intent-pda-only",
  })
}

// ---------------------------------------------------------------------------
// IX partitioner
// ---------------------------------------------------------------------------

function partitionJupiterIxs(
  fixture: JupiterRouteFixture,
  sourceAtaPubkey: string,
  receiverTokenPubkey: string,
  receiverPubkey: string,
): PartitionedIxs {
  const { instructions, placeholderTaker } = fixture

  const accountMap = buildJupiterAccountMap(
    placeholderTaker,
    sourceAtaPubkey,
    receiverTokenPubkey,
    receiverPubkey,
  )

  const innerCpiRaw: JupiterInstruction[] = [instructions.swapInstruction]
  if (instructions.otherInstructions) {
    innerCpiRaw.push(...instructions.otherInstructions)
  }

  const innerCpiIxs = innerCpiRaw.map((jix) =>
    jupiterInnerToSymbolicIx(jix, accountMap),
  )

  const setupIxCount = instructions.setupInstructions.length
  const cleanupIxCount = instructions.cleanupInstruction ? 1 : 0

  const peripheralRaw: JupiterInstruction[] = [...instructions.setupInstructions]
  if (instructions.cleanupInstruction) {
    peripheralRaw.push(instructions.cleanupInstruction)
  }
  const peripheralIxs = peripheralRaw.map(jupiterIxToTransactionIx)

  const computeBudgetIxs = instructions.computeBudgetInstructions.map(jupiterIxToTransactionIx)

  return {
    innerCpiIxs,
    peripheralIxs,
    setupIxCount,
    cleanupIxCount,
    computeBudgetIxs,
    jupiterAltAddresses: instructions.addressLookupTableAddresses,
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeHeader(overrides?: Partial<IntentHeader>): IntentHeader {
  return {
    user: PublicKey.unique(),
    srcMint: new PublicKey(USDC_MINT),
    amountIn: 1_000_000n,
    outMint: new PublicKey(USDT_MINT),
    receiver: PublicKey.unique(),
    minAmountOut: 950_000n,
    feeRecipients: [],
    deadline: BigInt(Math.floor(Date.now() / 1000) + 3600),
    salt: new Uint8Array(32).fill(0x07),
    executor: PAYER,
    ...overrides,
  }
}

function signatureCount(header: IntentHeader): number {
  return header.executor && !header.executor.equals(PAYER) ? 2 : 1
}

function makeCommonALT(): AddressLookupTableAccount {
  return new AddressLookupTableAccount({
    key: PublicKey.unique(),
    state: {
      deactivationSlot: BigInt("18446744073709551615"),
      lastExtendedSlot: 0,
      lastExtendedSlotStartIndex: 0,
      authority: PublicKey.unique(),
      addresses: [
        PROGRAM_ID,
        TOKEN_PROGRAM_ID,
        ASSOCIATED_TOKEN_PROGRAM_ID,
        SystemProgram.programId,
        SYSVAR_CLOCK_PUBKEY,
      ],
    },
  })
}

const altCache = new Map<string, AddressLookupTableAccount>()

async function loadJupiterALTs(
  connection: Connection,
  altAddresses: string[],
): Promise<AddressLookupTableAccount[]> {
  const loaded: AddressLookupTableAccount[] = []

  for (const addr of altAddresses) {
    const cached = altCache.get(addr)
    if (cached) {
      loaded.push(cached)
      continue
    }

    const result = await connection.getAddressLookupTable(new PublicKey(addr))
    if (!result.value) {
      throw new Error(`Failed to load Jupiter ALT ${addr}: account not found on-chain`)
    }

    altCache.set(addr, result.value)
    loaded.push(result.value)
  }

  return loaded
}

function classify(txBytes: number): "fits" | "phase-2 pressure" | "blocker" {
  if (txBytes <= SOLANA_TX_LIMIT * 0.90) return "fits"
  if (txBytes <= SOLANA_TX_LIMIT) return "phase-2 pressure"
  return "blocker"
}

// ---------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------

function measureJupiterExecute(
  name: string,
  header: IntentHeader,
  partitioned: PartitionedIxs,
  maxAccounts: number,
  opts: {
    includePeripherals: boolean
    lookupTables?: AddressLookupTableAccount[]
    jitoMode: boolean
  },
): JupiterScenarioResult {
  const { executeIx, headerBytes, callsBytes, tailPubkeys } = buildExecuteIntentIx({
    header,
    symbolicIxs: partitioned.innerCpiIxs,
    programId: PROGRAM_ID,
  })

  const computeBudgetIxs = opts.jitoMode
    ? stripCuPrice(partitioned.computeBudgetIxs)
    : partitioned.computeBudgetIxs

  const allIxs: TransactionInstruction[] = [...computeBudgetIxs]
  if (opts.includePeripherals) {
    allIxs.push(...partitioned.peripheralIxs)
  }
  allIxs.push(executeIx)

  const msg = new TransactionMessage({
    payerKey: PAYER,
    recentBlockhash: PublicKey.default.toBase58(),
    instructions: allIxs,
  }).compileToV0Message(opts.lookupTables)

  const tx = new VersionedTransaction(msg)

  let txBytes: number
  let msgBytes: number
  try {
    txBytes = tx.serialize().length
    msgBytes = tx.message.serialize().length
  } catch {
    const sigCount = signatureCount(header)
    const staticKeyCount = tx.message.staticAccountKeys.length
    msgBytes = 1 + 32 + 32 + staticKeyCount * 32 + 64
    txBytes = 1 + sigCount * 64 + msgBytes
  }

  const altAddressCount = opts.lookupTables
    ? opts.lookupTables.reduce((sum, t) => sum + t.state.addresses.length, 0)
    : 0

  const entry: JupiterScenarioResult = {
    name,
    kind: "execute",
    maxAccounts,
    calls: partitioned.innerCpiIxs.length,
    namedAccounts: 5,
    tailAccounts: tailPubkeys.length,
    signatures: signatureCount(header),
    headerBytes: headerBytes.length,
    callsBytes: callsBytes.length,
    messageBytes: msgBytes,
    transactionBytes: txBytes,
    usesAlt: !!opts.lookupTables && opts.lookupTables.length > 0,
    altAddressCount,
    setupIxCount: partitioned.setupIxCount,
    cleanupIxCount: partitioned.cleanupIxCount,
    computeBudgetIxCount: computeBudgetIxs.length,
    jupiterAltTableCount: partitioned.jupiterAltAddresses.length,
    outerIxCount:
      computeBudgetIxs.length +
      1 +
      (opts.includePeripherals ? partitioned.peripheralIxs.length : 0),
    jitoMode: opts.jitoMode,
    fits: txBytes <= SOLANA_TX_LIMIT,
    classification: classify(txBytes),
  }

  results.push(entry)
  return entry
}

// ---------------------------------------------------------------------------
// Test suite
// ---------------------------------------------------------------------------

const _env = loadTestEnv()
const _canRun = !LOAD_JUPITER_FROM_API || (!!_env.jupiterApiKey && !!_env.solanaRpcUrl)

describe.skipIf(!_canRun)("Jupiter Tx Size Measurements", () => {
  const env = _env
  const active = activeScenarios(env)
  const commonALT = makeCommonALT()

  const fixtureMap: JupiterFixtureMap = {}
  let rpcConnection: Connection | null = null
  let loaded = false

  beforeAll(async () => {
    if (LOAD_JUPITER_FROM_API) {
      const apiKey = requireJupiterApiKey(env)
      const rpcUrl = requireSolanaRpcUrl(env)
      rpcConnection = new Connection(rpcUrl, "confirmed")
      const placeholderTaker = PublicKey.unique().toBase58()

      for (const maxAcc of MAX_ACCOUNTS_MATRIX) {
        const key = `maxAccounts${maxAcc}`
        try {
          fixtureMap[key] = await loadJupiterRoute(
            maxAcc,
            env.jupiterApiUrl,
            apiKey,
            placeholderTaker,
          )
        } catch (err) {
          console.warn(`Jupiter route for maxAccounts=${maxAcc} failed: ${err}`)
          fixtureMap[key] = null
        }
      }
    } else {
      const fromDisk = loadFixtureMap()
      for (const [k, v] of Object.entries(fromDisk)) {
        fixtureMap[k] = v
      }
    }

    loaded = true
  }, 120_000)

  for (const maxAcc of MAX_ACCOUNTS_MATRIX) {
    const fixtureKey = `maxAccounts${maxAcc}`

    describe(`maxAccounts=${maxAcc}`, () => {
      function prepareFixture() {
        expect(loaded).toBe(true)
        const fixture = fixtureMap[fixtureKey]
        if (!fixture) {
          console.warn(`Skipping maxAccounts=${maxAcc}: no route available`)
          return null
        }

        const header = makeHeader()
        const receiverPubkey = header.receiver.toBase58()
        const sourceAta = PublicKey.unique().toBase58()
        const receiverToken = PublicKey.unique().toBase58()

        const partitioned = partitionJupiterIxs(
          fixture,
          sourceAta,
          receiverToken,
          receiverPubkey,
        )

        return { header, partitioned }
      }

      it.skipIf(!active.has("no_alt"))(`measures no-ALT`, () => {
        const ctx = prepareFixture()
        if (!ctx) return

        const r = measureJupiterExecute(
          `jupiter_execute_max${maxAcc}_no_alt`,
          ctx.header,
          ctx.partitioned,
          maxAcc,
          { includePeripherals: false, jitoMode: false },
        )

        expect(r.transactionBytes).toBeGreaterThan(0)
      })

      it.skipIf(!active.has("no_alt_peripherals"))(`measures no-ALT with peripherals`, () => {
        const ctx = prepareFixture()
        if (!ctx) return

        const r = measureJupiterExecute(
          `jupiter_execute_max${maxAcc}_no_alt_peripherals`,
          ctx.header,
          ctx.partitioned,
          maxAcc,
          { includePeripherals: true, jitoMode: false },
        )

        expect(r.transactionBytes).toBeGreaterThan(0)
      })

      it.skipIf(!active.has("with_alt"))(`measures with ALT`, async () => {
        const ctx = prepareFixture()
        if (!ctx) return

        const jupALTs = await loadJupiterALTs(rpcConnection!, ctx.partitioned.jupiterAltAddresses)

        const r = measureJupiterExecute(
          `jupiter_execute_max${maxAcc}_with_alt`,
          ctx.header,
          ctx.partitioned,
          maxAcc,
          {
            includePeripherals: false,
            lookupTables: [...jupALTs, commonALT],
            jitoMode: false,
          },
        )

        expect(r.transactionBytes).toBeGreaterThan(0)
      })

      it.skipIf(!active.has("with_alt_peripherals"))(`measures with ALT + peripherals`, async () => {
        const ctx = prepareFixture()
        if (!ctx) return

        const jupALTs = await loadJupiterALTs(rpcConnection!, ctx.partitioned.jupiterAltAddresses)

        const r = measureJupiterExecute(
          `jupiter_execute_max${maxAcc}_with_alt_peripherals`,
          ctx.header,
          ctx.partitioned,
          maxAcc,
          {
            includePeripherals: true,
            lookupTables: [...jupALTs, commonALT],
            jitoMode: false,
          },
        )

        expect(r.transactionBytes).toBeGreaterThan(0)
      })

      it.skipIf(!active.has("jito_no_alt"))(`measures no-ALT (jito)`, () => {
        const ctx = prepareFixture()
        if (!ctx) return

        const r = measureJupiterExecute(
          `jupiter_execute_max${maxAcc}_jito_no_alt`,
          ctx.header,
          ctx.partitioned,
          maxAcc,
          { includePeripherals: false, jitoMode: true },
        )

        expect(r.transactionBytes).toBeGreaterThan(0)
      })

      it.skipIf(!active.has("jito_no_alt_peripherals"))(`measures no-ALT with peripherals (jito)`, () => {
        const ctx = prepareFixture()
        if (!ctx) return

        const r = measureJupiterExecute(
          `jupiter_execute_max${maxAcc}_jito_no_alt_peripherals`,
          ctx.header,
          ctx.partitioned,
          maxAcc,
          { includePeripherals: true, jitoMode: true },
        )

        expect(r.transactionBytes).toBeGreaterThan(0)
      })

      it.skipIf(!active.has("jito_with_alt"))(`measures with ALT (jito)`, async () => {
        const ctx = prepareFixture()
        if (!ctx) return

        const jupALTs = await loadJupiterALTs(rpcConnection!, ctx.partitioned.jupiterAltAddresses)

        const r = measureJupiterExecute(
          `jupiter_execute_max${maxAcc}_jito_with_alt`,
          ctx.header,
          ctx.partitioned,
          maxAcc,
          {
            includePeripherals: false,
            lookupTables: [...jupALTs, commonALT],
            jitoMode: true,
          },
        )

        expect(r.transactionBytes).toBeGreaterThan(0)
      })

      it.skipIf(!active.has("jito_with_alt_peripherals"))(
        `measures with ALT + peripherals (jito)`,
        async () => {
          const ctx = prepareFixture()
          if (!ctx) return

          const jupALTs = await loadJupiterALTs(
            rpcConnection!,
            ctx.partitioned.jupiterAltAddresses,
          )

          const r = measureJupiterExecute(
            `jupiter_execute_max${maxAcc}_jito_with_alt_peripherals`,
            ctx.header,
            ctx.partitioned,
            maxAcc,
            {
              includePeripherals: true,
              lookupTables: [...jupALTs, commonALT],
              jitoMode: true,
            },
          )

          expect(r.transactionBytes).toBeGreaterThan(0)
        },
      )
    })
  }

  afterAll(() => {
    const validResults = results.filter((r) => r.transactionBytes > 0)

    const artifact = {
      generated: new Date().toISOString(),
      solana_tx_limit: SOLANA_TX_LIMIT,
      source: LOAD_JUPITER_FROM_API ? "api" : "fixture",
      route: { inputMint: USDC_MINT, outputMint: USDT_MINT, amount: "1000000" },
      max_accounts_matrix: MAX_ACCOUNTS_MATRIX,
      summary: {
        total: validResults.length,
        fits: validResults.filter((r) => r.classification === "fits").length,
        pressure: validResults.filter((r) => r.classification === "phase-2 pressure").length,
        blockers: validResults.filter((r) => r.classification === "blocker").length,
      },
      scenarios: validResults,
      phase2_notes: [] as string[],
    }

    for (const r of validResults) {
      if (r.classification === "blocker") {
        artifact.phase2_notes.push(
          `${r.name}: ${r.transactionBytes}B exceeds ${SOLANA_TX_LIMIT}B limit. ` +
            `Consider: reduce inner calls, use route-specific ALT, or split via Jito bundle.`,
        )
      } else if (r.classification === "phase-2 pressure") {
        artifact.phase2_notes.push(
          `${r.name}: ${r.transactionBytes}B is within limit but above 90% ` +
            `(${Math.round((r.transactionBytes / SOLANA_TX_LIMIT) * 100)}%). ` +
            `Monitor growth.`,
        )
      }
    }

    if (artifact.phase2_notes.length === 0) {
      artifact.phase2_notes.push(
        "All measured Jupiter scenarios fit within Solana tx limits.",
      )
    }

    const outPath = resolve(__dirname, "fixtures/tx_size_jupiter.json")
    writeFileSync(outPath, JSON.stringify(artifact, null, 2) + "\n")
  })
})

import { PublicKey } from "@solana/web3.js"
import {
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token"
import { borshSerialize, BorshSchema } from "borsher"

/**
 * Across Program ID (mainnet)
 */
export const ACROSS_PROGRAM_ID = new PublicKey(
  "DLv3NggMiSaef97YCkew5xKUHDh13tVGZ7tydt3ZeAru",
)

/**
 * Across state seed for mainnet (0)
 */
export const ACROSS_STATE_SEED = 0n

/**
 * Across adapter payload structure
 * Maps directly to backend data from generateAcrossInstructionsSolana
 */
export interface AcrossAdapterPayload {
  /** Recipient on destination chain (evmAddressToSolanaPublicKey(toAddress)) */
  recipient: PublicKey
  /** Output token on destination (evmAddressToSolanaPublicKey(toToken.address)) */
  outputToken: PublicKey
  /**
   * Output amount multiplier (scaled by 1e18)
   * Formula: outputAmount = (inputAmount * outputAmountMultiplier) / 1e18
   * Calculated as: (1e18 * suggestedFees.outputAmount) / action.fromAmount
   */
  outputAmountMultiplier: bigint
  /** Destination chain ID */
  destinationChainId: bigint
  /** Exclusive relayer (from relayer lookup, or PublicKey.default) */
  exclusiveRelayer: PublicKey
  /** Quote timestamp from Across API */
  quoteTimestamp: number
  /** Fill deadline timestamp */
  fillDeadline: number
  /** Exclusivity parameter (deadline offset or absolute timestamp) */
  exclusivityParameter: number
  /** Optional message for cross-chain calls (usually empty) */
  message: Uint8Array
}

/**
 * Return type containing both serialized payload and required accounts
 */
export interface SerializedAcrossPayload {
  /** Serialized adapter payload (for instruction data) */
  payload: Uint8Array
  /** Adapter-specific accounts to include in instruction */
  accounts: Array<{
    pubkey: PublicKey
    isSigner: boolean
    isWritable: boolean
  }>
}

/**
 * Borsh schema for Across adapter payload
 * Must match the Rust AcrossAdapterPayload struct exactly
 */
const AcrossAdapterPayloadSchema = BorshSchema.Struct({
  recipient: BorshSchema.Array(BorshSchema.u8, 32),
  outputToken: BorshSchema.Array(BorshSchema.u8, 32),
  outputAmountMultiplier: BorshSchema.u128,
  destinationChainId: BorshSchema.u64,
  exclusiveRelayer: BorshSchema.Array(BorshSchema.u8, 32),
  quoteTimestamp: BorshSchema.u32,
  fillDeadline: BorshSchema.u32,
  exclusivityParameter: BorshSchema.u32,
  message: BorshSchema.Vec(BorshSchema.u8),
})

/**
 * Derives the Across state PDA
 * @param seed - State seed (0 for mainnet)
 * @returns Across state PDA and bump
 */
export function deriveAcrossState(
  seed: bigint = ACROSS_STATE_SEED,
): [PublicKey, number] {
  const seedBuffer = Buffer.alloc(8)
  seedBuffer.writeBigUInt64LE(seed)

  return PublicKey.findProgramAddressSync(
    [Buffer.from("state"), seedBuffer],
    ACROSS_PROGRAM_ID,
  )
}

/**
 * Derives the Across vault ATA for a given mint
 * @param acrossState - Across state PDA
 * @param mint - Token mint address
 * @param isToken2022 - Whether the mint is Token-2022
 * @returns Across vault ATA address
 */
export function deriveAcrossVault(
  acrossState: PublicKey,
  mint: PublicKey,
  isToken2022: boolean = false,
): PublicKey {
  const tokenProgram = isToken2022 ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID

  return getAssociatedTokenAddressSync(
    mint,
    acrossState,
    true, // allowOwnerOffCurve - true for PDA-owned ATAs
    tokenProgram,
  )
}

/**
 * Serializes Across adapter payload to binary format.
 *
 * This is the Step 1 function for the two-step build process.
 * Use this to prepare the Across-specific payload, then pass it to
 * the generic `buildInstruction()` function.
 *
 * @param params - Across adapter parameters (from backend quote data)
 * @param mint - Token mint address
 * @param isToken2022 - Whether the mint is Token-2022
 * @returns Serialized payload and required accounts
 *
 * @example
 * ```typescript
 * // Step 1: Serialize Across payload
 * const { payload, accounts } = Instructions.Across.serializePayload(
 *   {
 *     recipient: evmAddressToSolanaPublicKey(toAddress),
 *     outputToken: evmAddressToSolanaPublicKey(toToken.address),
 *     // Multiplier = (1e18 * outputAmount) / inputAmount
 *     outputAmountMultiplier: BigInt('1000000000000000000') * suggestedFees.outputAmount / action.fromAmount,
 *     destinationChainId: BigInt(toChainId),
 *     exclusiveRelayer: new PublicKey(relayer.exclusiveRelayer),
 *     quoteTimestamp: Number(acrossEstimate.toolData.quoteTimestamp),
 *     fillDeadline: relayFillDeadline(),
 *     exclusivityParameter: Number(relayer.exclusivityDeadline),
 *     message: new Uint8Array([]),
 *   },
 *   mint,
 *   false
 * )
 *
 * // Step 2: Use generic buildInstruction
 * const instruction = Instructions.buildInstruction({
 *   programId: PROGRAM_ID_MAINNET,
 *   payer,
 *   mint,
 *   routeSeed,
 *   minAmount,
 *   adapterId: 1, // Across
 *   adapterPayload: payload,
 *   adapterAccounts: accounts,
 *   isToken2022: false,
 * })
 * ```
 */
export function serializePayload(
  params: AcrossAdapterPayload,
  mint: PublicKey,
  isToken2022: boolean = false,
): SerializedAcrossPayload {
  const {
    recipient,
    outputToken,
    outputAmountMultiplier,
    destinationChainId,
    exclusiveRelayer,
    quoteTimestamp,
    fillDeadline,
    exclusivityParameter,
    message,
  } = params

  // Derive Across accounts
  const [acrossState] = deriveAcrossState()
  const acrossVault = deriveAcrossVault(acrossState, mint, isToken2022)

  // Serialize payload
  const payload = borshSerialize(AcrossAdapterPayloadSchema, {
    recipient: recipient.toBytes(),
    outputToken: outputToken.toBytes(),
    outputAmountMultiplier,
    destinationChainId,
    exclusiveRelayer: exclusiveRelayer.toBytes(),
    quoteTimestamp,
    fillDeadline,
    exclusivityParameter,
    message: Array.from(message),
  })

  // Build adapter-specific accounts (positions 7-9)
  const accounts = [
    // Account 7: across_state (mut for number_of_deposits increment)
    { pubkey: acrossState, isSigner: false, isWritable: true },
    // Account 8: across_vault (mut for token transfer)
    { pubkey: acrossVault, isSigner: false, isWritable: true },
    // Account 9: across_program
    { pubkey: ACROSS_PROGRAM_ID, isSigner: false, isWritable: false },
  ]

  return {
    payload: new Uint8Array(payload),
    accounts,
  }
}

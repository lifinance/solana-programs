import { PublicKey } from "@solana/web3.js"
import {
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token"
import { borshSerialize, BorshSchema } from "borsher"

/**
 * Unit adapter payload structure
 */
export interface UnitAdapterPayload {
  /** Unit deposit wallet address (from Unit API) */
  unitDepositWallet: PublicKey
  /** Token type: 0 = SPL/wSOL, 1 = native SOL (future) */
  isNative: number
}

/**
 * Return type containing both serialized payload and required accounts
 */
export interface SerializedUnitPayload {
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
 * Borsh schema for Unit adapter payload
 */
const UnitAdapterPayloadSchema = BorshSchema.Struct({
  unitDepositWallet: BorshSchema.Array(BorshSchema.u8, 32), // PublicKey is 32 bytes
  isNative: BorshSchema.u8,
})

/**
 * Serializes Unit adapter payload to binary format.
 *
 * This is the Step 1 function for the two-step build process.
 * Use this to prepare the Unit-specific payload, then pass it to
 * the generic `buildInstruction()` function.
 *
 * @param params - Unit adapter parameters
 * @param mint - Token mint address
 * @param isToken2022 - Whether the mint is Token-2022
 * @returns Serialized payload and required accounts
 *
 * @example
 * ```typescript
 * // Step 1: Serialize Unit payload
 * const { payload, accounts } = Instructions.Unit.serializePayload(
 *   {
 *     unitDepositWallet: new PublicKey('...'),
 *     isNative: 0,
 *   },
 *   mint,
 *   false
 * )
 *
 * // Step 2: Use generic buildInstruction
 * const instruction = Instructions.buildInstruction({
 *   programId: PROGRAM_ID_DEVNET,
 *   payer,
 *   user,
 *   mint,
 *   routeSeed,
 *   minAmount,
 *   adapterId: 0,
 *   adapterPayload: payload,
 *   adapterAccounts: accounts,
 * })
 * ```
 */
export function serializePayload(
  params: UnitAdapterPayload,
  mint: PublicKey,
  isToken2022: boolean = false,
): SerializedUnitPayload {
  const { unitDepositWallet, isNative } = params

  // Determine token program
  const tokenProgram = isToken2022 ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID

  // Derive Unit deposit ATA
  const unitDepositAta = getAssociatedTokenAddressSync(
    mint,
    unitDepositWallet,
    false, // allowOwnerOffCurve - false for regular wallet
    tokenProgram,
  )

  // Serialize payload
  const payload = borshSerialize(UnitAdapterPayloadSchema, {
    unitDepositWallet: unitDepositWallet.toBytes(),
    isNative,
  })

  // Build adapter-specific accounts (positions 7-8)
  const accounts = [
    // Account 7: unit_deposit_wallet
    { pubkey: unitDepositWallet, isSigner: false, isWritable: false },
    // Account 8: unit_deposit_ata (mut)
    { pubkey: unitDepositAta, isSigner: false, isWritable: true },
  ]

  return {
    payload: new Uint8Array(payload),
    accounts,
  }
}

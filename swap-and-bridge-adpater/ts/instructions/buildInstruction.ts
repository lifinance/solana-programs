import { PublicKey, SystemProgram, TransactionInstruction } from "@solana/web3.js"
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token"
import { serializeInstructionData } from "../instructionData.js"
import { deriveVaultAuthority, deriveIntermediateVault } from "./unit/derivePda.js"

/**
 * Parameters for building a swap_and_bridge instruction
 */
export interface BuildInstructionParams {
  /** Router program ID */
  programId: PublicKey
  /** Account that pays for ATA creation (if needed) */
  payer: PublicKey
  /** Token mint address */
  mint: PublicKey
  /** Unique route seed (8 bytes) */
  routeSeed: Uint8Array
  /** Minimum amount expected (slippage protection) */
  minAmount: bigint
  /** Adapter ID (0 = Unit, 1 = Across, etc.) */
  adapterId: number
  /** Pre-serialized adapter-specific payload */
  adapterPayload: Uint8Array
  /** Adapter-specific accounts to append after the 7 shared accounts */
  adapterAccounts: Array<{
    pubkey: PublicKey
    isSigner: boolean
    isWritable: boolean
  }>
  /** Whether the mint is a Token-2022 mint (default: false) */
  isToken2022?: boolean
}

/**
 * Generic function to build a swap_and_bridge instruction for any adapter.
 *
 * This is the Step 2 function that accepts a pre-serialized adapter payload
 * and builds the complete transaction instruction.
 *
 * @param params - Instruction build parameters
 * @returns Transaction instruction for swap_and_bridge
 *
 * @example
 * ```typescript
 * // Step 1: Serialize adapter-specific payload
 * const payload = Instructions.Unit.serializePayload({
 *   unitDepositWallet: new PublicKey('...'),
 *   isNative: 0,
 * })
 *
 * // Step 2: Build instruction with serialized payload
 * const instruction = Instructions.buildInstruction({
 *   programId: PROGRAM_ID_DEVNET,
 *   payer,
 *   user,
 *   mint,
 *   routeSeed,
 *   minAmount,
 *   adapterId: 0,  // Unit
 *   adapterPayload: payload,
 *   adapterAccounts: unitAccounts,
 * })
 * ```
 */
export function buildInstruction(params: BuildInstructionParams): TransactionInstruction {
  const {
    programId,
    payer,
    mint,
    routeSeed,
    minAmount,
    adapterId,
    adapterPayload,
    adapterAccounts,
    isToken2022 = false,
  } = params

  // Validate route seed length
  if (routeSeed.length !== 8) {
    throw new Error(`routeSeed must be 8 bytes, got ${routeSeed.length}`)
  }

  // Determine which token program to use
  const tokenProgram = isToken2022 ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID

  // 1. Derive vault authority PDA
  const [vaultAuthority] = deriveVaultAuthority(programId, routeSeed, mint)

  // 2. Derive intermediate vault ATA
  const intermediateVault = deriveIntermediateVault(vaultAuthority, mint, tokenProgram)

  // 3. Build instruction data
  const instructionData = serializeInstructionData({
    SwapAndBridge: {
      routeSeed,
      minAmount,
      adapterId,
      adapterPayload,
    },
  })

  // 4. Build account list: 7 shared accounts + adapter-specific accounts
  const keys = [
    // Shared accounts (0-6)
    { pubkey: payer, isSigner: true, isWritable: true },
    { pubkey: vaultAuthority, isSigner: false, isWritable: false },
    { pubkey: intermediateVault, isSigner: false, isWritable: true },
    { pubkey: mint, isSigner: false, isWritable: false },
    { pubkey: tokenProgram, isSigner: false, isWritable: false },
    { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID, isSigner: false, isWritable: false },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },

    // Adapter-specific accounts (7+)
    ...adapterAccounts,
  ]

  // 5. Return transaction instruction
  return new TransactionInstruction({
    keys,
    programId,
    data: Buffer.from(instructionData),
  })
}

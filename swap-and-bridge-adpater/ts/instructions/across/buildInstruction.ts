import {
  PublicKey,
  SystemProgram,
  TransactionInstruction,
} from "@solana/web3.js"
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token"

import { serializeInstructionData } from "../../instructionData.js"
import {
  deriveVaultAuthority,
  deriveIntermediateVault,
} from "../unit/derivePda.js"
import {
  serializePayload,
  type AcrossAdapterPayload,
} from "./serializePayload.js"

/**
 * Builds a swap_and_bridge instruction for the Across adapter
 *
 * @param programId - The router program ID
 * @param payer - Account that pays for ATA creation (if needed)
 * @param mint - Token mint address
 * @param routeSeed - Unique route seed (8 bytes)
 * @param minAmount - Minimum amount expected (slippage protection)
 * @param acrossParams - Across-specific parameters from backend quote
 * @param isToken2022 - Whether the mint is a Token-2022 mint (default: false)
 * @returns Transaction instruction
 *
 * @example
 * ```typescript
 * const instruction = Instructions.Across.buildInstruction(
 *   PROGRAM_ID_MAINNET,
 *   payer,
 *   mint,
 *   routeSeed,
 *   minAmount,
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
 *   false
 * )
 * ```
 */
export function buildInstruction(
  programId: PublicKey,
  payer: PublicKey,
  mint: PublicKey,
  routeSeed: Uint8Array,
  minAmount: bigint,
  acrossParams: AcrossAdapterPayload,
  isToken2022: boolean = false,
): TransactionInstruction {
  // Validate route seed length
  if (routeSeed.length !== 8) {
    throw new Error(`routeSeed must be 8 bytes, got ${routeSeed.length}`)
  }

  // Determine which token program to use
  const tokenProgram = isToken2022 ? TOKEN_2022_PROGRAM_ID : TOKEN_PROGRAM_ID

  // 1. Derive vault authority PDA
  const [vaultAuthority] = deriveVaultAuthority(programId, routeSeed, mint)

  // 2. Derive intermediate vault ATA
  const intermediateVault = deriveIntermediateVault(
    vaultAuthority,
    mint,
    tokenProgram,
  )

  // 3. Serialize Across payload and get accounts
  const { payload: adapterPayload, accounts: adapterAccounts } =
    serializePayload(acrossParams, mint, isToken2022)

  // 4. Build instruction data
  const instructionData = serializeInstructionData({
    SwapAndBridge: {
      routeSeed,
      minAmount,
      adapterId: 1, // Across adapter ID
      adapterPayload,
    },
  })

  // 5. Build and return transaction instruction with 10 accounts (7 shared + 3 adapter)
  return new TransactionInstruction({
    keys: [
      // Account 0: payer (mut, signer)
      { pubkey: payer, isSigner: true, isWritable: true },
      // Account 1: vault_authority (PDA)
      { pubkey: vaultAuthority, isSigner: false, isWritable: false },
      // Account 2: intermediate_vault (mut, ATA)
      { pubkey: intermediateVault, isSigner: false, isWritable: true },
      // Account 3: mint
      { pubkey: mint, isSigner: false, isWritable: false },
      // Account 4: token_program (correct one based on isToken2022)
      { pubkey: tokenProgram, isSigner: false, isWritable: false },
      // Account 5: associated_token_program
      {
        pubkey: ASSOCIATED_TOKEN_PROGRAM_ID,
        isSigner: false,
        isWritable: false,
      },
      // Account 6: system_program
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      // Across adapter accounts (positions 7-9)
      ...adapterAccounts,
    ],
    programId,
    data: Buffer.from(instructionData),
  })
}

// Re-export for convenience
export { ACROSS_PROGRAM_ID } from "./serializePayload.js"
export type { AcrossAdapterPayload } from "./serializePayload.js"

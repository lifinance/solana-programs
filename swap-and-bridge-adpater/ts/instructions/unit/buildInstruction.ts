import {
  PublicKey,
  SystemProgram,
  TransactionInstruction,
} from "@solana/web3.js"
import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
  TOKEN_PROGRAM_ID,
  TOKEN_2022_PROGRAM_ID,
} from "@solana/spl-token"
import { serializeInstructionData } from "../../instructionData.js"
import type { UnitAdapterPayload } from "./types.js"
import { serializeUnitPayload } from "./types.js"
import { deriveVaultAuthority, deriveIntermediateVault } from "./derivePda.js"

/**
 * Builds a swap_and_bridge instruction for the Unit adapter
 *
 * @param programId - The router program ID
 * @param payer - Account that pays for ATA creation (if needed)
 * @param mint - Token mint address
 * @param routeSeed - Unique route seed (8 bytes)
 * @param minAmount - Minimum amount expected (slippage protection)
 * @param unitDepositWallet - Unit bridge destination wallet
 * @param isToken2022 - Whether the mint is a Token-2022 mint (default: false)
 * @returns Transaction instruction
 */
export function buildInstruction(
  programId: PublicKey,
  payer: PublicKey,
  mint: PublicKey,
  routeSeed: Uint8Array,
  minAmount: bigint,
  unitDepositWallet: PublicKey,
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

  // 3. Derive Unit deposit ATA
  const unitDepositAta = getAssociatedTokenAddressSync(
    mint,
    unitDepositWallet,
    false, // allowOwnerOffCurve - false for regular wallet
    tokenProgram,
  )

  // 4. Build Unit adapter payload
  const unitPayload: UnitAdapterPayload = {
    unitDepositWallet,
    isNative: 0, // Currently only SPL tokens supported
  }
  const adapterPayload = serializeUnitPayload(unitPayload)

  // 5. Build instruction data
  const instructionData = serializeInstructionData({
    SwapAndBridge: {
      routeSeed,
      minAmount,
      adapterId: 0, // Unit adapter ID
      adapterPayload,
    },
  })

  // 6. Build and return transaction instruction with 9 accounts (7 shared + 2 adapter)
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
      // Account 7: unit_deposit_wallet
      { pubkey: unitDepositWallet, isSigner: false, isWritable: false },
      // Account 8: unit_deposit_ata (mut)
      { pubkey: unitDepositAta, isSigner: false, isWritable: true },
    ],
    programId,
    data: Buffer.from(instructionData),
  })
}

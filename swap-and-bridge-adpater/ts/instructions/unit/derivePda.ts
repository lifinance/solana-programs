import { PublicKey } from "@solana/web3.js"
import { getAssociatedTokenAddressSync } from "@solana/spl-token"

/**
 * Derives the vault authority PDA
 *
 * Seeds: ["vault", route_seed, mint]
 *
 * @param programId - The router program ID
 * @param routeSeed - The route seed (8 bytes)
 * @param mint - The token mint
 * @returns [vault_authority, bump]
 */
export function deriveVaultAuthority(
  programId: PublicKey,
  routeSeed: Uint8Array,
  mint: PublicKey,
): [PublicKey, number] {
  if (routeSeed.length !== 8) {
    throw new Error(`routeSeed must be 8 bytes, got ${routeSeed.length}`)
  }

  return PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), Buffer.from(routeSeed), mint.toBuffer()],
    programId,
  )
}

/**
 * Derives the intermediate vault ATA
 *
 * This is an Associated Token Account owned by the vault_authority PDA
 *
 * @param vaultAuthority - The vault authority PDA
 * @param mint - The token mint
 * @param tokenProgram - The token program (TOKEN_PROGRAM_ID or TOKEN_2022_PROGRAM_ID)
 * @returns The intermediate vault ATA address
 */
export function deriveIntermediateVault(
  vaultAuthority: PublicKey,
  mint: PublicKey,
  tokenProgram: PublicKey,
): PublicKey {
  return getAssociatedTokenAddressSync(
    mint,
    vaultAuthority,
    true, // allowOwnerOffCurve - required for PDA ownership
    tokenProgram,
  )
}

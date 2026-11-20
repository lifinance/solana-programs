import {
  PublicKey,
  SystemProgram,
  SYSVAR_CLOCK_PUBKEY,
  AddressLookupTableProgram,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js"
import type { Connection, Signer } from "@solana/web3.js"
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token"

/**
 * Create a long-lived Address Lookup Table containing the shared addresses
 * used by most intent-factory transactions.
 *
 * This is optional — it does not affect protocol correctness, only tx size.
 * Re-use the returned table across executes for ~430B savings per tx.
 */
export async function createCommonALT(
  connection: Connection,
  authority: Signer,
  programId: PublicKey
): Promise<PublicKey> {
  const slot = await connection.getSlot()

  const [createIx, lookupTableAddress] =
    AddressLookupTableProgram.createLookupTable({
      authority: authority.publicKey,
      payer: authority.publicKey,
      recentSlot: slot,
    })

  const addresses = [
    programId,
    TOKEN_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
    SystemProgram.programId,
    SYSVAR_CLOCK_PUBKEY,
  ]

  const extendIx = AddressLookupTableProgram.extendLookupTable({
    payer: authority.publicKey,
    authority: authority.publicKey,
    lookupTable: lookupTableAddress,
    addresses,
  })

  const { blockhash } = await connection.getLatestBlockhash()

  const message = new TransactionMessage({
    payerKey: authority.publicKey,
    recentBlockhash: blockhash,
    instructions: [createIx, extendIx],
  }).compileToV0Message()

  const tx = new VersionedTransaction(message)
  tx.sign([authority])

  await connection.sendTransaction(tx)

  return lookupTableAddress
}

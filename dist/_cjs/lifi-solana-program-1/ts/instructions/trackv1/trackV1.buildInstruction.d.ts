import { PublicKey, TransactionInstruction } from "@solana/web3.js";
export declare function buildInstruction(
  programId: PublicKey,
  transactionId: Uint8Array,
  epoch: bigint,
): TransactionInstruction;

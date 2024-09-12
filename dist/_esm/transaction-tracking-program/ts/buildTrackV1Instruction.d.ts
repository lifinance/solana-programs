import { PublicKey, TransactionInstruction } from "@solana/web3.js";
export declare function buildTrackV1Instruction(
  programId: PublicKey,
  transactionId: Uint8Array,
  epoch: bigint,
): Promise<TransactionInstruction>;

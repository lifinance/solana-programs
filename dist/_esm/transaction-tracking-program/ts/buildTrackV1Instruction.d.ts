import { PublicKey, TransactionInstruction } from "@solana/web3.js";
export declare function buildTrackV1Instruction(programId: PublicKey, transaction_id: Uint8Array, epoch: bigint): Promise<TransactionInstruction>;

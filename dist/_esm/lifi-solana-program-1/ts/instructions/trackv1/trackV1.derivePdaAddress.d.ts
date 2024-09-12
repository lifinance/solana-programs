import { PublicKey } from "@solana/web3.js";
export declare function derivePdaAddress(
  programId: PublicKey,
  epoch: bigint,
): PublicKey;

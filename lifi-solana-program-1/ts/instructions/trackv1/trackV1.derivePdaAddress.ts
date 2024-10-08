import { PublicKey } from "@solana/web3.js";
import { BorshSchema, borshSerialize } from "borsher";

export function derivePdaAddress(
  programId: PublicKey,
  epoch: bigint,
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [borshSerialize(BorshSchema.u64, epoch)],
    programId,
  )[0];
}

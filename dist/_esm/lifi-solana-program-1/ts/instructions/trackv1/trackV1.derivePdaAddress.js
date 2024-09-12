import { PublicKey } from "@solana/web3.js";
import { BorshSchema, borshSerialize } from "borsher";
export function derivePdaAddress(programId, epoch) {
    return PublicKey.findProgramAddressSync([borshSerialize(BorshSchema.u64, epoch)], programId)[0];
}

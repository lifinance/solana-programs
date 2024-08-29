import {PublicKey, SYSVAR_CLOCK_PUBKEY, TransactionInstruction} from "@solana/web3.js";
import {serializeTrackingInstructionData} from "./trackingInstructionData";
import {BorshSchema, borshSerialize} from "borsher";

export function deriveTrackingV1PdaAddress(
    programId: PublicKey,
    epoch: bigint
): PublicKey {
    return PublicKey.findProgramAddressSync(
        [
            borshSerialize(BorshSchema.u64, epoch)
        ],
        programId
    )[0]
}
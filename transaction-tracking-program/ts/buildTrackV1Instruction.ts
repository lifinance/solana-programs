import {PublicKey, SYSVAR_CLOCK_PUBKEY, TransactionInstruction} from "@solana/web3.js";
import {serializeTrackingInstructionData} from "./trackingInstructionData";
import {BorshSchema, borshSerialize} from "borsher";

export async function buildTrackV1Instruction(
    programId: PublicKey,
    transaction_id: Uint8Array,
    epoch: bigint
): Promise<TransactionInstruction> {
    if (transaction_id.length !== 8)
        throw new Error('Invalid transaction_id length (' + transaction_id.length + ' bytes)');

    const epoch_track_account = PublicKey.findProgramAddressSync(
        [
            borshSerialize(BorshSchema.u64, epoch)
        ],
        programId
    )[0];

    return new TransactionInstruction({
        keys: [
            {
                pubkey: SYSVAR_CLOCK_PUBKEY,
                isSigner: false,
                isWritable: false,
            },
            {
                pubkey: epoch_track_account,
                isSigner: false,
                isWritable: false,
            }
        ],
        programId: programId,
        data:  serializeTrackingInstructionData({
            TrackV1: {
                transaction_id
            }
        })
    });
}
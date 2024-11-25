import {
  PublicKey,
  SYSVAR_CLOCK_PUBKEY,
  TransactionInstruction,
} from "@solana/web3.js";
import { serializeInstructionData } from "../../instructionData.js";
import { derivePdaAddress } from "./trackV1.derivePdaAddress.js";

export function buildInstruction(
  programId: PublicKey,
  transactionId: Uint8Array,
  epoch: bigint
): TransactionInstruction {
  if (transactionId.length !== 8)
    throw new Error(
      "Invalid transaction_id length (" + transactionId.length + " bytes)"
    );

  const epoch_track_account = derivePdaAddress(programId, epoch);
  const instruction_data = serializeInstructionData({
    TrackV1: {
      transaction_id: transactionId,
    },
  });

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
      },
    ],
    programId: programId,
    data: instruction_data,
  });
}

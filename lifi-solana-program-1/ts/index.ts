import { PublicKey } from "@solana/web3.js";

export * as TrackV1 from "./instructions/trackv1";
export * from "./instructionData";

export type InstructionData = {
  TrackV1: {
    transaction_id: Uint8Array;
  };
};

export const LIFI_PROGRAM_1_ADDRESS = new PublicKey(
  "3i5JeuZuUxeKtVysUnwQNGerJP2bSMX9fTFfS4Nxe3Br",
);

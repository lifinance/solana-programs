import { PublicKey } from "@solana/web3.js";
export * as TrackV1 from "./instructions/trackv1";
export * from "./instructionData";
export type InstructionData = {
  TrackV1: {
    transaction_id: Uint8Array;
  };
};
export declare const LIFI_PROGRAM_1_ADDRESS: PublicKey;
//# sourceMappingURL=index.d.ts.map

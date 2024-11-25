import { PublicKey } from "@solana/web3.js";

export * as Instructions from "./instructions/index.js";
export * from "./instructionData.js";

/**
 * The address of the program on both the Solana Mainnet and Devnet clusters
 */
export const PROGRAM_ADDRESS = new PublicKey(
  "3i5JeuZuUxeKtVysUnwQNGerJP2bSMX9fTFfS4Nxe3Br",
);

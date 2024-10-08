use processor::process_instruction;
use solana_program::entrypoint;

mod error;
mod instructions;
mod processor;
mod state;
mod utils;

// set `process_instruction` as the program's main entrypoint
entrypoint!(process_instruction);

// define the security.txt file for the program
security_txt! {
    // Required fields
    name: "LI.FI Solana Program 1",
    project_url: "https://github.com/lifinance/solana-programs/tree/main/lifi-solana-program-1",
    contacts: "email:security@li.finance",
    policy: "https://docs.li.fi/security-first",

    // Optional Fields
    preferred_languages: "en,de",
    source_code: "https://github.com/lifinance/solana-programs/tree/main/lifi-solana-program-1/program"
}

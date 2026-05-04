/// Generates conformance fixture JSON for cross-language parity tests.
/// Run with: cargo run --example gen_vector
use intent_factory::hash::compute_intent_hash;
use intent_factory::state::intent_header::IntentOutcome;
use intent_factory::state::IntentHeader;
use intent_factory::wire::{encode_calls, CallSpecOwned};

use anchor_lang::prelude::Pubkey;

fn main() {
    let user = Pubkey::new_from_array([1u8; 32]);
    let src_mint = Pubkey::new_from_array([2u8; 32]);
    let salt = [7u8; 32];
    let executor = Pubkey::new_from_array([8u8; 32]);

    let outcome_a = IntentOutcome {
        mint: Some(Pubkey::new_from_array([3u8; 32])),
        account: Pubkey::new_from_array([4u8; 32]),
        amount: 950_000,
    };
    let outcome_b = IntentOutcome {
        mint: Some(Pubkey::new_from_array([5u8; 32])),
        account: Pubkey::new_from_array([6u8; 32]),
        amount: 10_000,
    };

    let header = IntentHeader {
        user,
        src_mint: Some(src_mint),
        amount_in: 1_000_000,
        outcomes: vec![outcome_a.clone(), outcome_b.clone()],
        deadline: 1_700_000_000,
        salt,
        executor,
    };

    let header_bytes = header.canonical_encode();

    let tail_a = Pubkey::new_from_array([10u8; 32]);
    let tail_b = Pubkey::new_from_array([11u8; 32]);
    let tail_c = Pubkey::new_from_array([12u8; 32]);
    let tail_pubkeys = vec![tail_a, tail_b, tail_c];

    // program_ix = 2 (tail_a), accounts = [0, 1, 2, 3, 4]
    let call_0 = CallSpecOwned::new(
        2,
        vec![0, 1, 2, 3, 4],
        vec![true, true, false, true, false],
        vec![true, false, false, false, false],
        vec![0xAA, 0xBB, 0xCC, 0xDD],
    );

    // program_ix = 3 (tail_b), accounts = [0, 2, 4]
    let call_1 = CallSpecOwned::new(
        3,
        vec![0, 2, 4],
        vec![true, false, true],
        vec![true, false, false],
        vec![0x11, 0x22],
    );

    let calls_bytes = encode_calls(&[call_0, call_1]);

    let intent_hash = compute_intent_hash(&header_bytes);

    println!("{{");
    println!("  \"header_bytes\": \"{}\",", hex::encode(&header_bytes));
    println!("  \"calls_bytes\": \"{}\",", hex::encode(&calls_bytes));
    println!(
        "  \"tail_pubkeys\": [{}],",
        tail_pubkeys
            .iter()
            .map(|pk| format!("\"{}\"", hex::encode(pk.as_ref())))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("  \"intent_hash\": \"{}\",", hex::encode(intent_hash));
    println!("  \"header\": {{");
    println!("    \"user\": \"{}\",", hex::encode(user.as_ref()));
    println!("    \"src_mint\": \"{}\",", hex::encode(src_mint.as_ref()));
    println!("    \"amount_in\": {},", header.amount_in);
    println!("    \"outcomes\": [");
    for (i, o) in header.outcomes.iter().enumerate() {
        let mint_hex = match &o.mint {
            Some(pk) => hex::encode(pk.as_ref()),
            None => "null".to_string(),
        };
        let comma = if i < header.outcomes.len() - 1 { "," } else { "" };
        println!(
            "      {{ \"mint\": \"{}\", \"account\": \"{}\", \"amount\": {} }}{}",
            mint_hex,
            hex::encode(o.account.as_ref()),
            o.amount,
            comma
        );
    }
    println!("    ],");
    println!("    \"deadline\": {},", header.deadline);
    println!("    \"salt\": \"{}\",", hex::encode(&header.salt));
    println!("    \"executor\": \"{}\"", hex::encode(executor.as_ref()));
    println!("  }},");
    println!("  \"calls\": [");

    let calls_data = vec![
        ("program_ix", "2", "accounts", "[0, 1, 2, 3, 4]", "is_writable", "[true, true, false, true, false]", "is_signer", "[true, false, false, false, false]", "data", "aabbccdd"),
        ("program_ix", "3", "accounts", "[0, 2, 4]", "is_writable", "[true, false, true]", "is_signer", "[true, false, false]", "data", "1122"),
    ];
    for (i, c) in calls_data.iter().enumerate() {
        let comma = if i < calls_data.len() - 1 { "," } else { "" };
        println!(
            "    {{ \"{}\": {}, \"{}\": {}, \"{}\": {}, \"{}\": {}, \"{}\": \"{}\" }}{}",
            c.0, c.1, c.2, c.3, c.4, c.5, c.6, c.7, c.8, c.9, comma
        );
    }
    println!("  ]");
    println!("}}");
}

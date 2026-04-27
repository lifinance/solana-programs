/// Generates conformance fixture JSON for cross-language parity tests.
/// Run with: cargo run --example gen_vector
use intent_factory::hash::compute_intent_hash_from_pubkeys;
use intent_factory::state::IntentHeader;
use intent_factory::wire::{encode_calls, CallSpecOwned};

use anchor_lang::prelude::Pubkey;

fn main() {
    let user = Pubkey::new_from_array([1u8; 32]);
    let src_mint = Pubkey::new_from_array([2u8; 32]);
    let out_mint = Pubkey::new_from_array([3u8; 32]);
    let receiver = Pubkey::new_from_array([4u8; 32]);
    let fee_a = Pubkey::new_from_array([5u8; 32]);
    let fee_b = Pubkey::new_from_array([6u8; 32]);
    let salt = [7u8; 32];
    let executor = Pubkey::new_from_array([8u8; 32]);

    let header = IntentHeader {
        user,
        src_mint: Some(src_mint),
        amount_in: 1_000_000,
        out_mint: Some(out_mint),
        receiver,
        min_amount_out: 950_000,
        fee_recipients: vec![(fee_a, 10_000), (fee_b, 5_000)],
        deadline: 1_700_000_000,
        salt,
        executor: Some(executor),
    };

    let header_bytes = header.canonical_encode();

    let tail_a = Pubkey::new_from_array([10u8; 32]);
    let tail_b = Pubkey::new_from_array([11u8; 32]);
    let tail_c = Pubkey::new_from_array([12u8; 32]);
    let tail_pubkeys = vec![tail_a, tail_b, tail_c];

    // program_ix = 4 (tail_a), accounts = [0, 1, 2, 3, 5, 6]
    let call_0 = CallSpecOwned::new(
        4,
        vec![0, 1, 2, 3, 5, 6],
        vec![true, true, false, true, false, true],
        vec![true, false, false, false, false, false],
        vec![0xAA, 0xBB, 0xCC, 0xDD],
    );

    // program_ix = 5 (tail_b), accounts = [0, 2, 4]
    let call_1 = CallSpecOwned::new(
        5,
        vec![0, 2, 4],
        vec![true, false, true],
        vec![true, false, false],
        vec![0x11, 0x22],
    );

    let calls_bytes = encode_calls(&[call_0, call_1]);
    let intent_hash =
        compute_intent_hash_from_pubkeys(&header_bytes, &calls_bytes, &tail_pubkeys);

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
    println!("  \"intent_hash\": \"{}\"", hex::encode(intent_hash));
    println!("}}");
}

use solana_client_ext::*;

use solana_sdk::{
    message::Message, pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction,
    transaction::Transaction,
};

#[test]
fn cu() {
    let rpc_client = solana_client::rpc_client::RpcClient::new("https://api.devnet.solana.com");
    let new_keypair = Keypair::from_bytes(&[
        252, 148, 183, 236, 100, 64, 108, 105, 26, 181, 229, 97, 54, 43, 113, 1, 253, 4, 109, 80,
        183, 26, 222, 43, 209, 246, 12, 80, 15, 246, 53, 149, 189, 22, 176, 152, 33, 128, 187, 215,
        121, 56, 191, 187, 241, 223, 7, 109, 96, 88, 243, 76, 92, 122, 185, 245, 185, 255, 80, 125,
        80, 157, 229, 222,
    ])
    .unwrap();

    let transfer_ix =
        system_instruction::transfer(&new_keypair.pubkey(), &Pubkey::new_unique(), 10000);
    let msg = Message::new(&[transfer_ix], Some(&new_keypair.pubkey()));
    let blockhash = rpc_client.get_latest_blockhash().unwrap();
    let mut tx = Transaction::new(&[&new_keypair], msg, blockhash);

    // Test direct ReturnStruct results from process_rollup_transfers
    let accounts = tx.message.account_keys.clone();
    let rollup_c = RollUpChannel::new(accounts, &rpc_client);
    let results = rollup_c.process_rollup_transfers(&[tx.clone()]);

    println!("Direct rollup results:");
    for (i, result) in results.iter().enumerate() {
        println!(
            "Transaction {}: Success={}, CU={}, Result: {}",
            i, result.success, result.cu, result.result
        );
    }

    // Test through optimize_compute_units_unsigned_tx
    let optimized_cu = rpc_client
        .optimize_compute_units_unsigned_tx(&mut tx, &[&new_keypair])
        .unwrap();

    println!("Optimized CU: {}", optimized_cu);

    // Sign and send the transaction
    tx.sign(&[new_keypair], blockhash);

    let result = rpc_client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();
    println!(
        "Transaction signature: {} (https://explorer.solana.com/tx/{}?cluster=devnet)",
        result, result
    );

    // Get transaction details
    println!("Transaction details: {:?}", tx);
}

#[test]
fn test_failed_transaction() {
    let rpc_client = solana_client::rpc_client::RpcClient::new("https://api.devnet.solana.com");

    // Create a new keypair with no funds
    let empty_keypair = Keypair::new();

    // Try to transfer more SOL than the account would have (1 SOL)
    let transfer_ix = system_instruction::transfer(
        &empty_keypair.pubkey(),
        &Pubkey::new_unique(),
        1_000_000_000, // 1 SOL in lamports
    );

    let msg = Message::new(&[transfer_ix], Some(&empty_keypair.pubkey()));
    let blockhash = rpc_client.get_latest_blockhash().unwrap();
    let tx = Transaction::new(&[&empty_keypair], msg, blockhash);

    // Process the transaction - should fail due to insufficient funds
    let accounts = tx.message.account_keys.clone();
    let rollup_c = RollUpChannel::new(accounts, &rpc_client);
    let results = rollup_c.process_rollup_transfers(&[tx.clone()]);

    println!("Failed transaction test results:");
    for (i, result) in results.iter().enumerate() {
        println!(
            "Transaction {}: Success={}, CU={}, Result: {}",
            i, result.success, result.cu, result.result
        );

        // Verify that the transaction failed
        assert!(!result.success, "Transaction should have failed");

        // The error message should contain information about the failure
        assert!(
            result.result.contains("failed"),
            "Error message should indicate failure"
        );
    }

    // Test optimize_compute_units_unsigned_tx with a failing transaction
    let mut failing_tx = tx.clone();
    let result = rpc_client.optimize_compute_units_unsigned_tx(&mut failing_tx, &[&empty_keypair]);

    // Should return an error
    assert!(
        result.is_err(),
        "optimize_compute_units_unsigned_tx should return an error for a failing transaction"
    );

    if let Err(e) = result {
        println!(
            "Expected error from optimize_compute_units_unsigned_tx: {}",
            e
        );
        assert!(
            e.to_string().contains("failed"),
            "Error message should indicate failure"
        );
    }
}

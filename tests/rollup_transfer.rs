use solana_client_ext::*;
// use std::fs; // No longer needed for payer

use solana_sdk::{
    hash::Hash, message::Message, pubkey::Pubkey, signature::Keypair, signer::Signer,
    system_instruction, transaction::Transaction,
};

fn create_transfer_tx(
    from: &Keypair,
    to: &Pubkey,
    lamports: u64,
    recent_blockhash: Hash,
) -> Transaction {
    let ix = system_instruction::transfer(&from.pubkey(), to, lamports);
    let message = Message::new(&[ix], Some(&from.pubkey()));
    let mut tx = Transaction::new_unsigned(message);
    tx.sign(&[from], recent_blockhash);
    tx
}

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

// Test function from tagged_channel_tests.rs
#[test]
fn test_rollup_channel_tagging() {
    let rpc_client = solana_client::rpc_client::RpcClient::new("https://api.devnet.solana.com"); // Kept for blockhash

    let payer_bytes = [
        252, 148, 183, 236, 100, 64, 108, 105, 26, 181, 229, 97, 54, 43, 113, 1, 253, 4, 109, 80,
        183, 26, 222, 43, 209, 246, 12, 80, 15, 246, 53, 149, 189, 22, 176, 152, 33, 128, 187, 215,
        121, 56, 191, 187, 241, 223, 7, 109, 96, 88, 243, 76, 92, 122, 185, 245, 185, 255, 80, 125,
        80, 157, 229, 222,
    ];
    let payer = Keypair::from_bytes(&payer_bytes).unwrap();

    let recipient1 = Keypair::new().pubkey();
    let recipient2 = Keypair::new().pubkey();

    let recent_blockhash = rpc_client
        .get_latest_blockhash()
        .expect("Failed to get latest blockhash");

    let accounts_for_channel = vec![payer.pubkey(), recipient1, recipient2];
        let mut channel = RollUpChannel::new(accounts_for_channel, &rpc_client);

    let tx1 = create_transfer_tx(&payer, &recipient1, 1000, recent_blockhash);
    let tx2 = create_transfer_tx(&payer, &recipient2, 2000, recent_blockhash);
    let tx3 = create_transfer_tx(&payer, &recipient1, 500, recent_blockhash);

    // === Test Case 1: Process and tag a single transaction ===
    let tag_run1 = "run1".to_string();
    println!("Processing tx1 with tag: {}", tag_run1);
    let results_tx1 = channel.process_rollup_transfers_and_tag(&[tx1.clone()], tag_run1.clone());
    assert_eq!(results_tx1.len(), 1, "Expected 1 result for tx1");
    // Expect failure as payer/recipients likely have no funds/not rent exempt without airdrops
    assert!(
        !results_tx1[0].success,
        "TX1 simulation should have failed (no airdrop). Result: {}",
        results_tx1[0].result
    );

    let tagged_results_run1 = channel
        .get_tagged_results(&tag_run1)
        .expect("Tag run1 should exist");
    assert_eq!(
        tagged_results_run1.len(),
        1,
        "Expected 1 stored result for tag run1"
    );
    assert_eq!(
        tagged_results_run1[0].success, results_tx1[0].success,
        "Stored success status should match immediate result for tx1"
    );
    assert_eq!(
        tagged_results_run1[0].cu, results_tx1[0].cu,
        "Stored CU should match immediate result CU for tx1"
    );
    assert_eq!(
        tagged_results_run1[0].result, results_tx1[0].result,
        "Stored result string should match immediate result for tx1"
    );
    println!(
        "Found {} result(s) for tag '{}' after tx1.",
        tagged_results_run1.len(),
        tag_run1
    );

    // === Test Case 2: Add another transaction to the same tag ===
    println!("Processing tx2 with tag: {}", tag_run1);
    let results_tx2 = channel.process_rollup_transfers_and_tag(&[tx2.clone()], tag_run1.clone());
    assert_eq!(results_tx2.len(), 1, "Expected 1 result for tx2");
    assert!(
        !results_tx2[0].success,
        "TX2 simulation should have failed (no airdrop). Result: {}",
        results_tx2[0].result
    );

    let tagged_results_run1_updated = channel
        .get_tagged_results(&tag_run1)
        .expect("Tag run1 should still exist");
    assert_eq!(
        tagged_results_run1_updated.len(),
        2,
        "Expected 2 stored results for tag run1 after tx2"
    );
    assert_eq!(
        tagged_results_run1_updated[0].success, results_tx1[0].success,
        "First stored result for run1 (tx1) success status incorrect"
    );
    assert_eq!(
        tagged_results_run1_updated[0].cu, results_tx1[0].cu,
        "First stored result for run1 should be from tx1"
    );
    assert_eq!(
        tagged_results_run1_updated[0].result, results_tx1[0].result,
        "First stored result for run1 (tx1) result string incorrect"
    );
    assert_eq!(
        tagged_results_run1_updated[1].success, results_tx2[0].success,
        "Second stored result for run1 (tx2) success status incorrect"
    );
    assert_eq!(
        tagged_results_run1_updated[1].cu, results_tx2[0].cu,
        "Second stored result for run1 should be from tx2"
    );
    assert_eq!(
        tagged_results_run1_updated[1].result, results_tx2[0].result,
        "Second stored result for run1 (tx2) result string incorrect"
    );
    println!(
        "Found {} result(s) for tag '{}' after tx2.",
        tagged_results_run1_updated.len(),
        tag_run1
    );

    // === Test Case 3: Process a transaction with a new tag ===
    let tag_run2 = "run2".to_string();
    println!("Processing tx3 with tag: {}", tag_run2);
    let results_tx3 = channel.process_rollup_transfers_and_tag(&[tx3.clone()], tag_run2.clone());
    assert_eq!(results_tx3.len(), 1, "Expected 1 result for tx3");
    assert!(
        !results_tx3[0].success,
        "TX3 simulation should have failed (no airdrop). Result: {}",
        results_tx3[0].result
    );

    let tagged_results_run2 = channel
        .get_tagged_results(&tag_run2)
        .expect("Tag run2 should exist");
    assert_eq!(
        tagged_results_run2.len(),
        1,
        "Expected 1 stored result for tag run2"
    );
    assert_eq!(
        tagged_results_run2[0].success, results_tx3[0].success,
        "Stored success status should match immediate result for tx3"
    );
    assert_eq!(
        tagged_results_run2[0].cu, results_tx3[0].cu,
        "Stored CU should match immediate result CU for tx3"
    );
    assert_eq!(
        tagged_results_run2[0].result, results_tx3[0].result,
        "Stored result string should match immediate result for tx3"
    );
    println!(
        "Found {} result(s) for tag '{}' after tx3.",
        tagged_results_run2.len(),
        tag_run2
    );

    // === Test Case 4: Ensure tag_run1 is unaffected by tag_run2 ===
    let tagged_results_run1_final_check = channel
        .get_tagged_results(&tag_run1)
        .expect("Tag run1 should still be valid");
    assert_eq!(
        tagged_results_run1_final_check.len(),
        2,
        "Tag run1 should still have 2 results"
    );
    println!(
        "Tag '{}' still has {} results.",
        tag_run1,
        tagged_results_run1_final_check.len()
    );

    // === Test Case 5: Retrieve results for a non-existent tag ===
    let non_existent_tag = "non_existent_tag";
    println!(
        "Attempting to retrieve results for non-existent tag: {}",
        non_existent_tag
    );
    assert!(
        channel.get_tagged_results(non_existent_tag).is_none(),
        "Expected None for a non-existent tag"
    );
    println!("Verified retrieval for non-existent tag returns None.");

    // === Test Case 6: Process multiple transactions in one call with a tag ===
    let tag_multi = "run_multi_tx".to_string();
    let transactions_for_multi_tag = vec![tx1.clone(), tx2.clone(), tx3.clone()];
    println!(
        "Processing {} transactions with tag: {}",
        transactions_for_multi_tag.len(),
        tag_multi
    );
    let results_multi =
        channel.process_rollup_transfers_and_tag(&transactions_for_multi_tag, tag_multi.clone());
    assert_eq!(
        results_multi.len(),
        transactions_for_multi_tag.len(),
        "Immediate results count mismatch for multi-tx call"
    );
    for (i, res) in results_multi.iter().enumerate() {
        assert!(
            !res.success,
            "Transaction {} in multi-tag call should have failed (no airdrop). Result: {}",
            i, res.result
        );
    }

    let tagged_results_multi = channel
        .get_tagged_results(&tag_multi)
        .expect("Tag run_multi_tx should exist");
    assert_eq!(
        tagged_results_multi.len(),
        transactions_for_multi_tag.len(),
        "Stored results count mismatch for multi-tx call"
    );
    for i in 0..results_multi.len() {
        assert_eq!(
            tagged_results_multi[i].success, results_multi[i].success,
            "Success status mismatch for stored vs immediate result in multi-tx call, index {}",
            i
        );
        assert_eq!(
            tagged_results_multi[i].cu, results_multi[i].cu,
            "CU mismatch for stored vs immediate result in multi-tx call, index {}",
            i
        );
        assert_eq!(
            tagged_results_multi[i].result, results_multi[i].result,
            "Result string mismatch for stored vs immediate result in multi-tx call, index {}",
            i
        );
    }
    println!(
        "Found {} result(s) for tag '{}' after multi-tx call.",
        tagged_results_multi.len(),
        tag_multi
    );
}

use solana_client_ext::state::rollup_channel::RollUpChannel;
use solana_client_ext::AnalysisConfig;
use solana_client_ext::*;

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

    let accounts = tx.message.account_keys.clone();
    let rollup_c = RollUpChannel::new(accounts, &rpc_client);
    let results = rollup_c.simulate_transactions_raw(&[tx.clone()]);

    println!("Direct rollup results:");
    for (i, result) in results.iter().enumerate() {
        println!(
            "Transaction {}: Success={}, CU={}, Result: {}",
            i, result.success, result.cu, result.result
        );
    }

    let optimized_cu = rpc_client
        .optimize_compute_units_unsigned_tx(&mut tx, &[&new_keypair])
        .unwrap();

    println!("Optimized CU: {}", optimized_cu);

    tx.sign(&[new_keypair], blockhash);

    let result = rpc_client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();
    println!(
        "Transaction signature: {} (https://explorer.solana.com/tx/{}?cluster=devnet)",
        result, result
    );

    println!("Transaction details: {:?}", tx);
}

#[test]
fn test_failed_transaction() {
    let rpc_client = solana_client::rpc_client::RpcClient::new("https://api.devnet.solana.com");

    let empty_keypair = Keypair::new();

    let transfer_ix = system_instruction::transfer(
        &empty_keypair.pubkey(),
        &Pubkey::new_unique(),
        1_000_000_000,
    );

    let msg = Message::new(&[transfer_ix], Some(&empty_keypair.pubkey()));
    let blockhash = rpc_client.get_latest_blockhash().unwrap();
    let tx = Transaction::new(&[&empty_keypair], msg, blockhash);

    let accounts = tx.message.account_keys.clone();
    let rollup_c = RollUpChannel::new(accounts, &rpc_client);
    let results = rollup_c.simulate_transactions_raw(&[tx.clone()]);

    println!("Failed transaction test results:");
    for (i, result) in results.iter().enumerate() {
        println!(
            "Transaction {}: Success={}, CU={}, Result: {}",
            i, result.success, result.cu, result.result
        );

        assert!(!result.success, "Transaction should have failed");

        assert!(
            result.result.contains("failed"),
            "Error message should indicate failure"
        );
    }

    let mut failing_tx = tx.clone();
    let result = rpc_client.optimize_compute_units_unsigned_tx(&mut failing_tx, &[&empty_keypair]);

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

#[test]
fn test_rollup_channel_tagging() {
    let rpc_client = solana_client::rpc_client::RpcClient::new("https://api.devnet.solana.com");
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

    let config_cu_only_tag1 = AnalysisConfig {
        estimate_compute_units: true,
        tag: Some("run1_cu_only".to_string()),
    };
    let config_cu_only_tag2 = AnalysisConfig {
        estimate_compute_units: true,
        tag: Some("run2_cu_only".to_string()),
    };
    let config_cu_only_tag_multi = AnalysisConfig {
        estimate_compute_units: true,
        tag: Some("run_multi_cu_only".to_string()),
    };
    let config_cu_only_no_tag = AnalysisConfig {
        estimate_compute_units: true,
        tag: None,
    };

    println!("Processing tx1 with tag: {:?}", config_cu_only_tag1.tag);
    let analysis_results_tx1 =
        channel.process_transactions_with_analysis(&[tx1.clone()], &config_cu_only_tag1);
    assert_eq!(
        analysis_results_tx1.len(),
        1,
        "Expected 1 analysis result for tx1"
    );
    assert!(
        !analysis_results_tx1[0].base_simulation_success,
        "TX1 base simulation should have failed (no airdrop). Error: {:?}",
        analysis_results_tx1[0].top_level_error_message
    );
    assert_eq!(analysis_results_tx1[0].analysis_type, "compute_units");

    let tagged_results_run1 = channel
        .get_tagged_results(config_cu_only_tag1.tag.as_ref().unwrap())
        .expect("Tag run1_cu_only should exist");
    assert_eq!(
        tagged_results_run1.len(),
        1,
        "Expected 1 stored analysis result for tag run1_cu_only"
    );
    assert_eq!(
        tagged_results_run1[0].base_simulation_success,
        analysis_results_tx1[0].base_simulation_success
    );
    println!(
        "Found {} result(s) for tag '{:?}' after tx1.",
        tagged_results_run1.len(),
        config_cu_only_tag1.tag
    );

    println!("Processing tx2 with tag: {:?}", config_cu_only_tag1.tag);
    let analysis_results_tx2 =
        channel.process_transactions_with_analysis(&[tx2.clone()], &config_cu_only_tag1);
    assert_eq!(analysis_results_tx2.len(), 1);
    assert!(!analysis_results_tx2[0].base_simulation_success);

    let tagged_results_run1_updated = channel
        .get_tagged_results(config_cu_only_tag1.tag.as_ref().unwrap())
        .expect("Tag run1_cu_only should exist");
    assert_eq!(
        tagged_results_run1_updated.len(),
        2,
        "Expected 2 stored results for tag run1_cu_only after tx2"
    );
    println!(
        "Found {} result(s) for tag '{:?}' after tx2.",
        tagged_results_run1_updated.len(),
        config_cu_only_tag1.tag
    );

    println!("Processing tx3 with tag: {:?}", config_cu_only_tag2.tag);
    let analysis_results_tx3 =
        channel.process_transactions_with_analysis(&[tx3.clone()], &config_cu_only_tag2);
    assert_eq!(analysis_results_tx3.len(), 1);
    assert!(!analysis_results_tx3[0].base_simulation_success);

    let tagged_results_run2 = channel
        .get_tagged_results(config_cu_only_tag2.tag.as_ref().unwrap())
        .expect("Tag run2_cu_only should exist");
    assert_eq!(tagged_results_run2.len(), 1);
    println!(
        "Found {} result(s) for tag '{:?}' after tx3.",
        tagged_results_run2.len(),
        config_cu_only_tag2.tag
    );

    let tagged_results_run1_final_check = channel
        .get_tagged_results(config_cu_only_tag1.tag.as_ref().unwrap())
        .expect("Tag run1_cu_only should still be valid");
    assert_eq!(tagged_results_run1_final_check.len(), 2);

    assert!(channel.get_tagged_results("non_existent_tag").is_none());

    let transactions_for_multi_tag = vec![tx1.clone(), tx2.clone(), tx3.clone()];
    println!(
        "Processing {} transactions with tag: {:?}",
        transactions_for_multi_tag.len(),
        config_cu_only_tag_multi.tag
    );
    let analysis_results_multi = channel
        .process_transactions_with_analysis(&transactions_for_multi_tag, &config_cu_only_tag_multi);
    assert_eq!(
        analysis_results_multi.len(),
        transactions_for_multi_tag.len()
    );
    for res in &analysis_results_multi {
        assert!(!res.base_simulation_success);
    }

    let tagged_results_multi = channel
        .get_tagged_results(config_cu_only_tag_multi.tag.as_ref().unwrap())
        .expect("Tag for multi should exist");
    assert_eq!(tagged_results_multi.len(), transactions_for_multi_tag.len());

    println!("Processing tx1 again WITHOUT a tag");
    let results_tx1_no_tag =
        channel.process_transactions_with_analysis(&[tx1.clone()], &config_cu_only_no_tag);
    assert_eq!(results_tx1_no_tag.len(), 1);
    assert!(!results_tx1_no_tag[0].base_simulation_success);

    let tagged_results_run1_after_no_tag = channel
        .get_tagged_results(config_cu_only_tag1.tag.as_ref().unwrap())
        .expect("Tag run1_cu_only should still exist");
    assert_eq!(
        tagged_results_run1_after_no_tag.len(),
        2,
        "Tag run1_cu_only should still have 2 results after a non-tagged call"
    );
}

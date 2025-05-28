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
    let default_config = AnalysisConfig::default();
    let results = rollup_c.simulate_transactions_raw(&[tx.clone()], &default_config);

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
    let default_config_for_failure_test = AnalysisConfig::default();
    let results = rollup_c.simulate_transactions_raw(&[tx.clone()], &default_config_for_failure_test);

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
fn test_prioritization_fee_simulation() {
    let rpc_client = solana_client::rpc_client::RpcClient::new("https://api.devnet.solana.com");
    let payer_bytes = [
        177,  19, 110,  13,  66, 182, 187,  96,  61, 160,  89,  47,
        228, 176, 216, 157,   7, 230, 253,  20,  89,  42,  62,  26,
        171, 167, 112,  82,  61,  15,  28, 106,  68, 134,  51,  84,
          2,  28,   7,  33, 163,  70, 209,  54, 137,  31,   1, 190,
        138, 169,   2, 122, 137,  96,   9, 234, 165,  81, 218, 202,
         46,   5,  96, 229,
    ];
    let payer = Keypair::from_bytes(&payer_bytes).unwrap_or_else(|e| {
        panic!("Failed to create payer keypair from hardcoded bytes: {}. Ensure these bytes form a valid secret key.", e);
    });
    let recipient = Keypair::new().pubkey();

    let recent_blockhash = rpc_client
        .get_latest_blockhash()
        .expect("Failed to get latest blockhash");

    let lamports_for_rent_exemption = rpc_client
        .get_minimum_balance_for_rent_exemption(0)
        .expect("Failed to get minimum balance for rent exemption");

    println!("Attempting to send {} lamports (rent-exempt minimum) to recipient.", lamports_for_rent_exemption);

    let tx = create_transfer_tx(&payer, &recipient, lamports_for_rent_exemption, recent_blockhash);

    let accounts_for_channel = tx.message.account_keys.clone();
    let mut channel = RollUpChannel::new(accounts_for_channel, &rpc_client);

    let config_with_fee = AnalysisConfig {
        estimate_compute_units: true,
        calculate_priority_fee: true,
        tag: Some("test_fee_calc".to_string()),
    };

    println!("Processing tx with fee calculation, tag: {:?}", config_with_fee.tag);
    let analysis_results = channel.process_transactions_with_analysis(&[tx.clone()], &config_with_fee);

    assert_eq!(analysis_results.len(), 2, "Expected 2 analysis results (CU and Fee)");

    let mut cu_result_found = false;
    let mut fee_result_found = false;

    for result in &analysis_results {
        println!("Analysis Result Type: {}, Base Success: {}, Details: {:?}", result.analysis_type, result.base_simulation_success, result.details);
        
        if result.analysis_type == "compute_units" {
            cu_result_found = true;
            if let AnalysisResultDetail::ComputeUnits(cu_details) = &result.details {
                println!("  CU Consumed: {}", cu_details.cu_consumed);
            } else {
                panic!("Expected ComputeUnits details for compute_units analysis type");
            }
        }

        if result.analysis_type == "priority_fee" {
            fee_result_found = true;
            if let AnalysisResultDetail::PriorityFee(fee_details) = &result.details {
                println!(
                    "  Fee Details: Fee per CU: {}, Total Fee: {}, Error: {:?}",
                    fee_details.fee_per_cu_micro_lamports,
                    fee_details.total_fee_lamports,
                    fee_details.error_message
                );
                
                let raw_sim_cu_result = analysis_results.iter()
                    .find(|r| r.analysis_type == "compute_units");
                
                let mut raw_sim_cu = 0;
                let mut base_sim_actually_succeeded_for_cu_analysis = false;

                if let Some(r_cu) = raw_sim_cu_result {
                    base_sim_actually_succeeded_for_cu_analysis = r_cu.base_simulation_success;
                    if let AnalysisResultDetail::ComputeUnits(details) = &r_cu.details {
                        raw_sim_cu = details.cu_consumed;
                    }
                }

                if base_sim_actually_succeeded_for_cu_analysis && raw_sim_cu > 0 {
                     assert!(fee_details.error_message.is_none(), "Expected no error in fee details for successful simulation with CUs ({} CUs), but got: {:?}", raw_sim_cu, fee_details.error_message);
                     
                     // Calculate expected total fee based on details
                     let expected_total_fee = (fee_details.fee_per_cu_micro_lamports as u128 * raw_sim_cu as u128) / 1_000_000;
                     assert_eq!(fee_details.total_fee_lamports, expected_total_fee as u64, 
                                "Total fee lamports ({}) does not match expected calculated fee ({}) based on fee_per_cu_micro_lamports ({}) and raw_sim_cu ({}).", 
                                fee_details.total_fee_lamports, expected_total_fee, fee_details.fee_per_cu_micro_lamports, raw_sim_cu);

                } else if fee_details.error_message.is_none() {

                    assert_eq!(fee_details.total_fee_lamports, 0, "Expected zero total fee if base sim failed/no CUs ({}) and no specific fee error. Fee details: {:?}", raw_sim_cu, fee_details);
                    assert_eq!(fee_details.fee_per_cu_micro_lamports, 0, "Expected zero fee per CU if base sim failed/no CUs ({}) and no specific fee error. Fee details: {:?}", raw_sim_cu, fee_details);
                }

            } else {
                panic!("Expected PriorityFee details for priority_fee analysis type");
            }
        }
    }

    assert!(cu_result_found, "Compute units analysis result was not found.");
    assert!(fee_result_found, "Priority fee analysis result was not found.");

    let tagged_results = channel
        .get_tagged_results(config_with_fee.tag.as_ref().unwrap())
        .expect("Tag 'test_fee_calc' should exist");
    assert_eq!(tagged_results.len(), 2, "Expected 2 stored analysis results for the tag");

    let tagged_fee_result_exists = tagged_results.iter().any(|r| r.analysis_type == "priority_fee");
    assert!(tagged_fee_result_exists, "Tagged results should contain priority fee analysis.");

    println!("Prioritization fee simulation test completed.");
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
        calculate_priority_fee: false,
        tag: Some("run1_cu_only".to_string()),
    };
    let config_cu_only_tag2 = AnalysisConfig {
        estimate_compute_units: true,
        calculate_priority_fee: false,
        tag: Some("run2_cu_only".to_string()),
    };
    let config_cu_only_tag_multi = AnalysisConfig {
        estimate_compute_units: true,
        calculate_priority_fee: false,
        tag: Some("run_multi_cu_only".to_string()),
    };
    let config_cu_only_no_tag = AnalysisConfig {
        estimate_compute_units: true,
        calculate_priority_fee: false,
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

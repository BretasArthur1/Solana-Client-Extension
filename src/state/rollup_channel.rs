use std::sync::{Arc, RwLock};

use solana_client::rpc_client::RpcClient;
use solana_compute_budget::compute_budget::ComputeBudget;
use solana_sdk::fee::FeeStructure;
use solana_sdk::hash::Hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::rent_collector::RentCollector;
use solana_sdk::transaction::{SanitizedTransaction as SolanaSanitizedTransaction, Transaction};

use agave_feature_set::FeatureSet;
use solana_svm::transaction_processing_result::ProcessedTransaction;
use solana_svm::transaction_processor::{
    TransactionProcessingConfig, TransactionProcessingEnvironment,
};

use crate::state::rollup_account_loader::RollUpAccountLoader;
use crate::utils::helpers::{create_transaction_batch_processor, get_transaction_check_results};
use crate::{ForkRollUpGraph, ReturnStruct};

/// Handles a group of accounts and enables simulation of transactions
/// using Solana's SVM runtime with preconfigured defaults.
pub struct RollUpChannel<'a> {
    /// A list of the account keys extracted from the transaction,
    /// passed into the rollup channel for SVM simulation and processing.
    keys: Vec<Pubkey>,
    /// Reference to an RPC client used to fetch account and cluster data.
    rpc_client: &'a RpcClient,
}

impl<'a> RollUpChannel<'a> {
    /// Constructs a new `RollUpChannel` with a list of public keys and an RPC client reference.
    pub fn new(keys: Vec<Pubkey>, rpc_client: &'a RpcClient) -> Self {
        Self { keys, rpc_client }
    }

    /// Simulates a batch of Solana transactions using the SVM runtime.
    ///
    /// This method:
    /// 1. Converts `Transaction`s into `SanitizedTransaction`s
    /// 2. Creates an SVM batch processor with default settings
    /// 3. Executes the transactions using the processor
    /// 4. Returns execution results, including compute units used and logs
    pub fn process_rollup_transfers(&self, transactions: &[Transaction]) -> Vec<ReturnStruct> {
        // Step 1: Convert raw transactions into sanitized format required by the SVM processor.
        let sanitized = transactions
            .iter()
            .map(|tx| SolanaSanitizedTransaction::from_transaction_for_tests(tx.clone()))
            .collect::<Vec<SolanaSanitizedTransaction>>();

        // Default configuration values for SVM transaction simulation.
        // These can be overridden later if custom behavior is needed.
        let compute_budget = ComputeBudget::default();
        let feature_set = Arc::new(FeatureSet::all_enabled());
        let fee_structure = FeeStructure::default();
        let _rent_collector = RentCollector::default();

        // Custom account loader implementation for fetching account data via the RPC client.
        let account_loader = RollUpAccountLoader::new(&self.rpc_client);

        // Create an SVM-compatible transaction batch processor.
        // This is the entry point for executing transactions against the Solana runtime logic.
        let fork_graph = Arc::new(RwLock::new(ForkRollUpGraph {}));
        let processor = create_transaction_batch_processor(
            &account_loader,
            &feature_set,
            &compute_budget,
            Arc::clone(&fork_graph),
        );
        println!("transaction batch processor created ");

        // Create a simulation environment, similar to a Solana runtime slot.
        let processing_environment = TransactionProcessingEnvironment {
            blockhash: Hash::default(),
            blockhash_lamports_per_signature: fee_structure.lamports_per_signature,
            epoch_total_stake: 0,
            feature_set,
            fee_lamports_per_signature: 5000,
            rent_collector: None,
        };

        // Use the default transaction processing config.
        // Can be extended to support more fine-grained control.
        let processing_config = TransactionProcessingConfig::default();

        println!("transaction processing_config created ");

        // Step 2: Execute the sanitized transactions using the simulated runtime.
        let results = processor.load_and_execute_sanitized_transactions(
            &account_loader,
            &sanitized,
            get_transaction_check_results(transactions.len()),
            &processing_environment,
            &processing_config,
        );
        println!("Executed");

        // Step 3: Parse each transaction result and convert it into a ReturnStruct.
        let mut return_results = Vec::new();

        for (i, transaction_result) in results.processing_results.iter().enumerate() {
            let tx_result = match transaction_result {
                Ok(processed_tx) => {
                    match processed_tx {
                        ProcessedTransaction::Executed(executed_tx) => {
                            let cu = executed_tx.execution_details.executed_units;
                            let logs = executed_tx.execution_details.log_messages.clone();
                            let status = executed_tx.execution_details.status.clone();
                            let is_success = status.is_ok();

                            if is_success {
                                ReturnStruct::success(cu)
                            } else {
                                match status {
                                    Err(err) => {
                                        let error_msg =
                                            format!("Transaction {} failed with error: {}", i, err);
                                        let log_msg =
                                            logs.map(|logs| logs.join("\n")).unwrap_or_default();
                                        ReturnStruct {
                                            success: false,
                                            cu,
                                            result: format!("{}\nLogs:\n{}", error_msg, log_msg),
                                        }
                                    }
                                    _ => ReturnStruct::success(cu), // This shouldn't happen as we checked is_success
                                }
                            }
                        }
                        ProcessedTransaction::FeesOnly(fees_only) => {
                            ReturnStruct::failure(format!(
                                "Transaction {} failed with error: {}. Only fees were charged.",
                                i, fees_only.load_error
                            ))
                        }
                    }
                }
                Err(err) => ReturnStruct::failure(format!("Transaction {} failed: {}", i, err)),
            };
            return_results.push(tx_result);
        }

        /// If there were no results but transactions were submitted,
        // return a fallback result to avoid empty output.
        if return_results.is_empty() && !transactions.is_empty() {
            return_results.push(ReturnStruct::no_results());
        }

        return_results
    }
}

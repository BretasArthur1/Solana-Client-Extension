use std::collections::HashMap;
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

use crate::state::return_struct::{
    AnalysisResultDetail, ComputeUnitsDetails, RawSimulationResult, SimulationAnalysisResult,
};
use crate::state::rollup_account_loader::RollUpAccountLoader;
use crate::utils::helpers::{create_transaction_batch_processor, get_transaction_check_results};
use crate::AnalysisConfig;
use crate::ForkRollUpGraph;

/// Handles a group of accounts and simulates transactions using Solana's SVM.
///
/// Uses preconfigured defaults for the SVM runtime.
pub struct RollUpChannel<'a> {
    /// Account keys from the transaction, used for SVM simulation.
    #[allow(dead_code)]
    keys: Vec<Pubkey>,
    /// RPC client reference for fetching account and cluster data.
    rpc_client: &'a RpcClient,
    /// Stores `SimulationAnalysisResult` for tagged transactions.
    tagged_results: HashMap<String, Vec<SimulationAnalysisResult>>,
}

impl<'a> RollUpChannel<'a> {
    /// Constructs a `RollUpChannel`.
    ///
    /// Takes a list of public keys and an RPC client reference.
    pub fn new(keys: Vec<Pubkey>, rpc_client: &'a RpcClient) -> Self {
        Self {
            keys,
            rpc_client,
            tagged_results: HashMap::new(),
        }
    }

    /// Performs base simulation of transactions and returns raw results.
    ///
    /// This is the core simulation logic without extra analysis or tagging.
    pub fn simulate_transactions_raw(
        &self,
        transactions: &[Transaction],
    ) -> Vec<RawSimulationResult> {
        let sanitized = transactions
            .iter()
            .map(|tx| SolanaSanitizedTransaction::from_transaction_for_tests(tx.clone()))
            .collect::<Vec<SolanaSanitizedTransaction>>();

        // Default configuration for SVM transaction simulation.
        // Can be overridden if custom behavior is needed.
        let compute_budget = ComputeBudget::default();
        let feature_set = Arc::new(FeatureSet::all_enabled());
        let fee_structure = FeeStructure::default();
        let _rent_collector = RentCollector::default();

        // Custom account loader for fetching account data via RPC.
        let account_loader = RollUpAccountLoader::new(&self.rpc_client);

        // Creates an SVM-compatible transaction batch processor.
        // Entry point for executing transactions against Solana runtime logic.
        let fork_graph = Arc::new(RwLock::new(ForkRollUpGraph {}));
        let processor = create_transaction_batch_processor(
            &account_loader,
            &feature_set,
            &compute_budget,
            Arc::clone(&fork_graph),
        );
        println!("transaction batch processor created ");

        // Creates a simulation environment, similar to a Solana runtime slot.
        let processing_environment = TransactionProcessingEnvironment {
            blockhash: Hash::default(),
            blockhash_lamports_per_signature: fee_structure.lamports_per_signature,
            epoch_total_stake: 0,
            feature_set,
            fee_lamports_per_signature: 5000,
            rent_collector: None,
        };

        // Uses the default transaction processing config.
        // Can be extended for more fine-grained control.
        let processing_config = TransactionProcessingConfig::default();

        println!("transaction processing_config created ");

        // Executes sanitized transactions using the simulated runtime.
        let results = processor.load_and_execute_sanitized_transactions(
            &account_loader,
            &sanitized,
            get_transaction_check_results(transactions.len()),
            &processing_environment,
            &processing_config,
        );

        let mut return_results = Vec::new();
        for (i, transaction_result) in results.processing_results.iter().enumerate() {
            let tx_result = match transaction_result {
                Ok(processed_tx) => match processed_tx {
                    ProcessedTransaction::Executed(executed_tx) => {
                        let cu = executed_tx.execution_details.executed_units;
                        let logs = executed_tx.execution_details.log_messages.clone();
                        let status = executed_tx.execution_details.status.clone();
                        if status.is_ok() {
                            // Construct RawSimulationResult, potentially including logs if added to its fields
                            let res = RawSimulationResult::base_success(cu);
                            // If RawSimulationResult is extended to hold logs:
                            // if let Some(log_vec) = logs { res.logs = Some(log_vec); }
                            res
                        } else {
                            let error_msg = format!(
                                "Transaction {} failed with error: {}",
                                i,
                                status.unwrap_err()
                            );
                            let log_msg = logs.map(|l| l.join("\n")).unwrap_or_default();
                            RawSimulationResult::base_failure(format!(
                                "{}\nLogs:\n{}",
                                error_msg, log_msg
                            ))
                        }
                    }
                    ProcessedTransaction::FeesOnly(fees_only) => {
                        RawSimulationResult::base_failure(format!(
                            "Transaction {} failed with error: {}. Only fees were charged.",
                            i, fees_only.load_error
                        ))
                    }
                },
                Err(err) => {
                    RawSimulationResult::base_failure(format!("Transaction {} failed: {}", i, err))
                }
            };
            return_results.push(tx_result);
        }
        if return_results.is_empty() && !transactions.is_empty() {
            return_results.push(RawSimulationResult::base_no_results());
        }
        return_results
    }

    /// Processes transactions with specified analyses.
    ///
    /// Stores results if a tag is provided in the `AnalysisConfig`.
    pub fn process_transactions_with_analysis(
        &mut self,
        transactions: &[Transaction],
        config: &AnalysisConfig,
    ) -> Vec<SimulationAnalysisResult> {
        let raw_simulation_results = self.simulate_transactions_raw(transactions);

        let mut analysis_results: Vec<SimulationAnalysisResult> = Vec::new();

        for raw_res in raw_simulation_results.iter() {
            if config.estimate_compute_units {
                // Extract logs if RawSimulationResult is updated to hold them directly
                // For now, passing None for logs from raw_res.
                let logs_for_cu_details = None;
                // let logs_for_cu_details = raw_res.logs.clone(); // if RawSimulationResult had logs

                let cu_details = ComputeUnitsDetails {
                    cu_consumed: raw_res.cu,
                    logs: logs_for_cu_details,
                    error_message: if raw_res.success {
                        None
                    } else {
                        Some(raw_res.result.clone())
                    },
                };
                analysis_results.push(SimulationAnalysisResult {
                    base_simulation_success: raw_res.success,
                    analysis_type: "compute_units".to_string(),
                    details: AnalysisResultDetail::ComputeUnits(cu_details),
                    top_level_error_message: if raw_res.success {
                        None
                    } else {
                        Some(raw_res.result.clone())
                    },
                });
            }
        }

        if let Some(tag_str) = &config.tag {
            if !analysis_results.is_empty() {
                self.tagged_results
                    .entry(tag_str.clone())
                    .or_default()
                    .extend(analysis_results.clone());
            }
        }

        analysis_results
    }

    /// Retrieves stored `SimulationAnalysisResult` for a given tag.
    pub fn get_tagged_results(&self, tag: &str) -> Option<&Vec<SimulationAnalysisResult>> {
        self.tagged_results.get(tag)
    }
}

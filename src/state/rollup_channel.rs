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
    PrioritizationFeeDetails,
};
use crate::state::rollup_account_loader::RollUpAccountLoader;
use crate::utils::helpers::{create_transaction_batch_processor, get_transaction_check_results};
use crate::AnalysisConfig;
use crate::ForkRollUpGraph;
use crate::RpcClientExt;

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
        analysis_config: &AnalysisConfig,
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
            let mut fee_details: Option<PrioritizationFeeDetails> = None;
            let executed_cu = match transaction_result {
                Ok(ProcessedTransaction::Executed(executed_tx)) => executed_tx.execution_details.executed_units,
                _ => 0,
            };

            if analysis_config.calculate_priority_fee && executed_cu > 0 {
                let accounts_for_fee_estimation: Vec<Pubkey> = transactions[i].message.account_keys.iter().cloned().collect();
                match self.rpc_client.estimate_priority_fee_for_cu_sync(Some(&accounts_for_fee_estimation), executed_cu) {
                    Ok(estimated_fee) => {
                        fee_details = Some(PrioritizationFeeDetails {
                            fee_per_cu_micro_lamports: estimated_fee.fee_per_cu_micro_lamports,
                            total_fee_lamports: estimated_fee.total_fee_lamports,
                            error_message: None,
                        });
                    }
                    Err(e) => {
                        fee_details = Some(PrioritizationFeeDetails {
                            error_message: Some(format!("Failed to estimate priority fee: {}", e)),
                            ..Default::default()
                        });
                    }
                }
            }

            let tx_result: RawSimulationResult = match transaction_result {
                Ok(processed_tx) => match processed_tx {
                    ProcessedTransaction::Executed(executed_tx) => {
                        let cu = executed_tx.execution_details.executed_units;
                        let logs = executed_tx.execution_details.log_messages.clone();
                        let status = executed_tx.execution_details.status.clone();
                        if status.is_ok() {
                            let mut res = RawSimulationResult::base_success(cu);
                            res.prioritization_fee_details = fee_details;
                            res
                        } else {
                            let error_msg = format!(
                                "Transaction {} failed with error: {}",
                                i,
                                status.unwrap_err()
                            );
                            let log_msg = logs.map(|l| l.join("\n")).unwrap_or_default();
                            let mut res = RawSimulationResult::base_failure(format!(
                                "{}\nLogs:\n{}",
                                error_msg, log_msg
                            ));
                            res.prioritization_fee_details = fee_details; // Also add here for context if needed
                            res
                        }
                    }
                    ProcessedTransaction::FeesOnly(fees_only) => {
                        let mut res = RawSimulationResult::base_failure(format!(
                            "Transaction {} failed with error: {}. Only fees were charged.",
                            i, fees_only.load_error
                        ));
                        res.prioritization_fee_details = fee_details;
                        res
                    }
                },
                Err(err) => {
                    let mut res = RawSimulationResult::base_failure(format!("Transaction {} failed: {}", i, err));
                    res.prioritization_fee_details = fee_details;
                    res
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
        let raw_simulation_results = self.simulate_transactions_raw(transactions, config);

        let mut analysis_results: Vec<SimulationAnalysisResult> = Vec::new();

        for raw_res in raw_simulation_results.iter() {
            if config.estimate_compute_units {
                let logs_for_cu_details = None;

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
            // New block for priority fee analysis
            if config.calculate_priority_fee {
                if let Some(details) = &raw_res.prioritization_fee_details {
                    analysis_results.push(SimulationAnalysisResult {
                        base_simulation_success: raw_res.success, // Base success is relevant here too
                        analysis_type: "priority_fee".to_string(),
                        details: AnalysisResultDetail::PriorityFee(details.clone()),
                        top_level_error_message: details.error_message.clone().or_else(|| {
                            if !raw_res.success {
                                Some(raw_res.result.clone())
                            } else {
                                None
                            }
                        }),
                    });
                } else {
                    // This case might occur if fee calculation was skipped due to cu=0 or other reasons
                    // Or if it failed and RawSimulationResult wasn't populated (though current logic tries to populate with error)
                    analysis_results.push(SimulationAnalysisResult {
                        base_simulation_success: raw_res.success,
                        analysis_type: "priority_fee".to_string(),
                        details: AnalysisResultDetail::PriorityFee(PrioritizationFeeDetails {
                            error_message: Some("Priority fee details not available or calculation skipped.".to_string()),
                            ..Default::default()
                        }),
                        top_level_error_message: Some("Priority fee details not available or calculation skipped.".to_string()),
                    });
                }
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

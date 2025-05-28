// # RpcClientExt
//
/// `RpcClientExt` is an extension trait for the Solana Rust client (`RpcClient`).
/// It enhances transaction simulation and compute unit (CU) estimation by providing:
/// - Transaction simulation for estimating compute units used and catch errors early.
/// - Helpers to automatically insert `ComputeBudgetInstruction` in to messages or
///   transactions for optimal CU usage.
/// - Local compute estimation using Anza's SVM API
///
///
/// ## Simulation Results (`RawSimulationResult` & `SimulationAnalysisResult`)
///
/// The crate provides structs for detailed simulation outcomes:
/// - `RawSimulationResult`: For basic simulation success/failure, CUs, and messages.
/// - `SimulationAnalysisResult`: For results of specific analyses (e.g., CU estimation) performed on a simulation.
///
/// These structs help in understanding:
/// * Transaction success/failure status.
/// * Compute units consumed.
/// * Execution results or error messages.
///
///
/// # Examples
///
/// ## Example 1: Optimize Compute Units for a Message (RPC-based)
///
/// This example demonstrates using `RpcClientExt::optimize_compute_units_msg`
/// to estimate compute units via RPC simulation and automatically add a
/// `SetComputeUnitLimit` instruction to a message.
///
/// ```no_run
/// use solana_client::rpc_client::RpcClient;
/// use solana_client_ext::RpcClientExt; // Make sure this is in scope
/// use solana_sdk::{
///     message::Message, pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction,
///     transaction::Transaction,
/// };
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rpc_client = RpcClient::new("https://api.devnet.solana.com".to_string());
///     let payer = Keypair::new(); // In a real scenario, payer needs lamports
///     let recipient = Pubkey::new_unique();
///
///     // Create an instruction and a message
///     let instruction = system_instruction::transfer(&payer.pubkey(), &recipient, 10_000);
///     let mut message = Message::new(&[instruction], Some(&payer.pubkey()));
///
///     // Optimize compute units for the message (uses RPC simulation via RpcClientExt)
///     let estimated_cu = rpc_client.optimize_compute_units_msg(&mut message, &[&payer])?;
///     println!("Message optimized with estimated CUs (RPC-based): {}", estimated_cu);
///     // `message` now includes a SetComputeUnitLimit instruction.
///
///     // Create and send the transaction
///     let blockhash = rpc_client.get_latest_blockhash()?;
///     let tx = Transaction::new(&[&payer], message, blockhash);
///
///     // Note: Sending this transaction would require the payer to have SOL and the transaction to be signed.
///     // For demonstration, we show creation. To send:
///     // let signature = rpc_client.send_and_confirm_transaction_with_spinner(&tx)?;
///     // println!("Transaction signature: https://explorer.solana.com/tx/{}?cluster=devnet", signature);
///
///     Ok(())
/// }
/// ```
///
/// ## Example 2: Estimate and Optimize CUs Locally (SVM-based)
///
/// This example shows two ways to use local, SVM-based compute unit estimation:
/// 1. Directly using `RollUpChannel` to simulate transactions and get raw CU results.
/// 2. Using `RpcClientExt::optimize_compute_units_unsigned_tx` which leverages
///    local estimation to optimize an unsigned transaction.
///
/// ```no_run
/// use solana_client::rpc_client::RpcClient;
/// // Assuming RollUpChannel and RpcClientExt are correctly imported from your crate
/// use solana_client_ext::{state::rollup_channel::RollUpChannel, RpcClientExt};
/// use solana_sdk::{
///     message::Message, pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction,
///     transaction::Transaction,
/// };
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let rpc_client = RpcClient::new("https://api.devnet.solana.com".to_string());
///     let payer = Keypair::new(); // Payer needs lamports for actual execution
///     let recipient = Pubkey::new_unique();
///     
///     // Obtain a recent blockhash for creating transactions
///     // In a real application, ensure RPC calls are handled robustly.
///     let blockhash = rpc_client.get_latest_blockhash()?;
///
///     // Create a sample message
///     let instruction = system_instruction::transfer(&payer.pubkey(), &recipient, 10_000);
///     let common_message = Message::new(&[instruction], Some(&payer.pubkey()));
///
///     // Part 1: Using RollUpChannel directly for local CU estimation
///     let tx_to_simulate_locally = Transaction::new_unsigned(common_message.clone());
///     let accounts_in_tx = tx_to_simulate_locally.message.account_keys.clone();
///     let rollup_channel = RollUpChannel::new(accounts_in_tx, &rpc_client);
///     
///     // Simulate the transaction raw to get CU and other details
///     let simulation_results = rollup_channel.simulate_transactions_raw(&[tx_to_simulate_locally.clone()]);
///
///     println!("Local simulation results (RollUpChannel):");
///     for (i, result) in simulation_results.iter().enumerate() {
///         println!(
///             "  Transaction {}: Success={}, CU={}, Result: '{}'",
///             i, result.success, result.cu, result.result
///         );
///     }
///
///     // Part 2: Using RpcClientExt to optimize an unsigned transaction locally
///     // This also uses RollUpChannel (SVM-based) estimation internally.
///     let mut tx_to_optimize_locally = Transaction::new_unsigned(common_message.clone());
///     
///     // The `signers` argument is used by `estimate_compute_units_unsigned_tx` for context,
///     // though the underlying SVM simulation might not strictly perform signature verification
///     // depending on its configuration.
///     let estimated_cu_for_local_opt = rpc_client
///         .optimize_compute_units_unsigned_tx(&mut tx_to_optimize_locally, &[&payer])?;
///     println!("Unsigned transaction optimized with local CUs: {}", estimated_cu_for_local_opt);
///     // `tx_to_optimize_locally` now includes a SetComputeUnitLimit instruction based on local estimation.
///
///     // To send this optimized transaction:
///     // tx_to_optimize_locally.sign(&[&payer], blockhash);
///     // let signature = rpc_client.send_and_confirm_transaction_with_spinner(&tx_to_optimize_locally)?;
///     // println!("Locally Optimized Tx Signature: https://explorer.solana.com/tx/{}?cluster=devnet", signature);
///
///     Ok(())
/// }
/// ```
///
/// ## Example 3: Tagged Transaction Analysis with `TaggedAnalysisClient`
///
/// This example shows how to use `TaggedAnalysisClient` to perform analyses
/// (like compute unit estimation) on transactions, store results with a tag,
/// and retrieve them later.
///
/// ```no_run
/// use solana_client::rpc_client::RpcClient;
/// // Assuming TaggedAnalysisClient, AnalysisConfig, etc. are correctly imported
/// use solana_client_ext::{
///     AnalysisConfig, TaggedAnalysisClient, state::return_struct::AnalysisResultDetail
/// };
/// use solana_sdk::{
///     message::Message, pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction,
///     transaction::Transaction,
/// };
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // TaggedAnalysisClient wraps an RpcClient
///     let mut analysis_client = TaggedAnalysisClient::new("https://api.devnet.solana.com".to_string());
///
///     let payer = Keypair::new(); // Payer needs lamports for actual execution
///     let recipient1 = Pubkey::new_unique();
///     let recipient2 = Pubkey::new_unique();
///     
///     // Access the inner rpc_client for operations like getting blockhash if needed
///     let blockhash = analysis_client.rpc_client.get_latest_blockhash()?;
///
///     // Create some sample unsigned transactions for analysis
///     let ix1 = system_instruction::transfer(&payer.pubkey(), &recipient1, 1000);
///     let msg1 = Message::new(&[ix1], Some(&payer.pubkey()));
///     let tx1 = Transaction::new_unsigned(msg1);
///
///     let ix2 = system_instruction::transfer(&payer.pubkey(), &recipient2, 2000);
///     let msg2 = Message::new(&[ix2], Some(&payer.pubkey()));
///     let tx2 = Transaction::new_unsigned(msg2);
///
///     // Define an analysis configuration with a tag
///     let analysis_config = AnalysisConfig {
///         estimate_compute_units: true,
///         tag: Some("my_batch_analysis".to_string()),
///     };
///
///     // Analyze the transactions
///     // Note: For successful simulation leading to CU estimation, accounts might need to exist
///     // or the payer needs SOL, depending on the transaction type.
///     // Here, we focus on the mechanism of analysis and tagging.
///     let results = analysis_client.analyze_transactions(&[tx1.clone(), tx2.clone()], &analysis_config)?;
///
///     println!("Direct analysis results for tag '{}':", analysis_config.tag.as_ref().unwrap());
///     for (i, result) in results.iter().enumerate() {
///         println!("  Tx {}: Base Simulation Success: {}", i, result.base_simulation_success);
///         if let Some(err_msg) = &result.top_level_error_message {
///             println!("    Top-level Error: {}", err_msg);
///         }
///         if let AnalysisResultDetail::ComputeUnits(cu_details) = &result.details {
///             println!("    CU Consumed (from analysis): {}", cu_details.cu_consumed);
///             if let Some(sim_err) = &cu_details.error_message {
///                 println!("    CU Detail Error: {}", sim_err);
///             }
///         }
///     }
///
///     // Retrieve the stored tagged results later
///     if let Some(tagged_results) = analysis_client.get_tagged_analysis_results("my_batch_analysis") {
///         println!("Retrieved {} stored result(s) for tag 'my_batch_analysis'.", tagged_results.len());
///         // Process tagged_results as needed
///         assert_eq!(tagged_results.len(), results.len(), "Stored results count should match direct results count.");
///     } else {
///         println!("No results found for tag 'my_batch_analysis'");
///     }
///
///     Ok(())
/// }
/// ```
use error::SolanaClientExtError;
use solana_client::rpc_config::RpcSimulateTransactionConfig;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::{message::Message, pubkey::Pubkey, signers::Signers, transaction::Transaction};
use std::collections::HashMap;
mod error;
pub mod state;
mod utils;
use crate::state::fork_rollup_graph::ForkRollUpGraph;
use anyhow::Result;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_response::RpcPrioritizationFee;
pub use state::rollup_channel::RollUpChannel;
pub use crate::state::return_struct::{
    AnalysisResultDetail, ComputeUnitsDetails, RawSimulationResult, SimulationAnalysisResult,
    PrioritizationFeeDetails,
};

/// Configuration for transaction simulation analyses.
#[derive(Default, Debug, Clone)]
pub struct AnalysisConfig {
    /// If `true`, estimate compute units.
    pub estimate_compute_units: bool,
    /// If `true`, calculate and include prioritization fee details.
    pub calculate_priority_fee: bool,
    /// If `Some(tag_string)`, stores analysis results under this tag.
    pub tag: Option<String>,
}

/// Wraps `RpcClient` to provide stateful, tagged analysis results.
#[derive(Debug, Default)]
pub struct TaggedAnalysisClient {
    // Using a HashMap to store tagged results for quick lookups.
    // The key is the tag (String), and the value is the SimulationAnalysisResult.
    tagged_results_store: HashMap<String, SimulationAnalysisResult>,
}

impl TaggedAnalysisClient {
    pub fn new() -> Self {
        Self { tagged_results_store: HashMap::new() }
    }

    pub fn add_tagged_result(&mut self, tag: String, result: SimulationAnalysisResult) {
        self.tagged_results_store.insert(tag, result);
    }

    pub fn get_tagged_result(&self, tag: &str) -> Option<&SimulationAnalysisResult> {
        self.tagged_results_store.get(tag)
    }
}

/// Represents the details of an estimated prioritization fee.
#[derive(Debug, Clone, Default)]
pub struct EstimatedPrioritizationFee {
    /// The fee per compute unit in micro-lamports.
    pub fee_per_cu_micro_lamports: u64,
    /// The total estimated fee in lamports.
    pub total_fee_lamports: u64,
}

#[async_trait::async_trait]
pub trait RpcClientExtAsync {
    /// Estimates the total prioritization fee in lamports for the given CU.
    ///
    /// If `accounts` is `None`, fetches global average from recent slot.
    async fn estimate_priority_fee_for_cu(
        &self,
        accounts: Option<&[Pubkey]>,
        cu: u64,
    ) -> Result<EstimatedPrioritizationFee>;
}

pub trait RpcClientExt {
    /// Estimates CUs for an **unsigned transaction** using rollup-based simulation.
    ///
    /// Returns `Ok(Vec<u64>)` (CUs per transaction) or `Err` on simulation failure.
    ///
    /// ## Safety ⚠️
    /// No signature verification; on-chain results may differ.
    fn estimate_compute_units_unsigned_tx<'a, I: Signers + ?Sized>(
        &self,
        transaction: &Transaction,
        _signers: &'a I,
    ) -> Result<Vec<u64>, Box<dyn std::error::Error + 'static>>;

    /// Estimates CUs for a message via real transaction simulation.
    ///
    /// Signs and simulates the transaction.
    /// Returns `Ok(u64)` (CUs) or `Err` on failure/missing CU data.
    fn estimate_compute_units_msg<'a, I: Signers + ?Sized>(
        &self,
        msg: &Message,
        signers: &'a I,
    ) -> Result<u64, Box<dyn std::error::Error + 'static>>;

    /// Inserts a compute budget instruction into an unsigned transaction.
    ///
    /// Uses CU estimation for guidance. Modifies the transaction **in-place**.
    fn optimize_compute_units_unsigned_tx<'a, I: Signers + ?Sized>(
        &self,
        unsigned_transaction: &mut Transaction,
        signers: &'a I,
    ) -> Result<u32, Box<dyn std::error::Error + 'static>>;

    ///
    /// Optimizes CUs at the message level.
    ///
    /// Similar to `optimize_compute_units_unsigned_tx`.
    /// Useful for later transaction construction.
    fn optimize_compute_units_msg<'a, I: Signers + ?Sized>(
        &self,
        message: &mut Message,
        signers: &'a I,
    ) -> Result<u32, Box<dyn std::error::Error + 'static>>;

    /// Estimates the total prioritization fee for the given CU (synchronous).
    fn estimate_priority_fee_for_cu_sync(
        &self,
        accounts: Option<&[Pubkey]>,
        cu: u64,
    ) -> Result<EstimatedPrioritizationFee>;
}

#[async_trait::async_trait]
impl RpcClientExtAsync for RpcClient {
    /// Estimates the total priority fee (in lamports) required to execute a transaction
    /// with a given compute unit budget, based on recent prioritization fee data.
    async fn estimate_priority_fee_for_cu(
        &self,
        accounts: Option<&[Pubkey]>, // Optional list of accounts to base the fee estimation on
        cu: u64,                     // Target compute unit budget for which to estimate fees
    ) -> Result<EstimatedPrioritizationFee> {
        // Fetch recent prioritization fees using provided accounts or empty list if None
        let fees: Vec<RpcPrioritizationFee> = match accounts {
            Some(addrs) => self.get_recent_prioritization_fees(addrs).await?,
            None => self.get_recent_prioritization_fees(&[]).await?,
        };

        // Extract the highest fee per compute unit (in micro-lamports) from the results
        let best_fee_per_cu_micro = fees.iter().map(|f| f.prioritization_fee).max().unwrap_or(0);

        // Calculate total fee by multiplying best micro-lamport rate with requested CU,
        // then convert from micro-lamports to lamports (1 lamport = 1_000_000 micro-lamports)
        let total_lamports = (best_fee_per_cu_micro as u128 * cu as u128) / 1_000_000;

        // Return the total estimated fee in lamports
        Ok(EstimatedPrioritizationFee {
            fee_per_cu_micro_lamports: best_fee_per_cu_micro,
            total_fee_lamports: total_lamports as u64,
        })
    }
}

impl RpcClientExt for solana_client::rpc_client::RpcClient {
    fn estimate_compute_units_unsigned_tx<'a, I: Signers + ?Sized>(
        &self,
        transaction: &Transaction,
        _signers: &'a I,
    ) -> Result<Vec<u64>, Box<dyn std::error::Error + 'static>> {
        let accounts: Vec<Pubkey> = transaction.message.account_keys.clone();
        let channel = RollUpChannel::new(accounts, self);
        let raw_results = channel.simulate_transactions_raw(&[transaction.clone()], &AnalysisConfig {
            estimate_compute_units: true,
            calculate_priority_fee: false,
            tag: None,
        });

        let mut cus = Vec::new();
        let mut error_messages = Vec::new();

        for res in raw_results {
            if res.success {
                cus.push(res.cu);
            } else {
                error_messages.push(res.result);
            }
        }

        if !error_messages.is_empty() {
            return Err(Box::new(SolanaClientExtError::ComputeUnitsError(format!(
                "Transaction simulation failed:\n{}",
                error_messages.join("\n") // Original join character
            ))));
        }
        // If raw_results was empty (e.g. empty transactions slice), cus will be empty. This is fine.
        Ok(cus)
    }

    fn estimate_compute_units_msg<'a, I: Signers + ?Sized>(
        &self,
        message: &Message,
        signers: &'a I,
    ) -> Result<u64, Box<dyn std::error::Error + 'static>> {
        let config = RpcSimulateTransactionConfig {
            sig_verify: true,
            ..RpcSimulateTransactionConfig::default()
        };
        let mut tx = Transaction::new_unsigned(message.clone());
        tx.sign(signers, self.get_latest_blockhash()?);
        let result = self.simulate_transaction_with_config(&tx, config)?;
        let consumed_cu = result.value.units_consumed.ok_or_else(|| {
            Box::new(SolanaClientExtError::ComputeUnitsError(
                "Missing Compute Units from transaction simulation.".into(),
            ))
        })?;
        if consumed_cu == 0 && result.value.err.is_some() {
            return Err(Box::new(SolanaClientExtError::RpcError(
                format!(
                    "Transaction simulation failed: {:?}",
                    result.value.err.unwrap()
                )
                .into(),
            )));
        }
        Ok(consumed_cu)
    }

    fn optimize_compute_units_unsigned_tx<'a, I: Signers + ?Sized>(
        &self,
        transaction: &mut Transaction,
        signers: &'a I,
    ) -> Result<u32, Box<dyn std::error::Error + 'static>> {
        let optimal_cu_vec = self.estimate_compute_units_unsigned_tx(transaction, signers)?;
        let optimal_cu = *optimal_cu_vec.get(0).ok_or_else(|| {
            Box::new(SolanaClientExtError::ComputeUnitsError(
                "CU estimation returned no results.".to_string(),
            ))
        })? as u32;
        let optimize_ix =
            ComputeBudgetInstruction::set_compute_unit_limit(optimal_cu.saturating_add(optimal_cu));
        transaction
            .message
            .account_keys
            .push(solana_sdk::compute_budget::id());
        let compiled_ix = transaction.message.compile_instruction(&optimize_ix);
        transaction.message.instructions.insert(0, compiled_ix);
        Ok(optimal_cu)
    }

    fn optimize_compute_units_msg<'a, I: Signers + ?Sized>(
        &self,
        message: &mut Message,
        signers: &'a I,
    ) -> Result<u32, Box<dyn std::error::Error + 'static>> {
        let optimal_cu = u32::try_from(self.estimate_compute_units_msg(message, signers)?)?;
        let optimize_ix = ComputeBudgetInstruction::set_compute_unit_limit(
            optimal_cu.saturating_add(150 /*optimal_cu.saturating_div(100)*100*/),
        );
        message.account_keys.push(solana_sdk::compute_budget::id());
        let compiled_ix = message.compile_instruction(&optimize_ix);
        message.instructions.insert(0, compiled_ix);
        Ok(optimal_cu)
    }

    fn estimate_priority_fee_for_cu_sync(
        &self,
        accounts: Option<&[Pubkey]>,
        cu: u64,
    ) -> Result<EstimatedPrioritizationFee> {
        let fees = match accounts {
            Some(addrs) => self.get_recent_prioritization_fees(addrs)?,
            None => self.get_recent_prioritization_fees(&[])?,
        };

        let best_fee_per_cu_micro = fees.iter().map(|f| f.prioritization_fee).max().unwrap_or(0);
        let total_lamports = (best_fee_per_cu_micro as u128 * cu as u128) / 1_000_000;

        Ok(EstimatedPrioritizationFee {
            fee_per_cu_micro_lamports: best_fee_per_cu_micro,
            total_fee_lamports: total_lamports as u64,
        })
    }
}

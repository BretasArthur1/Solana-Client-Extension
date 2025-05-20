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
/// ## `ReturnStruct`
/// The crate also provides an helper struct returned by the simulations that includes:
/// * Transaction success/failure status
/// * Compute units consumed
/// * Execution result/error message
///
///
/// # Examples
///
/// ## Example 1: Simulate and Optimize Compute Usage with RPC
///
/// ```no_run
/// use solana_client::rpc_client::RpcClient;
/// use solana_client_ext::RpcClientExt;
/// use solana_sdk::{
///     message::Message, signature::Keypair, signer::Signer, system_instruction,
///     transaction::Transaction,
/// };
/// fn main() {
///     let rpc_client = RpcClient::new("https://api.devnet.solana.com");
///     let keypair = Keypair::new();
///     let keypair2 = Keypair::new();
///     let created_ix = system_instruction::transfer(&keypair.pubkey(), &keypair2.pubkey(), 10000);
///     let mut msg = Message::new(&[created_ix], Some(&keypair.pubkey()));
///
///     let optimized_cu = rpc_client
///         .optimize_compute_units_msg(&mut msg, &[&keypair])
///         .unwrap();
///     println!("Optimized compute units: {}", optimized_cu);
///
///     let tx = Transaction::new(&[&keypair], msg, rpc_client.get_latest_blockhash().unwrap());
///     let result = rpc_client
///         .send_and_confirm_transaction_with_spinner(&tx)
///         .unwrap();
///
///     println!(
///         "Transaction signature: https://explorer.solana.com/tx/{}?cluster=devnet",
///         result
///     );
/// }
/// ```
///
/// ## Example 2: Estimate CU Locally via Anza Rollup
///
/// Skip the RPC simulation step by using a local compute unit estimation engine.
///
///
/// ```no_run
/// use solana_client::rpc_client::RpcClient;
/// use solana_client_ext::RpcClientExt;
/// use solana_sdk::{
///     message::Message, signature::Keypair, signer::Signer, system_instruction,
///     transaction::Transaction,
/// };
/// fn main() {
///     let rpc_client = RpcClient::new("https://api.devnet.solana.com");
///     let keypair = Keypair::new();
///     let keypair2 = Keypair::new();
///     let created_ix = system_instruction::transfer(&keypair.pubkey(), &keypair2.pubkey(), 10000);
///     let mut msg = Message::new(&[created_ix], Some(&keypair.pubkey()));
///     let blockhash = rpc_client.get_latest_blockhash().unwrap();
///     let tx = Transaction::new(&[&keypair], msg, rpc_client.get_latest_blockhash().unwrap());
///
///    let accounts = tx.message.account_keys.clone();
///    let rollup_c = RollUpChannel::new(accounts, &rpc_client);
///    let results = rollup_c.process_rollup_transfers(&[tx.clone()]);
///
///    println!("Get simulation results from rollup:");
///    for (i, result) in results.iter().enumerate() {
///        println!(
///            "Transaction {}: Success={}, CU={}, Result: {}",
///            i, result.success, result.cu, result.result
///        );
///    }
///
///    let optimized_cu = rpc_client
///        .optimize_compute_units_unsigned_tx(&mut tx, &[&new_keypair])
///        .unwrap();
///
///    println!("Optimized CU: {}", optimized_cu);
///
///   tx.sign(&[new_keypair], blockhash);
///
///    let result = rpc_client
///        .send_and_confirm_transaction_with_spinner(&tx)
///        .unwrap();
///
///    println!(
///        "Transaction signature: {} (https://explorer.solana.com/tx/{}?cluster=devnet)",
///        result, result
///    );
///
/// }
/// ```
use error::SolanaClientExtError;
use solana_client::rpc_config::RpcSimulateTransactionConfig;
use solana_sdk::compute_budget::ComputeBudgetInstruction;
use solana_sdk::{message::Message, signers::Signers, transaction::Transaction};
mod error;
pub mod state;
mod utils;

use crate::state::fork_rollup_graph::ForkRollUpGraph;

pub use state::{return_struct::ReturnStruct, rollup_channel::RollUpChannel};

pub trait RpcClientExt {
    /// Estimates compute units for an **unsigned transaction**.
    /// This uses a rollup-based simulation (e.g., Anza SVM) to estimate CU usage.
    ///
    /// Returns:
    /// - `Ok(Vec<u64>)`: CU consumed per transaction.
    /// - `Err(...)`: If any transaction simulation fails.
    ///
    /// ## Safety ⚠️
    /// This doesn't perform signature verification. Results may differ on-chain.
    fn estimate_compute_units_unsigned_tx<'a, I: Signers + ?Sized>(
        &self,
        transaction: &Transaction,
        _signers: &'a I,
    ) -> Result<Vec<u64>, Box<dyn std::error::Error + 'static>>;

    /// Estimate compute units for a message, using real transaction simulation.
    ///
    /// Signs and simulates the transaction using the provided signers.
    ///
    /// Returns:
    /// - `Ok(u64)`: CU consumed.
    /// - `Err(...)`: If simulation fails or CU data is missing.
    fn estimate_compute_units_msg<'a, I: Signers + ?Sized>(
        &self,
        msg: &Message,
        signers: &'a I,
    ) -> Result<u64, Box<dyn std::error::Error + 'static>>;

    /// Insert a compute budget instruction into an unsigned transaction
    /// using CU estimation as guidance.
    ///
    /// This modifies the transaction **in-place**.
    fn optimize_compute_units_unsigned_tx<'a, I: Signers + ?Sized>(
        &self,
        unsigned_transaction: &mut Transaction,
        signers: &'a I,
    ) -> Result<u32, Box<dyn std::error::Error + 'static>>;

    ///
    /// Same as `optimize_compute_units_unsigned_tx`, but works at the message level.
    ///
    /// Useful when constructing a transaction later.
    fn optimize_compute_units_msg<'a, I: Signers + ?Sized>(
        &self,
        message: &mut Message,
        signers: &'a I,
    ) -> Result<u32, Box<dyn std::error::Error + 'static>>;
}

impl RpcClientExt for solana_client::rpc_client::RpcClient {
    fn estimate_compute_units_unsigned_tx<'a, I: Signers + ?Sized>(
        &self,
        transaction: &Transaction,
        _signers: &'a I,
    ) -> Result<Vec<u64>, Box<dyn std::error::Error + 'static>> {
        // GET SVM MESSAGE

        let accounts = transaction.message.account_keys.clone();
        // Build the rollup simulation context
        let rollup_c = RollUpChannel::new(accounts, self);
        // Process the transaction via rollup
        let results = rollup_c.process_rollup_transfers(&[transaction.clone()]);

        // Check if all transactions were successful
        let failures: Vec<&ReturnStruct> = results.iter().filter(|r| !r.success).collect();

        if !failures.is_empty() {
            let error_messages = failures
                .iter()
                .map(|r| r.result.clone())
                .collect::<Vec<String>>()
                .join("\n");

            return Err(Box::new(SolanaClientExtError::ComputeUnitsError(format!(
                "Transaction simulation failed:\n{}",
                error_messages
            ))));
        }

        // Return compute units for each successful transaction
        Ok(results.iter().map(|r| r.cu).collect())
    }

    fn estimate_compute_units_msg<'a, I: Signers + ?Sized>(
        &self,
        message: &Message,
        signers: &'a I,
    ) -> Result<u64, Box<dyn std::error::Error + 'static>> {
        // Enable signature verification
        let config = RpcSimulateTransactionConfig {
            sig_verify: true,
            ..RpcSimulateTransactionConfig::default()
        };

        // Sign the message and simulate
        let mut tx = Transaction::new_unsigned(message.clone());
        tx.sign(signers, self.get_latest_blockhash()?);
        let result = self.simulate_transaction_with_config(&tx, config)?;

        // Extract CU usage, fail if not reported
        let consumed_cu = result.value.units_consumed.ok_or(Box::new(
            SolanaClientExtError::ComputeUnitsError(
                "Missing Compute Units from transaction simulation.".into(),
            ),
        ))?;

        // CU may be zero if the transaction failed silently
        if consumed_cu == 0 {
            return Err(Box::new(SolanaClientExtError::RpcError(
                "Transaction simulation failed.".into(),
            )));
        }

        Ok(consumed_cu)
    }

    fn optimize_compute_units_unsigned_tx<'a, I: Signers + ?Sized>(
        &self,
        transaction: &mut Transaction,
        signers: &'a I,
    ) -> Result<u32, Box<dyn std::error::Error + 'static>> {
        // Estimate optimal CU
        let optimal_cu_vec = self.estimate_compute_units_unsigned_tx(transaction, signers)?;
        let optimal_cu = *optimal_cu_vec.get(0).unwrap() as u32;

        // Add buffer (doubling for safety)
        let optimize_ix =
            ComputeBudgetInstruction::set_compute_unit_limit(optimal_cu.saturating_add(optimal_cu));

        // Add compute budget account key
        transaction
            .message
            .account_keys
            .push(solana_sdk::compute_budget::id());

        let compiled_ix = transaction.message.compile_instruction(&optimize_ix);

        // Compile and insert the instruction
        transaction.message.instructions.insert(0, compiled_ix);

        Ok(optimal_cu)
    }

    fn optimize_compute_units_msg<'a, I: Signers + ?Sized>(
        &self,
        message: &mut Message,
        signers: &'a I,
    ) -> Result<u32, Box<dyn std::error::Error + 'static>> {
        // Estimate optimal CU from simulation
        let optimal_cu = u32::try_from(self.estimate_compute_units_msg(message, signers)?)?;

        // Add buffer
        let optimize_ix = ComputeBudgetInstruction::set_compute_unit_limit(
            optimal_cu.saturating_add(150 /*optimal_cu.saturating_div(100)*100*/),
        );
        // Include compute budget account
        message.account_keys.push(solana_sdk::compute_budget::id());

        // Compile and insert at front
        let compiled_ix = message.compile_instruction(&optimize_ix);
        message.instructions.insert(0, compiled_ix);

        Ok(optimal_cu)
    }
}

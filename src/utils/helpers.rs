use std::sync::{Arc, RwLock};

use solana_bpf_loader_program::syscalls::create_program_runtime_environment_v1;
use solana_compute_budget::{
    compute_budget::ComputeBudget, compute_budget_limits::ComputeBudgetLimits,
};
use solana_program_runtime::loaded_programs::ProgramCacheEntry;
use solana_sdk::transaction;
use solana_svm::account_loader::CheckedTransactionDetails;
use solana_svm::transaction_processing_callback::TransactionProcessingCallback;
use solana_svm::transaction_processor::TransactionBatchProcessor;
use solana_system_program::system_processor;

use crate::ForkRollUpGraph;
use agave_feature_set::FeatureSet;

/// Generates a vector of placeholder "checked" transactions to simulate what a
/// validator would normally do before execution (signature check, account ownership, etc).
///
/// In a real validator, this step ensures transactions are structurally valid
/// before passing them to the runtime. Here, we mock that behavior so that
/// we can run fully in-memory simulations without real pre-validation.
///
/// `len` defines how many mock results to return, used for simulating batches.
pub(crate) fn get_transaction_check_results(
    len: usize,
) -> Vec<transaction::Result<CheckedTransactionDetails>> {
    let _compute_budget_limit = ComputeBudgetLimits::default();
    vec![transaction::Result::Ok(CheckedTransactionDetails::new(None, 5000,)); len]
}

/// Creates a local, in-memory transaction processor capable of simulating
/// compute unit usage and program execution without submitting transactions to a real RPC node.
///
/// This processor mirrors the runtime behavior of a Solana validator.
/// It's primarily used for local CU estimation and transaction testing.
///
/// This is critical for features like `RpcClientExt::estimate_cu_local()`
/// which depend on deterministic, offline simulation of a transaction.
///
/// `fork_graph` is the mocked ledger state.
/// `feature_set` and `compute_budget` customize runtime behavior (e.g., instruction limits).
pub(crate) fn create_transaction_batch_processor<CB: TransactionProcessingCallback>(
    callbacks: &CB,
    feature_set: &FeatureSet,
    compute_budget: &ComputeBudget,
    fork_graph: Arc<RwLock<ForkRollUpGraph>>,
) -> TransactionBatchProcessor<ForkRollUpGraph> {
    // Create a new transaction batch processor for slot 1.
    //
    // We choose slot 1 deliberately: Solana treats programs deployed in slot 0
    // as not visible until slot 1. This ensures deployed programs are active during simulation.
    let processor = TransactionBatchProcessor::<ForkRollUpGraph>::new(
        /* slot */ 1,
        /* epoch */ 1,
        Arc::downgrade(&fork_graph),
        Some(Arc::new(
            create_program_runtime_environment_v1(feature_set, compute_budget, false, false)
                .unwrap(),
        )),
        None,
    );

    // Register the System Program as a built-in.
    //
    // This enables simulation of basic SOL instructions like transfers and account creation.
    processor.add_builtin(
        callbacks,
        solana_system_program::id(),
        "system_program",
        ProgramCacheEntry::new_builtin(
            0,
            b"system_program".len(),
            system_processor::Entrypoint::vm,
        ),
    );

    // Register the BPF Loader v2 as a built-in.
    //
    // This is needed to simulate execution of BPF-based programs,
    // including programs like SPL Token, associated token accounts, etc.
    processor.add_builtin(
        callbacks,
        solana_sdk::bpf_loader::id(),
        "solana_bpf_loader_program",
        ProgramCacheEntry::new_builtin(
            0,
            b"solana_bpf_loader_program".len(),
            solana_bpf_loader_program::Entrypoint::vm,
        ),
    );

    processor
}

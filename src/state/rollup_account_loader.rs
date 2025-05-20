use solana_client::rpc_client::RpcClient;
use solana_sdk::account::ReadableAccount;
use solana_sdk::{account::AccountSharedData, pubkey::Pubkey};
use solana_svm::transaction_processing_callback::TransactionProcessingCallback;
use std::collections::HashMap;
use std::sync::RwLock;

/// A lightweight account loader that retrieves account data from an RPC client,
/// with a built-in in-memory cache for fast repeated access during transaction simulation.
///
/// This struct is intended to be used with the SVM's `TransactionBatchProcessor` by
/// implementing the `TransactionProcessingCallback` trait.
///
/// It avoids redundant RPC calls by caching account data locally in a thread-safe
/// `RwLock<HashMap<...>>`.
pub struct RollUpAccountLoader<'a> {
    /// A local, thread-safe cache of account data by Pubkey.
    cache: RwLock<HashMap<Pubkey, AccountSharedData>>,
    // Reference to the RPC client used to fetch uncached accounts.
    rpc_client: &'a RpcClient,
}

impl<'a> RollUpAccountLoader<'a> {
    /// Create a new account loader using the given RPC client.
    ///
    /// This loader will attempt to cache all accounts retrieved, making it efficient
    /// for use in high-frequency local simulations.
    pub fn new(rpc_client: &'a RpcClient) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            rpc_client,
        }
    }
}

/// Implements the `TransactionProcessingCallback` trait, which allows this
/// loader to be used during SVM transaction processing.
///
/// The processor will use this callback to fetch account data as needed during execution.
impl TransactionProcessingCallback for RollUpAccountLoader<'_> {
    /// Attempts to retrieve account data for the given public key.
    ///
    /// First checks the internal cache. If the account is not cached, it fetches
    /// the data via RPC, stores it in the cache, and returns it.
    fn get_account_shared_data(&self, pubkey: &Pubkey) -> Option<AccountSharedData> {
        if let Some(account) = self.cache.read().unwrap().get(pubkey) {
            return Some(account.clone());
        }

        // If not cached, fetch from RPC
        let account: AccountSharedData = self.rpc_client.get_account(pubkey).ok()?.into();

        // Cache for future lookups
        self.cache.write().unwrap().insert(*pubkey, account.clone());

        Some(account)
    }

    /// Determines whether the specified account is owned by one of the provided owners.
    ///
    /// This is useful during transaction processing for filtering or validating accounts
    /// that must be owned by a specific program (e.g., System or Token program).
    fn account_matches_owners(&self, account: &Pubkey, owners: &[Pubkey]) -> Option<usize> {
        self.get_account_shared_data(account)
            .and_then(|account| owners.iter().position(|key| account.owner().eq(key)))
    }
}

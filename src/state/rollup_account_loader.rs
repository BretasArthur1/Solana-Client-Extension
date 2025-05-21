use solana_client::rpc_client::RpcClient;
use solana_sdk::account::ReadableAccount;
use solana_sdk::{account::AccountSharedData, pubkey::Pubkey};
use solana_svm::transaction_processing_callback::TransactionProcessingCallback;
use std::collections::HashMap;
use std::sync::RwLock;

/// Lightweight account loader with an in-memory cache.
///
/// Retrieves account data via RPC and caches it for fast repeated access.
/// Implements `TransactionProcessingCallback` for SVM integration.
pub struct RollUpAccountLoader<'a> {
    /// Local, thread-safe cache of account data (Pubkey -> AccountSharedData).
    cache: RwLock<HashMap<Pubkey, AccountSharedData>>,
    /// RPC client reference for fetching uncached accounts.
    rpc_client: &'a RpcClient,
}

impl<'a> RollUpAccountLoader<'a> {
    /// Creates a new `RollUpAccountLoader`.
    ///
    /// Uses the given RPC client and caches retrieved accounts.
    pub fn new(rpc_client: &'a RpcClient) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            rpc_client,
        }
    }
}

/// Implements `TransactionProcessingCallback` for SVM transaction processing.
///
/// The processor uses this to fetch account data during execution.
impl TransactionProcessingCallback for RollUpAccountLoader<'_> {
    /// Retrieves account data for a given public key.
    ///
    /// Checks cache first, then fetches via RPC and caches if not found.
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

    /// Checks if an account is owned by one of the provided owners.
    ///
    /// Useful for filtering or validating accounts against specific program owners.
    fn account_matches_owners(&self, account: &Pubkey, owners: &[Pubkey]) -> Option<usize> {
        self.get_account_shared_data(account)
            .and_then(|account| owners.iter().position(|key| account.owner().eq(key)))
    }
}

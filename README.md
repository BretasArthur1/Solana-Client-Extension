# Solana Rust Client Extension

This crate provides extensions for the Solana Rust client, focusing on compute unit estimation and optimization. It also provides transaction execution details via the `RawSimulationResult` and `SimulationAnalysisResult` structs for more robust transaction processing.

## Features
* Estimates compute units for Solana transactions
* Optimizes compute unit usage by adding a compute budget instruction
* Returns detailed transaction execution information (via `RawSimulationResult` and `SimulationAnalysisResult`):
  * Success/failure status
  * Compute units used
  * Detailed result message or error information

## Usage

To use this crate, add it to your `Cargo.toml` file:

```toml
[dependencies]
solana-client-ext = { git = "https://github.com/BretasArthur1/Solana-Rust-Client-Extension", version ="0.1.1"} # Replace with the right version
```

### Basic Usage Example

```rust
use solana_client::rpc_client::RpcClient;
use solana_client_ext::RpcClientExt;
use solana_sdk::{
    message::Message, signature::Keypair, signer::Signer, system_instruction,
    transaction::Transaction,
};

fn main() {
    let rpc_client = RpcClient::new("https://api.devnet.solana.com");
    let keypair = Keypair::new();
    let keypair2 = Keypair::new();
    let created_ix = system_instruction::transfer(&keypair.pubkey(), &keypair2.pubkey(), 10000);
    let mut msg = Message::new(&[created_ix], Some(&keypair.pubkey()));

    let optimized_cu = rpc_client
        .optimize_compute_units_msg(&mut msg, &[&keypair])
        .unwrap();
    println!("Optimized compute units: {}", optimized_cu);

    let tx = Transaction::new(&[&keypair], msg, rpc_client.get_latest_blockhash().unwrap());
    let result = rpc_client
        .send_and_confirm_transaction_with_spinner(&tx)
        .unwrap();

    println!(
        "Transaction signature: https://explorer.solana.com/tx/{}?cluster=devnet",
        result
    );
}
```

### Using Simulation Results for Detailed Transaction Information

You can get detailed transaction simulation results using `RawSimulationResult` (often via `RollUpChannel::simulate_transactions_raw`):

```rust
use solana_client::rpc_client::RpcClient;
use solana_client_ext::state::rollup_channel::RollUpChannel; 
use solana_sdk::{
    message::Message, // Added Message import
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction,
    transaction::Transaction,
};

fn main() {
    let rpc_client = RpcClient::new("https://api.devnet.solana.com".to_string());
    let payer = Keypair::new(); // Ensure payer has SOL for actual transactions
    
    // Create a simple transfer transaction
    let transfer_ix = system_instruction::transfer(
        &payer.pubkey(), 
        &Pubkey::new_unique(), 
        10000
    );
    let msg = Message::new(&[transfer_ix], Some(&payer.pubkey()));
    // let blockhash = rpc_client.get_latest_blockhash().unwrap(); // Needed if you were to sign and send
    let tx = Transaction::new_unsigned(msg); // For local simulation, blockhash isn't strictly part of RawSimulationResult focus
    
    // Process the transaction locally using RollUpChannel to get raw simulation results
    let accounts_for_simulation = tx.message.account_keys.clone(); // Collect all accounts involved
    let rollup_channel = RollUpChannel::new(accounts_for_simulation, &rpc_client);
    
    // simulate_transactions_raw returns Vec<RawSimulationResult>
    let raw_simulation_results = rollup_channel.simulate_transactions_raw(&[tx.clone()]);
    
    // Display transaction results from RawSimulationResult
    println!("Local Raw Simulation Results:");
    for (i, result) in raw_simulation_results.iter().enumerate() {
        println!("  Transaction {}: Success={}, CU={}, Result: '{}'", 
            i, result.success, result.cu, result.result);
    }
    
    // For more advanced, tagged analyses, refer to TaggedAnalysisClient and SimulationAnalysisResult in lib.rs examples.
}
```

[tx](img/opt.png)

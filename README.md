# Solana Rust Client Extension

This crate provides extensions for the Solana Rust client, focusing on compute unit estimation and optimization. It also provides transaction execution details via the `ReturnStruct` for more robust transaction processing.

## Features
* Estimates compute units for Solana transactions
* Optimizes compute unit usage by adding a compute budget instruction
* Returns detailed transaction execution information:
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

### Using ReturnStruct for Detailed Transaction Information

You can also get detailed transaction results using the `ReturnStruct`:

```rust
use solana_client::rpc_client::RpcClient;
use solana_client_ext::{RpcClientExt, RollUpChannel};
use solana_sdk::{
    pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction,
    transaction::Transaction,
};

fn main() {
    let rpc_client = RpcClient::new("https://api.devnet.solana.com");
    let keypair = Keypair::new();
    
    // Create a simple transfer transaction
    let transfer_ix = system_instruction::transfer(
        &keypair.pubkey(), 
        &Pubkey::new_unique(), 
        10000
    );
    let msg = Message::new(&[transfer_ix], Some(&keypair.pubkey()));
    let blockhash = rpc_client.get_latest_blockhash().unwrap();
    let tx = Transaction::new(&[&keypair], msg, blockhash);
    
    // Process the transaction and get detailed results
    let accounts = tx.message.account_keys.clone();
    let rollup_c = RollUpChannel::new(accounts, &rpc_client);
    let results = rollup_c.process_rollup_transfers(&[tx.clone()]);
    
    // Display transaction results
    for (i, result) in results.iter().enumerate() {
        println!("Transaction {}: Success={}, CU={}, Result: {}", 
            i, result.success, result.cu, result.result);
    }
}
```

[tx](img/opt.png)
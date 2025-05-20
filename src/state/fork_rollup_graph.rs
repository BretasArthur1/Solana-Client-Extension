use solana_program_runtime::loaded_programs::{BlockRelation, ForkGraph};
use solana_sdk::clock::Slot;

/// A minimal placeholder implementation of the `ForkGraph` trait.
///
/// In a real validator, `ForkGraph` models the ledger’s fork structure, which helps
/// determine relationships between blocks (e.g., which slots are ancestors of others).
///
/// This is required by the `TransactionBatchProcessor` so it can reason about
/// program visibility and slot relationships during transaction simulation.
///
/// In our case, we don’t need full fork tracking for local CU estimation or isolated
/// transaction simulation, so we stub it with an empty struct.
pub(crate) struct ForkRollUpGraph {}
/// Implements the `ForkGraph` trait for our mocked graph.
///
/// The `relationship()` method defines how two slots relate to each other.
/// For simulation purposes, we simply return `BlockRelation::Unknown` to indicate
/// that we make no assumptions about slot ancestry.
///
/// This is sufficient because:
/// - We simulate in a single slot (usually slot 1)
/// - We're not validating or resolving forks
/// - We're not simulating program activation/deactivation across slots
impl ForkGraph for ForkRollUpGraph {
    fn relationship(&self, _a: Slot, _b: Slot) -> BlockRelation {
        BlockRelation::Unknown
    }
}

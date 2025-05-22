/// Encapsulates the outcome of a simulated or real transaction execution.
///
/// Useful for tracking:
/// - Transaction success status
/// - Compute units consumed
/// - Result or error messages
#[derive(Debug, Clone)]
pub struct RawSimulationResult {
    /// `true` if the base transaction simulation succeeded without runtime errors.
    pub success: bool,
    /// Compute units consumed during execution.
    pub cu: u64, // May become Option<u64> or part of an enum variant later
    /// Human-readable result message for debugging/logs.
    /// Contains success details or base simulation error description.
    pub result: String,
}

impl RawSimulationResult {
    /// Constructs a successful base simulation result.
    pub fn base_success(cu: u64) -> Self {
        Self {
            success: true,
            cu,
            result: format!(
                "Base simulation executed successfully with {} compute units",
                cu
            ),
        }
    }

    /// Constructs a failed base simulation result.
    pub fn base_failure(error: impl ToString) -> Self {
        Self {
            success: false,
            cu: 0, // Or from simulation if available even on failure
            result: error.to_string(),
        }
    }

    /// Constructs a result for missing/empty base simulation responses.
    pub fn base_no_results() -> Self {
        Self {
            success: false,
            cu: 0,
            result: "No base simulation results returned".to_string(),
        }
    }
}

// New Type Definitions for Analysis Results

/// Details related to compute unit estimation.
#[derive(Debug, Clone)]
pub struct ComputeUnitsDetails {
    /// Compute units consumed.
    pub cu_consumed: u64,
    /// Optional execution logs.
    pub logs: Option<Vec<String>>,
    /// Optional error message specific to CU estimation.
    pub error_message: Option<String>, // Error specific to CU estimation, if any
}

/// Enum for different types of analysis result details.
#[derive(Debug, Clone)]
pub enum AnalysisResultDetail {
    /// Detailed results of compute unit analysis.
    ComputeUnits(ComputeUnitsDetails),
    // PriorityFee(PriorityFeeDetails),
    // Future analysis types can be added here
}

/// Represents the outcome of one or more analyses on a transaction simulation.
#[derive(Debug, Clone)]
pub struct SimulationAnalysisResult {
    /// `true` if the underlying base transaction simulation was successful.
    /// If `false`, specific analysis details might be missing or indicate failure.
    pub base_simulation_success: bool,
    /// Type of analysis (e.g., "compute_units", "priority_fee").
    pub analysis_type: String,
    /// Detailed result of the specific analysis.
    pub details: AnalysisResultDetail,
    /// Optional top-level error message.
    /// For issues with the analysis itself or to reiterate base simulation errors.
    pub top_level_error_message: Option<String>,
}

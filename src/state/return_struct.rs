/// A simple struct that encapsulates the outcome of a simulated or real transaction execution.
///
/// This is especially useful when working with local transaction simulation tools
/// (like `ClientExt`) where you want to track:
/// - Whether the transaction was successful
/// - How many compute units were consumed
/// - What the result or error message was
#[derive(Debug, Clone)]
pub struct ReturnStruct {
    /// `true` if the transaction executed successfully without runtime errors.
    pub success: bool,
    /// The number of compute units consumed during execution.
    ///
    /// This is only meaningful when `success == true`. On failure, this will be 0.
    pub cu: u64,
    /// A human-readable result message, used for debugging and logs.
    /// Can contain either success details or an error description.
    pub result: String,
}

impl ReturnStruct {
    /// Construct a successful result with the given compute unit usage.
    ///
    /// The compute unit count helps benchmark cost and complexity.
    pub fn success(cu: u64) -> Self {
        Self {
            success: true,
            cu,
            result: format!(
                "Transaction executed successfully with {} compute units",
                cu
            ),
        }
    }

    /// Construct a failed result with a specific error message.
    pub fn failure(error: impl ToString) -> Self {
        Self {
            success: false,
            cu: 0,
            result: error.to_string(),
        }
    }

    /// Construct a result representing a missing or empty response.
    ///
    /// It can occur when SVM engine doesn't return results—e.g.,
    /// due to a misconfigured processor, lack of transaction output, or internal error.
    pub fn no_results() -> Self {
        Self {
            success: false,
            cu: 0,
            result: "No transaction results returned".to_string(),
        }
    }
}

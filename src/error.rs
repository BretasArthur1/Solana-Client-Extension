use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum SolanaClientExtError {
    RpcError(String),
    ComputeUnitsError(String),
}

impl Display for SolanaClientExtError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SolanaClientExtError::RpcError(ref err) => write!(f, "RPC error: {}", err),
            SolanaClientExtError::ComputeUnitsError(ref err) => {
                write!(f, "Compute Units error: {}", err)
            }
        }
    }
}

impl Error for SolanaClientExtError {}

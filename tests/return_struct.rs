use solana_client_ext::*;

use crate::ReturnStruct;

#[test]
fn test_return_struct() {
    // Test ReturnStruct helper methods
    let success_result = ReturnStruct::success(5000);
    assert_eq!(success_result.success, true);
    assert_eq!(success_result.cu, 5000);

    let failure_result = ReturnStruct::failure("Test error message");
    assert_eq!(failure_result.success, false);
    assert_eq!(failure_result.cu, 0);
    assert_eq!(failure_result.result, "Test error message");

    let no_results = ReturnStruct::no_results();
    assert_eq!(no_results.success, false);
    assert_eq!(no_results.result, "No transaction results returned");
}

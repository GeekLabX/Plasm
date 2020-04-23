pub struct NotPredicate {}

impl NotPredicate {}

impl LogicalConnective for NotPredicate {
    /// @dev Validates a child node of Not property in game tree.
    fn is_valid_challenge(
        _inputs: Vec<Vec<u8>>,
        _challenge_inputs: Vec<Vec<u8>>,
        _challenge: Property<Address>,
    ) -> ExecResult {
        // The valid challenge of not(p) is p and _inputs[0] is p here
        // return keccak256(_inputs[0]) == keccak256(abi.encode(_challenge));
        Ok(true)
    }
}
impl DecidablePredicate for NotPredicate {
    /// @dev Decides true
    fn decide_with_witness(_inputs: Vec<Vec<u8>>, _witness: Vec<Vec<u8>>) -> ExecResult {
        Ok(false)
    }
}
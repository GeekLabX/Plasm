use crate::executor::*;
use crate::predicates::*;

pub struct EqualPredicate<'a, Ext: ExternalCall> {
    pub ext: &'a mut Ext,
}

impl<'a, Ext: ExternalCall> AtomicPredicateInterface<AddressOf<Ext>> for EqualPredicate<'a, Ext> {
    type Hash = HashOf<Ext>;
    fn decide(&self, inputs: Vec<Vec<u8>>) -> ExecResult<AddressOf<Ext>> {
        require!(inputs.len() > 1);
        require_with_message!(inputs[0] == inputs[1], "2 inputs must be equal");
        Ok(true)
    }

    fn ext_address(&self) -> AddressOf<Ext> {
        self.ext.ext_address()
    }
    fn ext_set_predicate_decision(
        &self,
        game_id: Self::Hash,
        decision: bool,
    ) -> ExecResult<AddressOf<Ext>> {
        self.ext.ext_set_predicate_decision(game_id, decision)
    }
    fn ext_get_property_id(&self, property: &Property<AddressOf<Ext>>) -> Self::Hash {
        self.ext_get_property_id(property)
    }
}

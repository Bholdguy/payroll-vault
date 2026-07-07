//! PayrollVault v2 - records agent payments with owner + operator access control.
use odra::casper_types::U256;
use odra::prelude::*;

/// Errors that can occur in the `PayrollVault` module.
#[odra::odra_error]
pub enum Error {
    /// The owner is not set (contract not initialized).
    OwnerNotSet = 1,
    /// The caller is not authorized to record payments.
    NotAuthorized = 2,
    /// Adding the amount would overflow the earnings counter.
    ArithmeticOverflow = 3,
    /// The caller is not the owner.
    NotOwner = 4,
    /// The caller is not the pending owner.
    NotPendingOwner = 5
}

/// Event emitted every time a payment is recorded.
#[odra::event]
pub struct PaymentRecorded {
    /// The agent the payment was recorded for.
    pub agent_id: String,
    /// The amount added to the agent's earnings.
    pub amount: U256,
    /// Block timestamp at the time of recording.
    pub block_time: u64,
    /// The account that recorded the payment.
    pub recorded_by: Address
}

/// Event emitted when an operator is added.
#[odra::event]
pub struct OperatorAdded {
    /// The new operator.
    pub operator: Address
}

/// Event emitted when an operator is removed.
#[odra::event]
pub struct OperatorRemoved {
    /// The removed operator.
    pub operator: Address
}

/// Event emitted when an ownership transfer is proposed.
#[odra::event]
pub struct OwnershipTransferStarted {
    /// Current owner.
    pub owner: Address,
    /// Proposed new owner.
    pub pending_owner: Address
}

/// Event emitted when ownership is transferred.
#[odra::event]
pub struct OwnershipTransferred {
    /// Previous owner.
    pub previous_owner: Address,
    /// New owner.
    pub new_owner: Address
}

/// A vault that tracks total earnings per agent.
/// The owner and any approved operator may record payments.
#[odra::module(
    events = [PaymentRecorded, OperatorAdded, OperatorRemoved, OwnershipTransferStarted, OwnershipTransferred],
    errors = Error
)]
pub struct PayrollVault {
    /// The contract owner (set to the deployer at init).
    owner: Var<Address>,
    /// Proposed new owner awaiting acceptance (two-step transfer).
    pending_owner: Var<Address>,
    /// Accounts allowed to record payments in addition to the owner.
    operators: Mapping<Address, bool>,
    /// agent_id -> total earnings.
    earnings: Mapping<String, U256>
}

#[odra::module]
impl PayrollVault {
    /// Initializes the contract, recording the deployer as the owner.
    pub fn init(&mut self) {
        self.owner.set(self.env().caller());
    }

    /// Adds `amount` to `agent_id`'s total earnings and emits
    /// a `PaymentRecorded` event. Callable by the owner or an operator.
    pub fn record_payment(&mut self, agent_id: String, amount: U256) {
        let caller = self.env().caller();
        self.assert_can_record(&caller);

        let current = self.earnings.get_or_default(&agent_id);
        let new_total = match current.checked_add(amount) {
            Some(total) => total,
            None => self.env().revert(Error::ArithmeticOverflow)
        };
        self.earnings.set(&agent_id, new_total);

        self.env().emit_event(PaymentRecorded {
            agent_id,
            amount,
            block_time: self.env().get_block_time(),
            recorded_by: caller
        });
    }

    /// Returns the total recorded earnings for `agent_id`.
    /// Returns zero for unknown agents.
    pub fn get_earnings(&self, agent_id: String) -> U256 {
        self.earnings.get_or_default(&agent_id)
    }

    /// Grants `operator` the right to record payments. Owner only.
    pub fn add_operator(&mut self, operator: Address) {
        self.assert_owner();
        self.operators.set(&operator, true);
        self.env().emit_event(OperatorAdded { operator });
    }

    /// Revokes `operator`'s right to record payments. Owner only.
    pub fn remove_operator(&mut self, operator: Address) {
        self.assert_owner();
        self.operators.set(&operator, false);
        self.env().emit_event(OperatorRemoved { operator });
    }

    /// Returns true if `account` is an approved operator.
    pub fn is_operator(&self, account: Address) -> bool {
        self.operators.get_or_default(&account)
    }

    /// Proposes a new owner. The new owner must call `accept_ownership`
    /// to complete the transfer. Owner only.
    pub fn transfer_ownership(&mut self, new_owner: Address) {
        self.assert_owner();
        self.pending_owner.set(new_owner);
        self.env().emit_event(OwnershipTransferStarted {
            owner: self.owner(),
            pending_owner: new_owner
        });
    }

    /// Completes an ownership transfer. Callable only by the pending owner.
    pub fn accept_ownership(&mut self) {
        let caller = self.env().caller();
        match self.pending_owner.get() {
            Some(pending) if pending == caller => {
                let previous_owner = self.owner();
                self.owner.set(caller);
                self.pending_owner.set(caller); // clear-by-overwrite; no longer pending
                self.env().emit_event(OwnershipTransferred {
                    previous_owner,
                    new_owner: caller
                });
            }
            _ => self.env().revert(Error::NotPendingOwner)
        }
    }

    /// Returns the current owner.
    pub fn owner(&self) -> Address {
        self.owner.get_or_revert_with(Error::OwnerNotSet)
    }

    fn assert_owner(&self) {
        if self.env().caller() != self.owner() {
            self.env().revert(Error::NotOwner);
        }
    }

    fn assert_can_record(&self, caller: &Address) {
        let is_owner = *caller == self.owner();
        let is_operator = self.operators.get_or_default(caller);
        if !is_owner && !is_operator {
            self.env().revert(Error::NotAuthorized);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, PayrollVault, PaymentRecorded};
    use odra::casper_types::U256;
    use odra::host::{Deployer, NoArgs};

    #[test]
    fn deployer_is_owner_and_can_record() {
        let env = odra_test::env();
        let deployer = env.get_account(0);
        env.set_caller(deployer);
        let mut vault = PayrollVault::deploy(&env, NoArgs);

        assert_eq!(vault.owner(), deployer);
        vault.record_payment("agent-007".to_string(), U256::from(100));
        vault.record_payment("agent-007".to_string(), U256::from(50));
        assert_eq!(vault.get_earnings("agent-007".to_string()), U256::from(150));
    }

    #[test]
    fn emits_payment_recorded_event_with_recorder() {
        let env = odra_test::env();
        let deployer = env.get_account(0);
        env.set_caller(deployer);
        let mut vault = PayrollVault::deploy(&env, NoArgs);

        vault.record_payment("agent-007".to_string(), U256::from(42));
        assert!(env.emitted_event(
            &vault,
            PaymentRecorded {
                agent_id: "agent-007".to_string(),
                amount: U256::from(42),
                block_time: 0,
                recorded_by: deployer
            }
        ));
    }

    #[test]
    fn operator_can_record_until_removed() {
        let env = odra_test::env();
        let owner = env.get_account(0);
        let operator = env.get_account(1);

        env.set_caller(owner);
        let mut vault = PayrollVault::deploy(&env, NoArgs);
        vault.add_operator(operator);
        assert!(vault.is_operator(operator));

        env.set_caller(operator);
        vault.record_payment("agent-1".to_string(), U256::from(10));
        assert_eq!(vault.get_earnings("agent-1".to_string()), U256::from(10));

        env.set_caller(owner);
        vault.remove_operator(operator);
        assert!(!vault.is_operator(operator));

        env.set_caller(operator);
        assert_eq!(
            vault
                .try_record_payment("agent-1".to_string(), U256::from(10))
                .unwrap_err(),
            Error::NotAuthorized.into()
        );
        assert_eq!(vault.get_earnings("agent-1".to_string()), U256::from(10));
    }

    #[test]
    fn stranger_cannot_record_or_manage_operators() {
        let env = odra_test::env();
        let owner = env.get_account(0);
        let stranger = env.get_account(2);

        env.set_caller(owner);
        let mut vault = PayrollVault::deploy(&env, NoArgs);

        env.set_caller(stranger);
        assert_eq!(
            vault
                .try_record_payment("agent-1".to_string(), U256::from(1))
                .unwrap_err(),
            Error::NotAuthorized.into()
        );
        assert_eq!(
            vault.try_add_operator(stranger).unwrap_err(),
            Error::NotOwner.into()
        );
    }

    #[test]
    fn two_step_ownership_transfer() {
        let env = odra_test::env();
        let owner = env.get_account(0);
        let new_owner = env.get_account(1);
        let stranger = env.get_account(2);

        env.set_caller(owner);
        let mut vault = PayrollVault::deploy(&env, NoArgs);
        vault.transfer_ownership(new_owner);

        // Still the old owner until accepted.
        assert_eq!(vault.owner(), owner);

        // A stranger cannot accept.
        env.set_caller(stranger);
        assert_eq!(
            vault.try_accept_ownership().unwrap_err(),
            Error::NotPendingOwner.into()
        );

        // The pending owner accepts.
        env.set_caller(new_owner);
        vault.accept_ownership();
        assert_eq!(vault.owner(), new_owner);

        // New owner has full rights; old owner has none.
        vault.add_operator(stranger);
        env.set_caller(owner);
        assert_eq!(
            vault.try_add_operator(owner).unwrap_err(),
            Error::NotOwner.into()
        );
    }
}

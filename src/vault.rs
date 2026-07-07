//! PayrollVault - records agent payments with strict access control.
use odra::casper_types::U256;
use odra::prelude::*;

/// Errors that can occur in the `PayrollVault` module.
#[odra::odra_error]
pub enum Error {
    /// The authorized caller is not set (contract not initialized).
    AuthorizedCallerNotSet = 1,
    /// The caller is not the authorized caller.
    NotAuthorized = 2,
    /// Adding the amount would overflow the earnings counter.
    ArithmeticOverflow = 3
}

/// Event emitted every time a payment is recorded.
#[odra::event]
pub struct PaymentRecorded {
    /// The agent the payment was recorded for.
    pub agent_id: String,
    /// The amount added to the agent's earnings.
    pub amount: U256,
    /// Block timestamp at the time of recording.
    pub block_time: u64
}

/// A vault that tracks total earnings per agent.
/// Only the account that deployed the contract may record payments.
#[odra::module(events = [PaymentRecorded], errors = Error)]
pub struct PayrollVault {
    /// The only account allowed to record payments (set at deployment).
    authorized_caller: Var<Address>,
    /// agent_id -> total earnings.
    earnings: Mapping<String, U256>
}

#[odra::module]
impl PayrollVault {
    /// Initializes the contract, recording the deployer as the sole
    /// authorized caller.
    pub fn init(&mut self) {
        self.authorized_caller.set(self.env().caller());
    }

    /// Adds `amount` to `agent_id`'s total earnings and emits
    /// a `PaymentRecorded` event.
    ///
    /// Reverts with `Error::NotAuthorized` if the caller is not the
    /// authorized caller.
    pub fn record_payment(&mut self, agent_id: String, amount: U256) {
        // Access control: this check runs before any state change.
        let caller = self.env().caller();
        let authorized = self
            .authorized_caller
            .get_or_revert_with(Error::AuthorizedCallerNotSet);
        if caller != authorized {
            self.env().revert(Error::NotAuthorized);
        }

        let current = self.earnings.get_or_default(&agent_id);
        let new_total = match current.checked_add(amount) {
            Some(total) => total,
            None => self.env().revert(Error::ArithmeticOverflow)
        };
        self.earnings.set(&agent_id, new_total);

        self.env().emit_event(PaymentRecorded {
            agent_id,
            amount,
            block_time: self.env().get_block_time()
        });
    }

    /// Returns the total recorded earnings for `agent_id`.
    /// Returns zero for unknown agents.
    pub fn get_earnings(&self, agent_id: String) -> U256 {
        self.earnings.get_or_default(&agent_id)
    }

    /// Returns the address allowed to record payments.
    pub fn authorized_caller(&self) -> Address {
        self.authorized_caller
            .get_or_revert_with(Error::AuthorizedCallerNotSet)
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, PayrollVault, PaymentRecorded};
    use odra::casper_types::U256;
    use odra::host::{Deployer, NoArgs};

    #[test]
    fn deployer_is_authorized_caller() {
        let env = odra_test::env();
        let deployer = env.get_account(0);
        env.set_caller(deployer);
        let vault = PayrollVault::deploy(&env, NoArgs);
        assert_eq!(vault.authorized_caller(), deployer);
    }

    #[test]
    fn records_payment_and_accumulates() {
        let env = odra_test::env();
        let deployer = env.get_account(0);
        env.set_caller(deployer);
        let mut vault = PayrollVault::deploy(&env, NoArgs);

        vault.record_payment("agent-007".to_string(), U256::from(100));
        vault.record_payment("agent-007".to_string(), U256::from(50));

        // Earnings accumulate across payments.
        assert_eq!(
            vault.get_earnings("agent-007".to_string()),
            U256::from(150)
        );
        // Other agents are unaffected and default to zero.
        assert_eq!(vault.get_earnings("agent-999".to_string()), U256::zero());
    }

    #[test]
    fn emits_payment_recorded_event() {
        let env = odra_test::env();
        env.set_caller(env.get_account(0));
        let mut vault = PayrollVault::deploy(&env, NoArgs);

        vault.record_payment("agent-007".to_string(), U256::from(42));

        assert!(env.emitted_event(
            &vault,
            PaymentRecorded {
                agent_id: "agent-007".to_string(),
                amount: U256::from(42),
                block_time: 0
            }
        ));
    }

    #[test]
    fn unauthorized_caller_cannot_record_payment() {
        let env = odra_test::env();
        let deployer = env.get_account(0);
        let attacker = env.get_account(1);

        env.set_caller(deployer);
        let mut vault = PayrollVault::deploy(&env, NoArgs);

        // Switch to a different account and attempt the call.
        env.set_caller(attacker);
        assert_eq!(
            vault
                .try_record_payment("agent-007".to_string(), U256::from(100))
                .unwrap_err(),
            Error::NotAuthorized.into()
        );

        // State must be unchanged.
        env.set_caller(deployer);
        assert_eq!(vault.get_earnings("agent-007".to_string()), U256::zero());
    }
}

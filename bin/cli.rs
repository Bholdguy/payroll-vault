//! CLI tool to deploy and interact with the PayrollVault contract.

use odra::host::{HostEnv, NoArgs};
use odra_cli::{
    deploy::DeployScript, ContractProvider, DeployedContractsContainer, DeployerExt, OdraCli
};
use payroll_vault::vault::PayrollVault;

/// Deploys the `PayrollVault` and adds it to the container.
pub struct PayrollVaultDeployScript;

impl DeployScript for PayrollVaultDeployScript {
    fn deploy(
        &self,
        env: &HostEnv,
        container: &mut DeployedContractsContainer
    ) -> Result<(), odra_cli::deploy::Error> {
        let _vault = PayrollVault::load_or_deploy(
            &env,
            NoArgs,
            container,
            350_000_000_000 // Adjust gas limit as needed
        )?;

        Ok(())
    }
}

/// Main function to run the CLI tool.
pub fn main() {
    OdraCli::new()
        .about("CLI tool for payroll-vault smart contract")
        .deploy(PayrollVaultDeployScript)
        .contract::<PayrollVault>()
        .build()
        .run();
}

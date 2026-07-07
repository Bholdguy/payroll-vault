# x402 -> payrollvault integration

patched version of the casper-x402 example resource server
(github.com/make-software/casper-x402, examples/server/main.go).

added: in the OnAfterSettle hook, when the facilitator reports a
successful settlement, the server invokes payrollvault's
record_payment entry point via the project cli — agent_id is the
payer's account (00-prefixed account hash), amount is the settled
amount in raw token units (X402 has 9 decimals; 7.5 X402 = 7500000000).

result: every settled x402 machine payment is automatically written
into the on-chain agent earnings ledger. demonstrated on casper testnet.

to run: drop this file over examples/server/main.go in the casper-x402
repo, configure per that repo's docs, and set PAYROLL_CLI / PAYROLL_DIR
env vars to point at a built payroll-vault cli and project folder.

# Insurance Treasury Contract

This contract implements a segregated insurance fund for the vesting vault system, providing financial backstop against critical smart contract vulnerabilities.

## Features

- **Micro-fee Collection**: Automatically collects 1% of all DeFi yield generated through the yield adapter
- **Segregated Storage**: Fund is physically separated from main vesting token mappings
- **Multi-signature Security**: Requires unanimous approval from 5-of-5 Security Council members
- **Timelock Protection**: 14-day timelock on bailout executions
- **Asset Restrictions**: Only accepts USDC and XLM to prevent correlated market crashes
- **Transparency**: Emits events for all fund capitalizations

## Security Considerations

- Unauthorized access attempts result in `Error::UnauthorizedBailoutAccess`
- Absolute immutability against standard admin/relayer interventions
- Extreme consensus requirements for fund disbursement

## Usage

1. Initialize with security council and supported assets
2. Authorize yield adapters to deposit fees
3. Yield adapter automatically routes 1% of yield to insurance fund
4. In case of critical vulnerability, security council can request and execute bailout after timelock

## Events

- `InsuranceFundCapitalized`: Emitted when yield is routed to the fund
- `BailoutRequested`: Emitted when a bailout is requested
- `BailoutExecuted`: Emitted when a bailout is successfully executed
# Build Anchor binary
anchor build

# Sync Program ID
anchor keys sync

# Deploy to Devnet / Mainnet
anchor deploy --provider.cluster mainnet --provider.wallet ./deployer-key.json
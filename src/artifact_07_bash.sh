# 1. Build optimized smart contract artifact
anchor build

# 2. Deploy to Solana Devnet
solana config set --url https://api.devnet.solana.com
anchor deploy --provider.cluster devnet

# 3. Initialize on-chain configuration
npx ts-node -e "
import { Program, AnchorProvider, Wallet } from '@coral-xyz/anchor';
import { Connection, Keypair } from '@solana/web3.js';
// Run programmatic initialization call
"
# 1. Clone repository & install dependencies
git clone https://github.com/enterprise-solana/price-sentinel.git
cd price-sentinel
npm install

# 2. Compile Anchor Program
anchor build

# 3. Deploy to Solana Devnet / Mainnet
solana config set --url https://api.devnet.solana.com
anchor deploy --program-name price_sentinel

# 4. Run automated test suite
npm run test

# 5. Launch CLI parity engine
npm run start:sentinel
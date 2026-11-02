# 1. Clone repository & install dependencies
git clone https://github.com/solana-sentinel/solprice-compare.git
cd solprice-compare
npm install

# 2. Configure environment credentials
cp .env.example .env
# Fill in SOLANA_RPC_URL, SLACK_WEBHOOK_URL, and RESEND_API_KEY

# 3. Build Anchor on-chain program
anchor build
anchor deploy --provider.cluster devnet

# 4. Launch Sentinel Daemon in Production Mode
npm run build
npm start
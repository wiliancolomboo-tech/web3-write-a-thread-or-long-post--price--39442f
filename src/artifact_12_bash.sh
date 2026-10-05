# 1. Clone repository & configure environment
cp .env.example .env
nano .env

# Required environment secrets:
# SOLANA_RPC_URL="https://your-dedicated-triton-or-helius-rpc"
# DISCORD_WEBHOOK_URL="https://discord.com/api/webhooks/..."
# RESEND_API_KEY="re_123456789"
# ALERT_EMAIL_RECIPIENT="ops-desk@fund.com"

# 2. Build and verify test suites
npm install
npm test

# 3. Compile Anchor Quote Guard (Optional On-Chain Program)
anchor build
anchor test

# 4. Turnkey deployment via Docker Compose
docker-compose up -d --build

# 5. Live log tailing & telemetry streaming
docker-compose logs -f solprism-comparator
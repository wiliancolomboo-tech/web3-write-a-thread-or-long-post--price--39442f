# 1. Clone repository and install dependencies
git clone https://github.com/sponsor-org/solana-price-comparison.git
cd solana-price-comparison
npm install

# 2. Configure Environment
cat <<EOF > .env
DISCORD_WEBHOOK_URL=https://api.example.com/webhooks/alerts/placeholder
ALERT_SPREAD_BPS=10
EOF

# 3. Execute Deterministic Test Suite
npm test

# 4. Run Single Direct Comparison Sweep
npm run build && npm start

# 5. One-Click Dockerized Deployment
docker compose up -d --build
docker compose logs -f solcompare-agent
# 1. Build and verify the Anchor program
anchor build

# 2. Execute on-chain tests in localnet
anchor test

# 3. Deploy to Solana Devnet / Mainnet
solana config set --url https://api.mainnet-beta.solana.com
anchor deploy --program-name sol_price_verifier

# 4. Launch the Dockerized enterprise comparison monitor
docker compose up -d --build

# 5. Inspect real-time execution logs
docker logs -f sol-price-engine
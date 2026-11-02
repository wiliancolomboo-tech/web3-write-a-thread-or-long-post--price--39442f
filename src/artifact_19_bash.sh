# 1. Clone repository
git clone https://github.com/sponsor-org/solcompare.git && cd solcompare

# 2. Build and verify the Rust core engine
cd solcompare-core
cargo test --release

# 3. Spin up the containerized enterprise sentinel
cd ..
cp .env.example .env
docker compose up -d --build

# 4. View live institutional price comparison streaming logs
docker compose logs -f solcompare-agent
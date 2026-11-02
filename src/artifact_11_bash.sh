# 1. Clone & Enter Directory
cd solprice-engine

# 2. Configure Environment
cp .env.example .env
nano .env

# 3. Build & Test Locally
npm install
npm run build
npm test

# 4. Turnkey Docker Deployment
docker-compose up -d --build

# 5. Inspect Live Log Streaming
docker-compose logs -f solprice-comparator
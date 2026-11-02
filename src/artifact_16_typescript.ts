import http from "http";

export function initHealthServer(port: number = 8080) {
  const server = http.createServer((req, res) => {
    if (req.url === "/health") {
      res.writeHead(200, { "Content-Type": "application/json" });
      res.end(
        JSON.stringify({
          status: "UP",
          engine: "AegisPrice Solana Aggregator",
          timestamp: new Date().toISOString(),
          uptimeSecs: process.uptime(),
        })
      );
    } else {
      res.writeHead(404);
      res.end();
    }
  });

  server.listen(port, () => {
    console.log(`Healthcheck telemetry listening on :${port}/health`);
  });
}
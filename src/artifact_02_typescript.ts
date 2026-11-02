export interface TokenConfig {
  symbol: string;
  mint: string;
  decimals: number;
}

export interface PoolQuote {
  venue: "RaydiumCPMM" | "OrcaWhirlpool" | "PhoenixCLOB" | "PythOracle";
  tokenIn: string;
  tokenOut: string;
  amountIn: number;
  expectedAmountOut: number;
  effectivePrice: number;
  priceImpactBps: number;
  feePaid: number;
  timestamp: number;
}

export interface ComparisonResult {
  tokenIn: string;
  tokenOut: string;
  amountIn: number;
  bestVenue: string;
  bestAmountOut: number;
  worstVenue: string;
  worstAmountOut: number;
  spreadBps: number;
  quotes: PoolQuote[];
  timestamp: number;
}

export interface EngineConfig {
  rpcUrl: string;
  wsUrl: string;
  alertWebhookUrl?: string;
  minSpreadThresholdBps: number;
  pollIntervalMs: number;
}
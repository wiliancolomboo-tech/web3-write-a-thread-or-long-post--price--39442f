export enum VenueId {
  ORCA = 0,
  RAYDIUM = 1,
  PHOENIX = 2,
  JUPITER = 3,
}

export interface QuoteComparison {
  venue: VenueId;
  venueName: string;
  inAmountSol: number;
  outAmountUsdc: number;
  effectivePriceUsdcPerSol: number;
  slippageBps: number;
  estimatedFeeSol: number;
  netRealizedPriceUsdc: number;
}

export interface ComparisonReport {
  timestamp: string;
  slot: number;
  pair: string;
  quotes: QuoteComparison[];
  bestVenue: string;
  worstVenue: string;
  spreadBps: number;
  arbOpportunity: boolean;
}
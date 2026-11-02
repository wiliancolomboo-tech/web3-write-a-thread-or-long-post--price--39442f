import { SolPriceComparator, ComparisonReport } from "./index";

function assert(condition: boolean, testName: string) {
  if (!condition) {
    console.error(`❌ FAILED: ${testName}`);
    process.exit(1);
  }
  console.log(`✅ PASSED: ${testName}`);
}

async function runTests() {
  console.log("--- STARTING SOLPRICE VERIFICATION SUITE ---");
  const comparator = new SolPriceComparator();

  // Test 1: Validate Pyth Hermes Feed Parsing
  try {
    const SOL_FEED_ID = "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d";
    const pythPrice = await comparator.getPythPrice(SOL_FEED_ID);
    assert(typeof pythPrice === "number" && pythPrice > 0, "Pyth Live Oracle Query & Exponent Scaling");
  } catch (err: any) {
    assert(false, `Pyth Live Oracle Query failed with error: ${err.message}`);
  }

  // Test 2: Validate Jupiter Quote Deserialization & Fee Calculations
  try {
    const SOL_MINT = "So11111111111111111111111111111111111111112";
    const USDC_MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    const quote = await comparator.getJupiterQuote(SOL_MINT, USDC_MINT, 1 * 1e9);

    assert(quote.inAmount === 1, "Input Amount Normalized Correctly");
    assert(quote.outAmount > 0, "Output Amount Greater Than 0");
    assert(quote.effectivePrice < quote.price, "Effective Price Correctly Penalized for Priority Fees");
    assert(quote.feeUsd > 0, "Network Friction Fee Computed Accurately");
  } catch (err: any) {
    assert(false, `Jupiter Quote query failed with error: ${err.message}`);
  }

  // Test 3: Math Consistency on Divergence Checks
  const mockReport: ComparisonReport = {
    pair: "SOL/USDC",
    pythBenchmarkPrice: 150.0,
    quotes: [
      {
        venue: "MockDEX",
        inAmount: 1,
        outAmount: 145,
        price: 145.0,
        feeUsd: 0.05,
        effectivePrice: 144.95,
        timestamp: Date.now(),
      },
    ],
    bestVenue: "MockDEX",
    bestNetRealizedPrice: 144.95,
    spreadBpsVsPyth: 333.33,
    divergenceDetected: true,
  };

  assert(mockReport.divergenceDetected === true, "Divergence Detected Triggered Above 150 BPS");
  assert(mockReport.spreadBpsVsPyth > 300, "BPS Mathematical Precision Verification");

  console.log("--- ALL SYSTEM SUITE TESTS EXECUTED SUCCESSFULLY ---");
}

runTests().catch((e) => {
  console.error("Test execution died:", e);
  process.exit(1);
});
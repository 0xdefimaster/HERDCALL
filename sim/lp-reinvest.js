// "Back to liquidity" share of the locked position's fees (CURVE_SPEC §5.7): the pool pays fees in SOL, but adding to
// a full-range position needs SOL and tokens at the pool price, so part of the SOL must first buy tokens in the same
// pool. A permissionless call can be sandwiched: someone buys before it and sells after. This models the pool as
// constant product (a full-range DAMM v2 position is very close to it) with the fee taken in SOL, and finds when a
// sandwich pays. Floats: this is an economic model, not the program's integer math.

/** Pool: x SOL, y tokens, fee f on the SOL side of every swap (DAMM "fees in token B"). */
export function buy(pool, sol, f) {
  const net = sol * (1 - f);
  const out = (pool.y * net) / (pool.x + net);
  return { pool: { x: pool.x + net, y: pool.y - out }, out };
}

export function sell(pool, tokens, f) {
  const gross = (pool.x * tokens) / (pool.y + tokens);
  return { pool: { x: pool.x - gross, y: pool.y + tokens }, out: gross * (1 - f) };
}

/** Swap part of `p` SOL into tokens and add both at the new pool ratio (bisection on the swapped part). */
export function reinvest(pool, p, f) {
  let lo = 0;
  let hi = p;
  for (let i = 0; i < 100; i += 1) {
    const s = (lo + hi) / 2;
    const { pool: q, out } = buy(pool, s, f);
    // SOL left per token bought, against the pool's SOL per token.
    if ((p - s) / out > q.x / q.y) lo = s;
    else hi = s;
  }
  const { pool: q, out } = buy(pool, lo, f);
  const sol = p - lo;
  return { pool: { x: q.x + sol, y: q.y + out }, swapped: lo, addedSol: sol, addedTokens: out };
}

/** Attacker profit (SOL) of buying `a` before the reinvest and selling everything right after. */
export function sandwichProfit(pool, p, a, f) {
  const front = buy(pool, a, f);
  const r = reinvest(front.pool, p, f);
  const back = sell(r.pool, front.out, f);
  return back.out - a;
}

/** Best sandwich over a grid of attack sizes (in multiples of the pool's SOL reserve). */
export function bestSandwich(pool, p, f) {
  let best = { a: 0, profit: 0 };
  for (let i = 1; i <= 400; i += 1) {
    const a = pool.x * (i / 200); // up to 2x the pool
    const profit = sandwichProfit(pool, p, a, f);
    if (profit > best.profit) best = { a, profit };
  }
  return best;
}

/** Largest reinvest (share of the pool's SOL) no sandwich profits from. */
export function safeShare(f, pool = { x: 100, y: 100 }) {
  let lo = 0;
  let hi = 1;
  for (let i = 0; i < 40; i += 1) {
    const m = (lo + hi) / 2;
    if (bestSandwich(pool, pool.x * m, f).profit > 0) hi = m;
    else lo = m;
  }
  return lo;
}

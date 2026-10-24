// Decisions of the HerdCall keeper, without any I/O (CURVE_SPEC §5.7). Prices are DAMM v2 √prices (Q64.64, BigInt).
//
// DAMM v2 has no TWAP or oracle, so the keeper keeps its own: it samples each pool's price and takes the median of the
// last window. The band it signs is that median ±0.25% in √price (about ±0.5% in price), inside the program's 2% limit.
// It does not reinvest while the current price is outside the band (a price pushed just before is not followed).

export const DEFAULTS = {
  sampleSecs: 30,
  windowSecs: 30 * 60,
  // A median needs this many samples spread over at least this share of the window.
  minSamples: 20,
  minCoverage: 0.8,
  claimEverySecs: 6 * 3600,
  reinvestEverySecs: 3600,
  // Alert when the share has been at or above the minimum for longer than this, then again every `alertRepeatSecs`.
  alertPendingSecs: 24 * 3600,
  alertRepeatSecs: 6 * 3600,
  reinvestMinLamports: 10_000_000n,
};

/** Half-width of the band in √price: 1/400 = 0.25% (about 0.5% in price). */
const BAND_DIVISOR = 400n;

export function median(values) {
  if (values.length === 0) return null;
  const s = [...values].sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  const m = Math.floor(s.length / 2);
  return s.length % 2 ? s[m] : (s[m - 1] + s[m]) / 2n;
}

/** Price samples of one pool: `[{ t, sqrt }]`, t in seconds. */
export function addSample(samples, t, sqrt, windowSecs) {
  const kept = samples.filter((s) => s.t > t - windowSecs);
  kept.push({ t, sqrt });
  return kept;
}

/** Median of the window, or `null` with the reason if there is not enough history yet. */
export function windowMedian(samples, now, cfg = DEFAULTS) {
  const inWindow = samples.filter((s) => s.t > now - cfg.windowSecs);
  if (inWindow.length < cfg.minSamples) return { median: null, reason: `${inWindow.length}/${cfg.minSamples} samples` };
  const span = inWindow[inWindow.length - 1].t - inWindow[0].t;
  if (span < cfg.windowSecs * cfg.minCoverage) return { median: null, reason: `samples span ${span}s of ${cfg.windowSecs}s` };
  return { median: median(inWindow.map((s) => s.sqrt)), reason: null };
}

export function band(m) {
  return [m - m / BAND_DIVISOR, m + m / BAND_DIVISOR];
}

export const inBand = ([lo, hi], p) => lo <= p && p <= hi;

/**
 * What to do for one launch now. `state` holds what the keeper remembers for it:
 * `{ samples, lastClaimAt, lastReinvestAt, pendingSince, lastAlertAt }` (times in seconds).
 * `cap`: lamports one call may take from the pool (see `reinvestCap`), if known.
 * Returns `{ claim, reinvest: { band } | null, skip: reason | null, alert: message | null, state }`.
 */
export function decide({ now, pending, current, cap = null, state, cfg = DEFAULTS }) {
  const next = { ...state };
  const out = { claim: false, reinvest: null, skip: null, alert: null };

  out.claim = now - (state.lastClaimAt ?? -Infinity) >= cfg.claimEverySecs;

  if (pending >= cfg.reinvestMinLamports) {
    next.pendingSince = state.pendingSince ?? now;
    const waited = now - next.pendingSince;
    if (waited > cfg.alertPendingSecs && now - (state.lastAlertAt ?? -Infinity) >= cfg.alertRepeatSecs) {
      out.alert = `${pending} lamports waiting to be reinvested for ${Math.floor(waited / 3600)}h`;
      next.lastAlertAt = now;
    }
    if (cap !== null && cap < cfg.reinvestMinLamports) {
      // The program would reject it: one call may take less than the minimum from this pool.
      out.skip = `pool too small: one call may take ${cap} lamports, below the minimum`;
    } else if (now - (state.lastReinvestAt ?? -Infinity) < cfg.reinvestEverySecs) {
      out.skip = 'reinvested less than an interval ago';
    } else {
      const { median: m, reason } = windowMedian(state.samples ?? [], now, cfg);
      if (m === null) out.skip = `no median yet (${reason})`;
      else if (!inBand(band(m), current)) out.skip = 'current price is outside the band around the median';
      else out.reinvest = { band: band(m) };
    }
  } else {
    next.pendingSince = null;
  }
  return { ...out, state: next };
}

// Health of the HerdCall keeper, without any I/O: which alerts to raise, how often, and whether it is healthy.
//
// Raised (besides the "share waiting more than a day" alert in logic.mjs):
// - the keeper's SOL balance is below what claims and reinvests need (fees and temporary token accounts);
// - the admin has appointed another keeper: this key can no longer reinvest (claims stay permissionless);
// - the loop keeps failing (RPC down or rate limited);
// - sending keeps failing for a launch (a bundle that never lands, an RPC refusing transactions).

export const MONITOR_DEFAULTS = {
  minBalanceLamports: 50_000_000n,
  maxLoopErrors: 3,
  maxSendFailures: 3,
  alertRepeatSecs: 6 * 3600,
  // Unhealthy if no loop finished within this many sample intervals.
  staleLoops: 4,
};

/** Alerts that apply now, each with a stable `key` for throttling. */
export function healthAlerts({ balance, isKeeper, loopErrors, sendFailures = {}, cfg = MONITOR_DEFAULTS }) {
  const out = [];
  if (balance !== null && balance < cfg.minBalanceLamports) {
    out.push({ key: 'balance', message: `keeper balance ${balance} lamports, below ${cfg.minBalanceLamports}: top it up` });
  }
  if (isKeeper === false) {
    out.push({ key: 'not-keeper', message: 'this key is no longer the reinvest keeper (the admin appointed another): reinvesting stopped' });
  }
  if (loopErrors >= cfg.maxLoopErrors) {
    out.push({ key: 'loop', message: `${loopErrors} loops in a row failed (RPC?)` });
  }
  for (const [launch, f] of Object.entries(sendFailures)) {
    if (f.count >= cfg.maxSendFailures) {
      out.push({ key: `send:${launch}`, message: `${f.count} ${f.what} attempts in a row failed for ${launch}: ${f.last}`, launch });
    }
  }
  return out;
}

/**
 * Throttle: an alert is sent when it first appears and then at most every `repeatSecs` while it stays. One that
 * clears and comes back is sent again at once. `sent`: `{ key: last sent at }`.
 */
export function throttle(sent, now, alerts, repeatSecs = MONITOR_DEFAULTS.alertRepeatSecs) {
  const next = {};
  const send = [];
  for (const a of alerts) {
    const last = sent[a.key];
    if (last === undefined || now - last >= repeatSecs) {
      send.push(a);
      next[a.key] = now;
    } else {
      next[a.key] = last;
    }
  }
  return { send, sent: next };
}

/** Counts consecutive failed sends per launch; a success clears it. */
export function recordSend(failures, launch, what, result) {
  const next = { ...failures };
  if (result.ok) delete next[launch];
  else next[launch] = { what, count: (failures[launch]?.what === what ? failures[launch].count : 0) + 1, last: String(result.err ?? 'failed') };
  return next;
}

/** `{ ok, reasons }` for the health endpoint. */
export function healthStatus({ now, lastLoopAt, sampleSecs, isKeeper, loopErrors, cfg = MONITOR_DEFAULTS }) {
  const reasons = [];
  if (lastLoopAt === null || now - lastLoopAt > sampleSecs * cfg.staleLoops) reasons.push('no loop finished recently');
  if (isKeeper === false) reasons.push('not the reinvest keeper');
  if (loopErrors >= cfg.maxLoopErrors) reasons.push('loop failing');
  return { ok: reasons.length === 0, reasons };
}

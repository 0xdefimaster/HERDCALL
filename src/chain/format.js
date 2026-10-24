// Display helpers for on-chain amounts (BN, bigint or number of base units).
export const LAMPORTS = 1_000_000_000n;
export const TOKEN_UNITS = 1_000_000n;

export const big = (x) => (typeof x === 'bigint' ? x : BigInt(x?.toString?.() ?? x ?? 0));

function decimal(units, scale, digits) {
  const v = big(units);
  const neg = v < 0n;
  const a = neg ? -v : v;
  const whole = a / scale;
  const frac = (a % scale).toString().padStart(scale.toString().length - 1, '0').slice(0, digits).replace(/0+$/, '');
  return `${neg ? '-' : ''}${whole.toLocaleString('en-US')}${frac ? `.${frac}` : ''}`;
}

export const sol = (lamports, digits = 4) => decimal(lamports, LAMPORTS, digits);
export const tokens = (units, digits = 2) => decimal(units, TOKEN_UNITS, digits);

/** Parses a decimal string into base units; null if not a positive amount with at most `decimals` places. */
export function parseUnits(text, decimals) {
  const m = /^\s*(\d*)(?:\.(\d*))?\s*$/.exec(text ?? '');
  if (!m || (!m[1] && !m[2])) return null;
  const frac = (m[2] ?? '').slice(0, decimals).padEnd(decimals, '0');
  const v = BigInt(m[1] || '0') * 10n ** BigInt(decimals) + BigInt(frac || '0');
  return v > 0n ? v : null;
}

export const PHASES = { lobby: 'Lobby', curve: 'Curve', breakingOut: 'Breaking out', migrated: 'On Meteora', cancelled: 'Cancelled' };
export const phaseOf = (launch) => Object.keys(launch.phase)[0];
export const short = (key) => {
  const s = key?.toBase58?.() ?? String(key ?? '');
  return s ? `${s.slice(0, 4)}…${s.slice(-4)}` : '';
};

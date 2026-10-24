import { test } from 'node:test';
import assert from 'node:assert/strict';
import { MONITOR_DEFAULTS, healthAlerts, healthStatus, recordSend, throttle } from './monitor.mjs';

const quiet = { balance: 1_000_000_000n, isKeeper: true, loopErrors: 0 };
const keys = (alerts) => alerts.map((a) => a.key).sort();

test('no alerts when all is well', () => {
  assert.deepEqual(healthAlerts(quiet), []);
});

test('low balance, replaced keeper, failing loop', () => {
  assert.deepEqual(keys(healthAlerts({ ...quiet, balance: MONITOR_DEFAULTS.minBalanceLamports - 1n })), ['balance']);
  assert.deepEqual(keys(healthAlerts({ ...quiet, balance: MONITOR_DEFAULTS.minBalanceLamports })), []);
  assert.deepEqual(keys(healthAlerts({ ...quiet, balance: null })), [], 'unknown balance is not an alert');
  assert.deepEqual(keys(healthAlerts({ ...quiet, isKeeper: false })), ['not-keeper']);
  assert.deepEqual(keys(healthAlerts({ ...quiet, loopErrors: 2 })), []);
  assert.deepEqual(keys(healthAlerts({ ...quiet, loopErrors: 3 })), ['loop']);
});

test('send failures count per launch and kind, and clear on success', () => {
  let f = {};
  for (let i = 0; i < 2; i++) f = recordSend(f, 'L1', 'reinvest', { ok: false, err: 'bundle dropped' });
  assert.deepEqual(keys(healthAlerts({ ...quiet, sendFailures: f })), []);
  f = recordSend(f, 'L1', 'reinvest', { ok: false, err: 'bundle dropped' });
  const [a] = healthAlerts({ ...quiet, sendFailures: f });
  assert.equal(a.key, 'send:L1');
  assert.match(a.message, /3 reinvest attempts .* bundle dropped/);
  // A different kind of send restarts the count; a success clears it.
  assert.equal(recordSend(f, 'L1', 'claim', { ok: false }).L1.count, 1);
  assert.equal(recordSend(f, 'L1', 'reinvest', { ok: true }).L1, undefined);
  assert.equal(recordSend(f, 'L2', 'claim', { ok: false }).L1.count, 3, 'other launches unaffected');
});

test('throttle: first at once, then every repeat interval, again at once after clearing', () => {
  const a = [{ key: 'balance', message: 'low' }];
  let s = {};
  let r = throttle(s, 0, a, 100);
  assert.equal(r.send.length, 1);
  s = r.sent;
  r = throttle(s, 50, a, 100);
  assert.equal(r.send.length, 0);
  s = r.sent;
  r = throttle(s, 100, a, 100);
  assert.equal(r.send.length, 1);
  s = r.sent;
  r = throttle(s, 110, [], 100);
  assert.deepEqual(r.sent, {}, 'cleared');
  r = throttle(r.sent, 120, a, 100);
  assert.equal(r.send.length, 1, 'back: sent at once');
});

test('health status', () => {
  const base = { now: 1000, lastLoopAt: 990, sampleSecs: 30, isKeeper: true, loopErrors: 0 };
  assert.deepEqual(healthStatus(base), { ok: true, reasons: [] });
  assert.equal(healthStatus({ ...base, lastLoopAt: null }).ok, false);
  assert.equal(healthStatus({ ...base, lastLoopAt: 1000 - 121 }).ok, false);
  assert.deepEqual(healthStatus({ ...base, isKeeper: false }).reasons, ['not the reinvest keeper']);
  assert.equal(healthStatus({ ...base, loopErrors: 3 }).ok, false);
});

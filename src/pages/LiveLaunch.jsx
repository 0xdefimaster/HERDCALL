import React, { useEffect, useState } from 'react';
import { Link, useParams } from 'react-router-dom';
import * as anchor from '@anchor-lang/core';
import { Bar, Card, CardTitle, PixelButton, Row, Tag, TokenAvatar } from '../components/ui';
import {
  ata, breakoutAccounts, claimAccounts, claimLpFeesAccounts, depositAccounts, depositKey, interfaceKey, keysOf,
  launchKeys, refundAccounts, releaseNameAccounts, settleAccounts, tradeAccounts, withdrawCreatorFeesAccounts,
} from '../../scripts/herdcall-client.mjs';
import { programError, useHerdCall, usePoll } from '../chain/HerdCallContext';
import { INTERFACE_OWNER, explorerTx } from '../chain/config';
import { LAMPORTS, PHASES, TOKEN_UNITS, big, parseUnits, phaseOf, short, sol, tokens } from '../chain/format';
import { lobbyShare, marketOf, projectLobby, withSlippage } from '../chain/quote';
import { formatClock } from './Lobby';
import { useWallet } from '../wallet/WalletContext';

const { web3, BN } = anchor;
const INPUT = 'min-h-[54px] px-3.5 border-[3px] border-hc-cream bg-hc-bg text-hc-cream font-terminal text-[30px] w-full';
const BREAKOUT_CU = 1_400_000;
const SLIPPAGE_BPS = 100; // 1%
const bn = (x) => new BN(x.toString());

function useNow() {
  const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));
  useEffect(() => {
    const id = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(id);
  }, []);
  return now;
}

function useAction() {
  const [state, setState] = useState({ busy: null, result: null });
  const run = async (name, fn) => {
    setState({ busy: name, result: null });
    try {
      setState({ busy: null, result: { name, signature: await fn() } });
    } catch (e) {
      setState({ busy: null, result: { name, error: programError(e?.logs) ?? e?.message ?? String(e) } });
    }
  };
  return { ...state, run };
}

function Result({ result }) {
  if (!result) return null;
  if (result.error) return <p role="alert" className="m-0 text-hc-down break-words">{result.error}</p>;
  return (
    <p role="status" className="m-0 text-hc-up break-words">
      Confirmed. <a href={explorerTx(result.signature)} target="_blank" rel="noreferrer" className="underline">View transaction</a>
    </p>
  );
}

function Stat({ label, children }) {
  return (
    <div className="bg-hc-bg border-[3px] border-hc-line p-3">
      <div className="text-sm text-hc-muted">{label}</div>
      <div className="font-terminal text-[30px] leading-none">{children}</div>
    </div>
  );
}

function AmountInput({ id, label, value, onChange, unit }) {
  return (
    <div className="flex flex-col gap-2">
      <label htmlFor={id} className="text-[15px] text-hc-muted">{label} ({unit})</label>
      <input id={id} type="text" inputMode="decimal" value={value} onChange={(e) => onChange(e.target.value)} className={INPUT} />
    </div>
  );
}

function ConnectFirst() {
  const { connect } = useWallet();
  return <PixelButton onClick={connect} className="min-h-[56px]">Connect wallet</PixelButton>;
}

// ------------------------------------------------------------------ phases

function LobbyPanel({ l, k, mine, now, act }) {
  const { program, user, send } = useHerdCall();
  const [amount, setAmount] = useState('0.1');
  const total = big(l.totalDeposits);
  const lamports = parseUnits(amount, 9);
  const closed = now >= l.closeAt.toNumber();
  const expired = now >= l.settleDeadline.toNumber();
  const projection = projectLobby(l, closed || !lamports ? total : total + lamports);
  const myDeposit = big(mine?.amount ?? 0);
  const preview = closed || !lamports ? myDeposit : myDeposit + lamports;
  const share = projection.empty ? 0n : lobbyShare(preview, closed || !lamports ? total : total + lamports, projection.X);
  const remaining = Math.max(0, l.closeAt.toNumber() - now);
  const minTotal = big(l.params.minTotal);

  return (
    <>
      <Card aria-label="Bell countdown" highlight className="flex-[2_1_420px] min-w-0 p-8 flex flex-col items-center text-center gap-4">
        <div className="font-pixel text-[15px] text-hc-muted">{closed ? 'The bell has rung' : 'The bell rings in'}</div>
        <div aria-live="polite" className="font-terminal leading-[0.85] text-hc-orange text-[clamp(90px,13vw,170px)]">{formatClock(remaining)}</div>
        <div className="w-full grid grid-cols-3 gap-3 text-left">
          <Stat label="Herd gathered">{l.depositors}</Stat>
          <Stat label="SOL in the pen">{sol(total, 3)}</Stat>
          <Stat label="Minimum">{sol(minTotal, 2)}</Stat>
        </div>
        <Bar value={Number(total >= minTotal ? 100n : (total * 100n) / minTotal)} className="w-full h-5" />
        <div className="w-full text-left p-3.5 border-[3px] border-hc-orange bg-hc-bg flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1">
          <span className="text-hc-body text-base">{closed ? 'The pen buys' : 'If the bell rang now, the pen buys'}</span>
          <span className="font-terminal text-[30px] leading-none text-hc-orange">{projection.empty ? 'nothing yet (below the minimum)' : `${tokens(projection.X, 0)} $${l.ticker}`}</span>
        </div>
        <p className="m-0 text-left text-hc-body text-base">
          One buy at one price when the bell rings; everyone gets tokens in proportion to their deposit. A Lobby below {sol(minTotal, 2)} SOL opens no curve and everyone takes their deposit back.
        </p>
      </Card>
      <Card aria-label="Join the herd" className="flex-[1_1_300px] min-w-0 flex flex-col gap-4">
        <CardTitle className="text-[22px]">{closed ? 'Settle' : 'Join the herd'}</CardTitle>
        {!closed && (
          <>
            <AmountInput id="deposit" label="Amount" value={amount} onChange={setAmount} unit="SOL" />
            {!user ? <ConnectFirst /> : (
              <PixelButton
                className="min-h-[56px]"
                disabled={!!act.busy || !lamports}
                onClick={() => act.run('deposit', () => send(program.methods.deposit(bn(lamports)).accountsStrict(depositAccounts(user, k))))}
              >
                {act.busy === 'deposit' ? 'Sending…' : 'Answer the call'}
              </PixelButton>
            )}
          </>
        )}
        {closed && !expired && (
          <>
            <p className="m-0 text-hc-body">Anyone can ring the bell now. It must happen before the settle window ends, or everyone takes their deposit back.</p>
            {!user ? <ConnectFirst /> : (
              <PixelButton className="min-h-[56px]" disabled={!!act.busy} onClick={() => act.run('settle', () => send(program.methods.settle().accountsStrict(settleAccounts(k, l.creator))))}>
                {act.busy === 'settle' ? 'Sending…' : 'Ring the bell (settle)'}
              </PixelButton>
            )}
          </>
        )}
        {expired && mine && !mine.withdrawn && (
          <PixelButton className="min-h-[56px]" disabled={!!act.busy} onClick={() => act.run('refund', () => send(program.methods.refund().accountsStrict(refundAccounts(user, k))))}>
            Not settled in time: refund {sol(mine.amount)} SOL
          </PixelButton>
        )}
        <Row label="Your deposit"><span className="font-terminal text-[22px]">{sol(myDeposit)} SOL</span></Row>
        <Row label={closed ? 'Your tokens' : 'You would get'}><span className="font-terminal text-[22px]">{share > 0n ? `${tokens(share)} $${l.ticker}` : '—'}</span></Row>
        <Result result={act.result} />
      </Card>
    </>
  );
}

function ClaimCard({ l, k, mine, act }) {
  const { program, user, send } = useHerdCall();
  if (!mine || mine.withdrawn) return null;
  const share = lobbyShare(big(mine.amount), big(l.totalDeposits), big(l.lobbyTokens));
  return (
    <Card highlight className="flex flex-col gap-3">
      <CardTitle>Your Lobby tokens</CardTitle>
      <Row label="Deposited"><span className="font-terminal text-[22px]">{sol(mine.amount)} SOL</span></Row>
      <Row label="Tokens"><span className="font-terminal text-[22px]">{tokens(share)} ${l.ticker}</span></Row>
      <PixelButton disabled={!!act.busy} onClick={() => act.run('claim', () => send(program.methods.claim().accountsStrict(claimAccounts(user, k))))}>
        {act.busy === 'claim' ? 'Sending…' : 'Claim'}
      </PixelButton>
    </Card>
  );
}

function TradeCard({ l, k, balance, act }) {
  const { program, user, send } = useHerdCall();
  const [side, setSide] = useState('buy');
  const [amount, setAmount] = useState('0.1');
  const m = marketOf(l);
  const iface = INTERFACE_OWNER ? interfaceKey(new web3.PublicKey(INTERFACE_OWNER)) : null;
  let quote = null;
  let error = null;
  const units = parseUnits(amount, side === 'buy' ? 9 : 6);
  try {
    if (units && side === 'buy') quote = m.quoteBuyExactIn(units);
    if (units && side === 'sell') quote = m.quoteSell(units);
  } catch (e) {
    error = side === 'sell' && units > big(l.sold) ? 'More than the curve has sold' : e.message;
  }
  const buy = () => act.run('buy', () => send(program.methods.buyExactIn(bn(units), bn(withSlippage(quote.tokens, SLIPPAGE_BPS))).accountsStrict(tradeAccounts(user, k, iface))));
  const sell = () => act.run('sell', () => send(program.methods.sell(bn(units), bn(withSlippage(quote.received, SLIPPAGE_BPS))).accountsStrict(tradeAccounts(user, k, iface))));

  return (
    <Card aria-label="Trade" className="flex-[1_1_300px] min-w-0 flex flex-col gap-4">
      <div role="group" aria-label="Side" className="grid grid-cols-2 gap-2">
        {['buy', 'sell'].map((s) => (
          <button
            key={s}
            type="button"
            aria-pressed={side === s}
            onClick={() => { setSide(s); setAmount(s === 'buy' ? '0.1' : ''); }}
            className={`min-h-[48px] border-[3px] font-pixel text-sm uppercase cursor-pointer ${side === s ? 'border-hc-bg bg-hc-orange text-hc-bg' : 'border-hc-line bg-hc-bg text-hc-cream'}`}
          >
            {s}
          </button>
        ))}
      </div>
      <AmountInput id="trade-amount" label={side === 'buy' ? 'You pay' : 'You sell'} value={amount} onChange={setAmount} unit={side === 'buy' ? 'SOL' : `$${l.ticker}`} />
      {side === 'buy' && quote && (
        <>
          <Row label="You get"><span className="font-terminal text-[22px]">{tokens(quote.tokens)} ${l.ticker}</span></Row>
          <Row label="Fee (1%)"><span className="font-terminal text-[22px]">{sol(quote.fee, 6)} SOL</span></Row>
          {quote.refund > 0n && <Row label="Refunded (curve end)"><span className="font-terminal text-[22px]">{sol(quote.refund, 6)} SOL</span></Row>}
        </>
      )}
      {side === 'sell' && quote && (
        <>
          <Row label="You get"><span className="font-terminal text-[22px]">{sol(quote.received, 6)} SOL</span></Row>
          <Row label="Fee (1%)"><span className="font-terminal text-[22px]">{sol(quote.fee, 6)} SOL</span></Row>
        </>
      )}
      {error && <p className="m-0 text-hc-warn">{error}</p>}
      <Row label="Slippage limit"><span className="font-terminal text-[22px]">{SLIPPAGE_BPS / 100}%</span></Row>
      {!user ? <ConnectFirst /> : side === 'buy' ? (
        <PixelButton className="min-h-[56px]" disabled={!!act.busy || !quote?.tokens} onClick={buy}>{act.busy === 'buy' ? 'Sending…' : `Buy $${l.ticker}`}</PixelButton>
      ) : (
        <PixelButton className="min-h-[56px]" disabled={!!act.busy || !quote || units > balance} onClick={sell}>{act.busy === 'sell' ? 'Sending…' : `Sell $${l.ticker}`}</PixelButton>
      )}
      <Row label="Your balance"><span className="font-terminal text-[22px]">{tokens(balance)} ${l.ticker}</span></Row>
      <Result result={act.result} />
    </Card>
  );
}

function CurvePanel({ l }) {
  const sold = big(l.sold);
  const supply = big(l.params.curveSupply);
  // Spot price in SOL per whole token (display only): v / t lamports per base unit.
  const price = (Number(big(l.v)) / Number(big(l.t))) * (Number(TOKEN_UNITS) / Number(LAMPORTS));
  return (
    <Card highlight className="flex-[2_1_420px] min-w-0 flex flex-col gap-4">
      <CardTitle>{phaseOf(l) === 'breakingOut' ? 'Curve sold out' : 'Bonding curve'}</CardTitle>
      <Bar value={Number((sold * 100n) / supply)} className="w-full h-6" />
      <div className="grid grid-cols-2 gap-3">
        <Stat label="Sold">{tokens(sold, 0)}</Stat>
        <Stat label="Of">{tokens(supply, 0)}</Stat>
        <Stat label="Reserve (SOL)">{sol(l.reserve, 3)}</Stat>
        <Stat label="Price (SOL per token)">{price.toPrecision(4)}</Stat>
      </div>
      <p className="m-0 text-hc-body">When the curve sells out, anyone can move it to Meteora DAMM v2 at the last curve price. The whole position is locked for good.</p>
    </Card>
  );
}

function BreakoutCard({ l, k, act }) {
  const { program, user, send, config } = useHerdCall();
  return (
    <Card className="flex-[1_1_300px] min-w-0 flex flex-col gap-3">
      <CardTitle>Breakout</CardTitle>
      <p className="m-0 text-hc-body">
        Permissionless. The caller fronts about 0.05 SOL for the pool accounts and gets the breakout fee ({sol(l.params.migrationFee, 3)} SOL) back from the reserve.
      </p>
      {!user ? <ConnectFirst /> : (
        <PixelButton
          className="min-h-[56px]"
          disabled={!!act.busy || !config}
          onClick={() => act.run('breakout', () => send(program.methods.breakout().accountsStrict(breakoutAccounts(user, k, config.dexConfig)), { computeUnits: BREAKOUT_CU }))}
        >
          {act.busy === 'breakout' ? 'Sending…' : 'Break out'}
        </PixelButton>
      )}
      <Result result={act.result} />
    </Card>
  );
}

function PoolPanel({ l, k, act }) {
  const { program, user, send } = useHerdCall();
  return (
    <Card highlight className="flex-[2_1_420px] min-w-0 flex flex-col gap-3">
      <CardTitle>On Meteora DAMM v2</CardTitle>
      <Row label="Pool"><span className="font-terminal text-xl" title={l.dexPool.toBase58()}>{short(l.dexPool)}</span></Row>
      <Row label="Position (locked for good)"><span className="font-terminal text-xl" title={l.dexPosition.toBase58()}>{short(l.dexPosition)}</span></Row>
      <Row label="Opened with"><span className="font-terminal text-xl">{sol(l.lpNative, 3)} SOL + {tokens(l.lpTokens, 0)} tokens</span></Row>
      <Row label="Burned at breakout"><span className="font-terminal text-xl">{tokens(l.burned, 0)} tokens</span></Row>
      <Row label="LP fees claimed"><span className="font-terminal text-xl">{sol(l.lpFeesClaimed, 6)} SOL</span></Row>
      <Row label="Back to liquidity: waiting"><span className="font-terminal text-xl">{sol(l.lpReinvestPending, 6)} SOL</span></Row>
      <Row label="Back to liquidity: added"><span className="font-terminal text-xl">{sol(l.lpReinvested, 6)} SOL</span></Row>
      <p className="m-0 text-hc-body">Anyone can claim the position's fees; they are split at once. Only the keeper adds the liquidity share, inside its price band.</p>
      {!user ? <ConnectFirst /> : (
        <PixelButton disabled={!!act.busy} onClick={() => act.run('claim-lp', () => send(program.methods.claimLpFees().accountsStrict(claimLpFeesAccounts(user, k, { pool: l.dexPool }))))}>
          {act.busy === 'claim-lp' ? 'Sending…' : 'Claim pool fees'}
        </PixelButton>
      )}
      <Result result={act.result} />
    </Card>
  );
}

function CreatorCard({ l, k, creatorFees, act }) {
  const { program, user, send } = useHerdCall();
  if (!user || !user.equals(l.creator)) return null;
  const phase = phaseOf(l);
  return (
    <Card className="flex flex-col gap-3">
      <CardTitle>Creator</CardTitle>
      <Row label="Fees to withdraw"><span className="font-terminal text-[22px]">{sol(creatorFees, 6)} SOL</span></Row>
      <PixelButton disabled={!!act.busy || creatorFees <= 0n} onClick={() => act.run('creator-fees', () => send(program.methods.withdrawCreatorFees().accountsStrict(withdrawCreatorFeesAccounts(user, k))))}>
        Withdraw fees
      </PixelButton>
      {phase === 'cancelled' && (
        <PixelButton variant="secondary" disabled={!!act.busy} onClick={() => act.run('release', () => send(program.methods.releaseName().accountsStrict(releaseNameAccounts(k, user))))}>
          Release name and ticker (bond back)
        </PixelButton>
      )}
    </Card>
  );
}

// ------------------------------------------------------------------ page

export default function LiveLaunch() {
  const { mint } = useParams();
  const hc = useHerdCall();
  const act = useAction();
  const now = useNow();
  let mintKey = null;
  try {
    mintKey = new web3.PublicKey(mint);
  } catch {
    /* shown below */
  }
  const { data, error, loading } = usePoll(async () => {
    if (!hc.program || !mintKey) return null;
    const launchKey = launchKeys(mintKey, 'X', 'x').launch;
    const l = await hc.program.account.launch.fetchNullable(launchKey);
    if (!l) return { l: null };
    const k = keysOf(l);
    const [mine, bal, fees, rent] = await Promise.all([
      hc.user ? hc.program.account.deposit.fetchNullable(depositKey(k.launch, hc.user)) : null,
      hc.user ? hc.connection.getTokenAccountBalance(ata(hc.user, k.mint)).then((r) => BigInt(r.value.amount), () => 0n) : 0n,
      hc.connection.getBalance(k.creatorFees),
      hc.connection.getMinimumBalanceForRentExemption(9),
    ]);
    return { l, k, mine, balance: bal, creatorFees: BigInt(Math.max(0, fees - rent)) };
  }, [hc.program, mint, hc.user?.toBase58()]);

  if (!hc.enabled) return <main className="max-w-[1240px] mx-auto px-6 pt-8"><Card>No cluster configured. See Setup.</Card></main>;
  if (!mintKey) return <main className="max-w-[1240px] mx-auto px-6 pt-8"><Card>Not a mint address.</Card></main>;
  if (loading && !data) return <main className="max-w-[1240px] mx-auto px-6 pt-8"><Card>Loading…</Card></main>;
  if (error && !data) return <main className="max-w-[1240px] mx-auto px-6 pt-8"><Card role="alert">{String(error.message ?? error)}</Card></main>;
  if (!data?.l) return <main className="max-w-[1240px] mx-auto px-6 pt-8"><Card>No HerdCall launch for this mint. <Link to="/" className="underline">Back to the board</Link></Card></main>;

  const { l, k, mine, balance, creatorFees } = data;
  const phase = phaseOf(l);
  return (
    <main className="max-w-[1240px] mx-auto px-6 pt-8 pb-12 flex flex-col gap-6">
      <div className="flex flex-wrap items-center gap-3.5">
        <TokenAvatar initial={l.ticker.slice(0, 1)} />
        <div>
          <h1 className="m-0 font-pixel font-bold text-2xl leading-tight">{l.name}</h1>
          <div className="font-terminal text-xl text-hc-muted">${l.ticker}</div>
        </div>
        <div className="flex flex-wrap gap-2 ml-auto">
          <Tag tone={phase === 'cancelled' ? 'outline' : 'orange'}>{PHASES[phase]}</Tag>
          {l.bonded ? <Tag>Bonded</Tag> : <Tag tone="outline">Unbonded</Tag>}
          <Tag tone="outline">{hc.cluster}</Tag>
        </div>
      </div>
      <div className="flex flex-wrap gap-6 items-stretch">
        {phase === 'lobby' && <LobbyPanel l={l} k={k} mine={mine} now={now} act={act} />}
        {(phase === 'curve' || phase === 'breakingOut') && <CurvePanel l={l} />}
        {phase === 'curve' && <TradeCard l={l} k={k} balance={balance} act={act} />}
        {phase === 'breakingOut' && <BreakoutCard l={l} k={k} act={act} />}
        {phase === 'migrated' && <PoolPanel l={l} k={k} act={act} />}
        {phase === 'cancelled' && (
          <Card className="flex-[2_1_420px] flex flex-col gap-3">
            <CardTitle>Cancelled</CardTitle>
            <p className="m-0 text-hc-body">The Lobby did not reach the minimum, or was not settled in time. Every depositor takes their SOL back.</p>
            {mine && !mine.withdrawn && (
              <PixelButton disabled={!!act.busy} onClick={() => act.run('refund', () => hc.send(hc.program.methods.refund().accountsStrict(refundAccounts(hc.user, k))))}>
                Refund {sol(mine.amount)} SOL
              </PixelButton>
            )}
            <Result result={act.result} />
          </Card>
        )}
      </div>
      <div className="flex flex-wrap gap-6 items-start">
        <div className="flex-[1_1_300px] min-w-0 flex flex-col gap-6">
          {phase !== 'lobby' && phase !== 'cancelled' && <ClaimCard l={l} k={k} mine={mine} act={act} />}
          <CreatorCard l={l} k={k} creatorFees={creatorFees} act={act} />
        </div>
        <Card className="flex-[1_1_300px] min-w-0 flex flex-col gap-2">
          <CardTitle>Launch</CardTitle>
          <Row label="Mint"><span className="font-terminal text-xl" title={l.mint.toBase58()}>{short(l.mint)}</span></Row>
          <Row label="Creator"><span className="font-terminal text-xl" title={l.creator.toBase58()}>{short(l.creator)}</span></Row>
          <Row label="Lobby total"><span className="font-terminal text-xl">{sol(l.totalDeposits, 3)} SOL · {l.depositors} wallets</span></Row>
          <Row label="Lobby tokens"><span className="font-terminal text-xl">{tokens(l.lobbyTokens, 0)}</span></Row>
          <Row label="Mint and freeze authority">None</Row>
        </Card>
      </div>
    </main>
  );
}

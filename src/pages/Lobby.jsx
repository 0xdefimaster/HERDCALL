import React, { useEffect, useState } from 'react';
import { Navigate, useParams } from 'react-router-dom';
import { Bar, Card, CardTitle, PixelButton, Row, Tag, TokenAvatar } from '../components/ui';
import { MOCK_BOARD, MOCK_LOBBY, findToken } from '../data/mock';
import { useWallet } from '../wallet/WalletContext';

const QUICK_AMOUNTS = ['0.1', '0.5', '1', '5'];

export function formatClock(secs) {
  return `${String(Math.floor(secs / 60)).padStart(2, '0')}:${String(secs % 60).padStart(2, '0')}`;
}

export default function LobbyRoute() {
  const { symbol } = useParams();
  const token = symbol ? findToken(symbol) : MOCK_BOARD.find((t) => t.status === 'lobby');
  if (!token) return <Navigate to="/" replace />;
  if (token.status !== 'lobby') return <Navigate to={`/token/${token.symbol}`} replace />;
  return <Lobby key={token.symbol} token={token} />;
}

function Lobby({ token }) {
  const { publicKey, connect } = useWallet();
  const [secs, setSecs] = useState(token.lobbySecs);
  const [amount, setAmount] = useState('0.5');
  const [joined, setJoined] = useState(false);
  const [herd, setHerd] = useState(token.herd);
  const [pen, setPen] = useState(token.pen);

  useEffect(() => {
    if (secs === 0) return undefined;
    const timer = setTimeout(() => setSecs((s) => s - 1), 1000);
    return () => clearTimeout(timer);
  }, [secs]);

  const cleared = secs === 0;
  const progress = Math.round(((MOCK_LOBBY.windowSecs - secs) / MOCK_LOBBY.windowSecs) * 100);

  const join = () => {
    const sol = parseFloat(amount);
    if (!(sol > 0)) return;
    setJoined(true);
    setHerd((h) => h + 1);
    setPen((p) => p + sol);
  };

  return (
    <main className="max-w-[1240px] mx-auto px-6 pt-8 pb-12 flex flex-wrap gap-6 items-stretch">
      <Card aria-label="Token" className="flex-[1_1_260px] min-w-0 flex flex-col gap-4">
        <div className="flex items-center gap-3.5">
          <TokenAvatar initial={token.initial} />
          <div>
            <h1 className="m-0 font-pixel font-bold text-2xl leading-tight">{token.name}</h1>
            <div className="font-terminal text-xl text-hc-muted">${token.symbol}</div>
          </div>
        </div>
        <p className="m-0 text-hc-body">{token.description}</p>
        <div className="border-t-[3px] border-dashed border-hc-line pt-4 flex flex-col gap-2.5 text-base">
          {token.bonded ? (
            <>
              <Row label="Creator"><Tag>Bellwether</Tag></Row>
              <Row label="Bond"><span className="font-terminal text-[22px]">{token.bondSol}.0 SOL</span></Row>
              <Row label="Creator bag vests"><span className="font-terminal text-[22px]">{token.vestDays} days</span></Row>
              <Row label="Fee escrow">50% to breakout</Row>
            </>
          ) : (
            <>
              <Row label="Creator"><Tag tone="outline">Unbonded</Tag></Row>
              <Row label="Bond">None</Row>
            </>
          )}
          <Row label="History">{token.launches} launches · {token.trampled} trampled</Row>
        </div>
      </Card>

      <Card aria-label="Bell countdown" highlight className="flex-[2_1_420px] min-w-0 p-8 flex flex-col items-center text-center gap-4">
        {cleared ? (
          <>
            <div className="font-pixel text-[15px] text-hc-muted">The bell has rung</div>
            <div className="font-pixel font-bold uppercase leading-[1.05] text-hc-orange text-[clamp(36px,5vw,60px)]">The herd is in.</div>
            <p className="m-0 text-hc-body">
              Everyone entered at the same clearing price. Curve trading is open.
            </p>
          </>
        ) : (
          <>
            <div className="font-pixel text-[15px] text-hc-muted">The bell rings in</div>
            <div aria-live="polite" className="font-terminal leading-[0.85] text-hc-orange text-[clamp(110px,15vw,190px)]">{formatClock(secs)}</div>
          </>
        )}
        <Bar value={progress} className="w-full h-5" />
        <div className="w-full grid grid-cols-3 gap-3">
          <div className="bg-hc-bg border-[3px] border-hc-line p-3"><div className="text-sm text-hc-muted">Herd gathered</div><div className="font-terminal text-[34px] leading-none">{herd.toLocaleString('en-US')}</div></div>
          <div className="bg-hc-bg border-[3px] border-hc-line p-3"><div className="text-sm text-hc-muted">SOL in the pen</div><div className="font-terminal text-[34px] leading-none">{pen.toFixed(1)}</div></div>
          <div className="bg-hc-bg border-[3px] border-hc-line p-3"><div className="text-sm text-hc-muted">Snipers w/ edge</div><div className="font-terminal text-[34px] leading-none text-hc-navy-text">0</div></div>
        </div>
        <div className="w-full text-left p-3.5 border-[3px] border-dashed border-hc-line text-hc-body text-base">
          Orders in the Lobby are batched. When the bell rings, the whole pen clears at <strong className="text-hc-cream">one price</strong>. Being first in the block gives no edge.
        </div>
      </Card>

      <Card aria-label="Join the herd" className="flex-[1_1_300px] min-w-0 flex flex-col gap-4">
        <CardTitle className="text-[22px]">Join the herd</CardTitle>
        <div className="flex flex-col gap-2">
          <label htmlFor="amt" className="text-[15px] text-hc-muted">Amount (SOL)</label>
          <input
            id="amt"
            type="text"
            inputMode="decimal"
            value={amount}
            disabled={joined}
            onChange={(e) => setAmount(e.target.value)}
            className="min-h-[54px] px-3.5 border-[3px] border-hc-cream bg-hc-bg text-hc-cream font-terminal text-[32px]"
          />
        </div>
        <div className="grid grid-cols-4 gap-2">
          {QUICK_AMOUNTS.map((v) => (
            <button
              key={v}
              type="button"
              disabled={joined}
              onClick={() => setAmount(v)}
              className="min-h-[44px] border-[3px] border-hc-line bg-hc-bg text-hc-cream font-terminal text-2xl cursor-pointer disabled:opacity-50"
            >
              {v}
            </button>
          ))}
        </div>
        {joined ? (
          <div className="min-h-[58px] bg-hc-navy text-hc-navy-text font-pixel text-sm flex items-center justify-center text-center">
            In the pen · {amount} SOL
          </div>
        ) : !publicKey ? (
          <PixelButton onClick={connect} className="min-h-[58px] text-[17px]">Connect wallet to join</PixelButton>
        ) : (
          <PixelButton onClick={join} disabled={cleared} className="min-h-[58px] text-[17px]">Answer the call</PixelButton>
        )}
        <p className="m-0 text-[15px] text-hc-muted">
          You pay the clearing price, same as everyone. Anything above the launch cap is refunded. Preview only — no transaction is sent yet.
        </p>
        <div className="border-t-[3px] border-dashed border-hc-line pt-3.5">
          <h3 className="m-0 mb-2.5 font-pixel text-[13px] font-normal text-hc-muted">Recent joins</h3>
          <ul className="list-none m-0 p-0 flex flex-col gap-1.5 font-terminal text-[22px]">
            {MOCK_LOBBY.joins.map((j) => (
              <li key={j.who} className="flex justify-between gap-2"><span className="text-hc-body">{j.who}</span><span>{j.sol} SOL</span></li>
            ))}
          </ul>
        </div>
      </Card>
    </main>
  );
}

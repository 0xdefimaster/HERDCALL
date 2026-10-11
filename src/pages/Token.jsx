import React from 'react';
import { Link, Navigate, useParams } from 'react-router-dom';
import { Bar, Card, CardTitle, PixelButton, Row, Tag, TokenAvatar } from '../components/ui';
import { MOCK_HOLDERS, MOCK_SIGNALS, findToken, formatAge, formatUsd } from '../data/mock';
import { FEE_SPLIT } from '../lib/fees';

const STATUS_LABEL = {
  curve: (t) => `Bell rang ${formatAge(t.ageMin)} ago`,
  broke: () => 'Broke out',
  takeover: () => 'Community vote open',
};

function signalsFor(t) {
  if (t.bonded) return MOCK_SIGNALS;
  return [{ label: 'Unbonded creator · no bond, no vest', value: 'No', bar: 20, warn: true }, ...MOCK_SIGNALS.slice(1)];
}

const TAG_COLORS = {
  muted: 'text-hc-muted',
  trust: 'text-hc-navy-text',
  body: 'text-hc-body',
  warn: 'text-hc-warn',
};

const CHART_POINTS =
  '0,200 40,200 40,184 80,184 80,190 120,190 120,166 160,166 160,152 200,152 200,158 240,158 240,130 280,130 280,136 320,136 320,112 360,112 360,118 400,118 400,94 440,94 440,104 480,104 480,82 520,82 520,88 560,88 560,68 600,68 600,74 640,74 640,58 700,58';

export default function Token() {
  const { symbol } = useParams();
  const t = findToken(symbol ?? 'MOOVE');
  if (!t) return <Navigate to="/" replace />;
  if (t.status === 'lobby') return <Navigate to={`/lobby/${t.symbol}`} replace />;
  const fees = t.bonded ? FEE_SPLIT.bonded : FEE_SPLIT.unbonded;
  const takeoverOpen = t.status === 'takeover';
  return (
    <main className="max-w-[1240px] mx-auto px-6 pt-7 pb-14 flex flex-col gap-6">
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-3.5">
          <TokenAvatar initial={t.initial} size="w-[60px] h-[60px] text-[26px]" />
          <div>
            <h1 className="m-0 font-pixel font-bold text-[28px] leading-tight">
              {t.name} <span className="font-terminal font-normal text-2xl text-hc-muted">${t.symbol}</span>
            </h1>
            <div className="flex flex-wrap gap-2 mt-1.5">
              {t.bonded ? <Tag>Bellwether</Tag> : <Tag tone="outline">Unbonded</Tag>}
              <Tag tone="outline">{STATUS_LABEL[t.status](t)}</Tag>
            </div>
          </div>
        </div>
        <div className="flex flex-wrap gap-7">
          {[
            ['Price', t.price],
            ['Market cap', formatUsd(t.marketCapUsd)],
            ['Herd', t.herd],
          ].map(([label, value]) => (
            <div key={label}>
              <div className="text-sm text-hc-muted">{label}</div>
              <div className="font-terminal text-[28px] leading-none">{value}</div>
            </div>
          ))}
        </div>
      </div>

      <div className="flex flex-wrap gap-6 items-start">
        <div className="flex-[999_1_560px] min-w-0 flex flex-col gap-6">
          <Card aria-label="Price chart">
            <div className="flex flex-wrap justify-between gap-2 text-[15px] text-hc-muted">
              <span>Price · 1m</span>
              <span>Dashed line = launch clearing price</span>
            </div>
            <svg viewBox="0 0 700 260" width="100%" height="260" role="img" aria-label="Price chart rising from the launch clearing price" className="block mt-3" shapeRendering="crispEdges">
              <line x1="0" y1="200" x2="700" y2="200" stroke="#AFC2EE" strokeWidth="3" strokeDasharray="8 8" />
              <polyline fill="none" stroke="#FF7A1A" strokeWidth="5" points={CHART_POINTS} />
              <text x="8" y="190" fill="#AFC2EE" fontFamily="VT323, monospace" fontSize="20">clear {t.clearingPrice.replace(' SOL', '')}</text>
            </svg>
          </Card>

          <Card aria-label="Breakout progress">
            <div className="flex flex-wrap justify-between gap-2">
              <span className="font-pixel font-bold text-[15px]">Breakout progress</span>
              <span className="font-terminal text-[22px] text-hc-body">{t.breakoutProgress}% · [BREAKOUT THRESHOLD] SOL</span>
            </div>
            <Bar value={t.breakoutProgress} className="mt-3 h-[22px]" />
            <p className="mt-2.5 mb-0 text-[15px] text-hc-muted">At 100%: liquidity migrates and locks, mint and freeze authority are revoked, creator escrow unlocks.</p>
          </Card>

          <Card aria-label="The herd">
            <CardTitle className="text-xl">The herd</CardTitle>
            <div className="mt-3 overflow-x-auto">
              <table className="w-full min-w-[480px] border-collapse text-base">
                <thead>
                  <tr>
                    <th scope="col" className="text-left p-2 border-b-[3px] border-hc-line text-hc-muted font-normal">Holder</th>
                    <th scope="col" className="text-right p-2 border-b-[3px] border-hc-line text-hc-muted font-normal">Share</th>
                    <th scope="col" className="text-left p-2 border-b-[3px] border-hc-line text-hc-muted font-normal">Tag</th>
                  </tr>
                </thead>
                <tbody>
                  {MOCK_HOLDERS.map((h) => (
                    <tr key={h.who}>
                      <td className="p-2 border-b-2 border-dashed border-hc-line font-terminal text-[22px]">{h.who}</td>
                      <td className="p-2 border-b-2 border-dashed border-hc-line text-right font-terminal text-[22px]">{h.pct}</td>
                      <td className={`p-2 border-b-2 border-dashed border-hc-line font-pixel text-xs ${TAG_COLORS[h.kind]}`}>{h.tag}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </Card>
        </div>

        <div className="flex-[1_1_340px] min-w-0 flex flex-col gap-6">
          <Card aria-label="Trade" highlight>
            <div className="grid grid-cols-2 border-[3px] border-hc-line">
              <button type="button" className="min-h-[44px] bg-hc-orange text-hc-bg font-pixel font-bold text-[15px] cursor-pointer">Buy</button>
              <button type="button" className="min-h-[44px] bg-hc-bg text-hc-body font-pixel text-[15px] cursor-pointer">Sell</button>
            </div>
            <label htmlFor="buy" className="block mt-4 text-[15px] text-hc-muted">Amount (SOL)</label>
            <input id="buy" type="text" inputMode="decimal" defaultValue="0.5" className="mt-1.5 w-full box-border min-h-[54px] px-3.5 border-[3px] border-hc-cream bg-hc-bg text-hc-cream font-terminal text-[30px]" />
            <div className="mt-2.5 flex justify-between gap-2 text-[15px] text-hc-muted">
              <span>Fee 1% · {fees.rektBack}% to Rekt-back</span>
              <span className="font-terminal text-xl">≈ 78,400 {t.symbol}</span>
            </div>
            <PixelButton className="mt-4 w-full min-h-[56px] text-base">Buy ${t.symbol}</PixelButton>
          </Card>

          <Card aria-label="Fair Score">
            <div className="flex items-center justify-between gap-3">
              <CardTitle className="text-xl">Fair Score</CardTitle>
              <div className="font-terminal text-[56px] leading-[0.9] text-hc-navy-text">
                {t.fairScore}
                <span className="text-2xl text-hc-muted">/100</span>
              </div>
            </div>
            <ul className="list-none mt-4 mb-0 p-0 flex flex-col gap-3.5">
              {signalsFor(t).map((s) => (
                <li key={s.label}>
                  <div className="flex justify-between gap-2 text-[15px]">
                    <span>{s.label}</span>
                    <span className={s.warn ? 'text-hc-warn' : 'text-hc-navy-text'}>{s.value}</span>
                  </div>
                  <Bar value={s.bar} color={s.warn ? 'bg-hc-warn' : 'bg-hc-navy-text'} className="mt-1.5 h-2.5 border-2" />
                </li>
              ))}
            </ul>
            <p className="mt-4 mb-0 text-sm text-hc-muted">A signal, not a gate. Low-score tokens stay tradeable — they just say so.</p>
          </Card>

          <Card aria-label="Community Takeover status">
            <div className="flex items-center justify-between gap-3">
              <CardTitle className="text-xl">Takeover</CardTitle>
              {takeoverOpen ? <Tag tone="orange">Vote open</Tag> : <Tag tone="outline">Not eligible</Tag>}
            </div>
            <div className="mt-3.5 flex flex-col gap-2 text-base">
              <Row label="Creator trampled">{t.trampled > 0 ? 'Yes' : 'No'}</Row>
              <Row label="Last creator activity">{takeoverOpen ? '19d ago' : '2h ago'}</Row>
              <Row label="Inactivity trigger"><span className="font-terminal text-xl">[N] days</span></Row>
            </div>
            <p className="mt-3.5 mb-0 text-sm text-hc-muted">
              {takeoverOpen ? (
                <>The herd is voting to take over creator rights. <Link to="/takeover">Cast your vote</Link></>
              ) : (
                <>The creator is active, so the herd can't open a takeover. <Link to="/takeover">See a live takeover</Link></>
              )}
            </p>
          </Card>
        </div>
      </div>
    </main>
  );
}

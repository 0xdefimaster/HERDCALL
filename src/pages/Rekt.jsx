import React from 'react';
import { Bar, Card, CardTitle, PixelButton, Tag } from '../components/ui';
import { MOCK_BADGES, MOCK_CLAIMS, MOCK_LUCKY_HOOF, MOCK_POOLS, XP_RULES, formatUsd } from '../data/mock';
import { LUCKY_HOOF } from '../lib/fees';

const RANKS = ['Calf', 'Yearling', 'Bull', 'Bellwether'];
const CURRENT_RANK = 'Yearling';

export default function Rekt() {
  return (
    <main className="max-w-[1240px] mx-auto px-6 pt-8 pb-14 flex flex-col gap-6">
      <Card aria-label="Herd rank" highlight className="p-6 flex flex-wrap gap-7 items-center">
        <img
          src="/herdcall-hero.webp"
          alt="HerdCall mascot"
          className="shrink-0 w-[120px] h-[120px] object-cover object-[50%_28%] border-[3px] border-hc-line [image-rendering:pixelated]"
        />
        <div className="flex-[1_1_380px] min-w-0">
          <div className="font-pixel text-[13px] text-hc-muted">Season 1 · Herd rank</div>
          <h1 className="mt-1.5 mb-0 font-pixel font-bold text-[44px] leading-none uppercase">{CURRENT_RANK}</h1>
          <div className="mt-4 flex justify-between gap-2 font-terminal text-xl text-hc-muted">
            <span>2,340 XP</span>
            <span>Bull at 4,000 XP</span>
          </div>
          <Bar value={58} className="mt-1.5 h-5" />
          <ol className="list-none mt-3.5 mb-0 p-0 flex flex-wrap gap-2 font-pixel text-xs">
            {RANKS.map((rank) => {
              const index = RANKS.indexOf(rank);
              const current = RANKS.indexOf(CURRENT_RANK);
              const cls =
                index === current
                  ? 'bg-hc-orange text-hc-bg font-bold'
                  : index < current
                    ? 'bg-hc-line text-hc-body'
                    : 'border-2 border-hc-line text-hc-muted';
              return (
                <li key={rank} className={`px-2.5 py-1 ${cls}`}>{rank}</li>
              );
            })}
          </ol>
        </div>
      </Card>

      <Card aria-label="Lucky Hoof" highlight className="flex flex-col gap-5">
        <div className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0 max-w-[620px]">
            <div className="flex flex-wrap items-center gap-2.5">
              <CardTitle className="text-2xl">Lucky Hoof</CardTitle>
              <Tag tone="orange">1 in {LUCKY_HOOF.odds}</Tag>
            </div>
            <p className="mt-2 mb-0 text-hc-body">
              Held a bag for {LUCKY_HOOF.minHoldDays}+ days and sold at a loss, or held it all the way to zero? That's a ticket. Every week one in every{' '}
              {LUCKY_HOOF.odds} tickets gets its full loss refunded, up to {LUCKY_HOOF.maxPayoutSol} SOL.
            </p>
          </div>
          <div className="text-right">
            <div className="text-sm text-hc-muted">Next draw in</div>
            <div className="font-terminal text-5xl leading-[0.9] text-hc-orange">{MOCK_LUCKY_HOOF.drawIn}</div>
          </div>
        </div>
        <div className="grid grid-cols-[repeat(auto-fit,minmax(150px,1fr))] gap-3">
          {[
            ['Tickets this week', MOCK_LUCKY_HOOF.ticketsThisWeek.toLocaleString('en-US')],
            ['Winners this week', Math.floor(MOCK_LUCKY_HOOF.ticketsThisWeek / LUCKY_HOOF.odds)],
            ['Fund balance', formatUsd(MOCK_POOLS.luckyHoofUsd)],
            ['Odds per ticket', `${(100 / LUCKY_HOOF.odds).toFixed(0)}%`],
          ].map(([label, value]) => (
            <div key={label} className="bg-hc-bg border-[3px] border-hc-line p-3">
              <div className="text-sm text-hc-muted">{label}</div>
              <div className="font-terminal text-[32px] leading-none">{value}</div>
            </div>
          ))}
        </div>
        <div className="flex flex-wrap gap-6">
          <div className="flex-[1_1_320px] min-w-0">
            <h3 className="m-0 font-pixel font-bold text-sm uppercase">Your tickets</h3>
            <ul className="list-none mt-2.5 mb-0 p-0 flex flex-col gap-2">
              {MOCK_LUCKY_HOOF.yourTickets.map((t) => (
                <li key={t.token} className="flex justify-between gap-3 pb-2 border-b-2 border-dashed border-hc-line">
                  <span>
                    <span className="font-terminal text-[22px]">{t.token}</span>{' '}
                    <span className="text-sm text-hc-muted">{t.how} · held {t.heldDays}d</span>
                  </span>
                  <span className="font-terminal text-[22px] text-hc-down whitespace-nowrap">-{t.lossSol} SOL</span>
                </li>
              ))}
            </ul>
          </div>
          <div className="flex-[1_1_280px] min-w-0">
            <h3 className="m-0 font-pixel font-bold text-sm uppercase">Last week's Lucky Hooves</h3>
            <ul className="list-none mt-2.5 mb-0 p-0 flex flex-col gap-2">
              {MOCK_LUCKY_HOOF.lastWinners.map((w) => (
                <li key={w.who} className="flex justify-between gap-3 pb-2 border-b-2 border-dashed border-hc-line">
                  <span className="font-terminal text-[22px]">{w.who} <span className="text-hc-muted">{w.token}</span></span>
                  <span className="font-terminal text-[22px] text-hc-up whitespace-nowrap">+{w.sol} SOL</span>
                </li>
              ))}
            </ul>
          </div>
        </div>
        <p className="m-0 text-sm text-hc-muted">
          Winners are picked with on-chain randomness (VRF) that anyone can verify. One ticket per wallet, per token, per week; minimum loss {LUCKY_HOOF.minLossSol} SOL.
          Tokens with no trades for {LUCKY_HOOF.deadTokenDays} days count as held to zero. Refunds are capped by the fund: not insurance, not a guarantee.
        </p>
      </Card>

      <div className="flex flex-wrap gap-6 items-start">
        <Card aria-label="Rekt-back" className="flex-[999_1_560px] min-w-0">
          <div className="flex flex-wrap items-end justify-between gap-4">
            <div>
              <CardTitle className="text-2xl leading-tight">Back on your hooves</CardTitle>
              <p className="mt-1.5 mb-0 text-hc-muted text-base">Capped, pool-funded compensation when a creator breaks the rules.</p>
            </div>
            <div className="text-right">
              <div className="text-sm text-hc-muted">Claimable</div>
              <div className="font-terminal text-5xl leading-[0.9] text-hc-navy-text">0.84 SOL</div>
            </div>
          </div>
          <div className="mt-4 overflow-x-auto">
            <table className="w-full min-w-[520px] border-collapse text-base">
              <thead>
                <tr>
                  <th scope="col" className="text-left p-2 border-b-[3px] border-hc-line text-hc-muted font-normal">Token</th>
                  <th scope="col" className="text-left p-2 border-b-[3px] border-hc-line text-hc-muted font-normal">What happened</th>
                  <th scope="col" className="text-right p-2 border-b-[3px] border-hc-line text-hc-muted font-normal">Your share</th>
                </tr>
              </thead>
              <tbody>
                {MOCK_CLAIMS.map((c) => (
                  <tr key={c.token}>
                    <td className="px-2 py-2.5 border-b-2 border-dashed border-hc-line font-terminal text-[22px]">{c.token}</td>
                    <td className="px-2 py-2.5 border-b-2 border-dashed border-hc-line text-hc-body">{c.what}</td>
                    <td className="px-2 py-2.5 border-b-2 border-dashed border-hc-line text-right font-terminal text-[22px]">{c.sol}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <PixelButton className="mt-5 min-h-[56px] text-base px-6">Claim 0.84 SOL</PixelButton>
          <p className="mt-3.5 mb-0 text-sm text-hc-muted">Payouts are capped by the pool balance. Not insurance, not a guarantee. Every payout is public on-chain.</p>
        </Card>

        <div className="flex-[1_1_340px] min-w-0 flex flex-col gap-6">
          <Card aria-label="How XP works">
            <CardTitle>How you earn XP</CardTitle>
            <ul className="list-none mt-3.5 mb-0 p-0 flex flex-col gap-2.5">
              {XP_RULES.map((x) => (
                <li key={x.what} className="flex justify-between gap-3">
                  <span>{x.what}</span>
                  <span className="font-terminal text-2xl leading-none text-hc-orange whitespace-nowrap">{x.pts}</span>
                </li>
              ))}
            </ul>
            <p className="mt-3.5 mb-0 pt-3 border-t-[3px] border-dashed border-hc-line text-[15px] text-hc-muted">Volume earns nothing. Wash trading earns nothing.</p>
          </Card>

          <Card aria-label="Badges">
            <CardTitle>Badges</CardTitle>
            <div className="mt-3.5 flex flex-wrap gap-2.5">
              {MOCK_BADGES.map((b) => (
                <span
                  key={b.name}
                  className={`px-2.5 py-1.5 border-[3px] font-pixel text-xs ${b.earned ? 'border-hc-orange text-hc-orange' : 'border-hc-line text-hc-muted'}`}
                >
                  {b.name}
                </span>
              ))}
            </div>
          </Card>
        </div>
      </div>
    </main>
  );
}

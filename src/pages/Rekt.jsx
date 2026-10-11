import React from 'react';
import { Bar, Card, CardTitle, PixelButton } from '../components/ui';
import { MOCK_BADGES, MOCK_CLAIMS, XP_RULES } from '../data/mock';

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

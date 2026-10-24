import React, { useEffect, useMemo, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { ChartIcon, CycleIcon, HLogo } from '../components/icons';
import { Bar, Card, CardTitle, Change, PixelButton, Sparkline, Tag, TokenAvatar } from '../components/ui';
import { MOCK_BOARD, MOCK_POOLS, formatAge, formatUsd } from '../data/mock';
import { DIAMOND_HOOVES, diamondHoovesBudget, feeBreakdown } from '../lib/fees';
import { formatClock } from './Lobby';
import LiveLaunches from '../components/LiveLaunches';

const TABS = [
  { key: 'all', label: 'All' },
  { key: 'new', label: 'New' },
  { key: 'lobby', label: 'Lobby open' },
  { key: 'trending', label: 'Trending' },
  { key: 'near', label: 'Near breakout' },
  { key: 'broke', label: 'Broke out' },
  { key: 'takeover', label: 'Takeovers' },
];

const NEAR_BREAKOUT = 80;

const TAB_FILTERS = {
  all: () => true,
  new: (t) => t.status === 'lobby' || t.ageMin <= 15,
  lobby: (t) => t.status === 'lobby',
  trending: (t) => t.change24h > 0,
  near: (t) => t.status === 'curve' && t.breakoutProgress >= NEAR_BREAKOUT,
  broke: (t) => t.status === 'broke',
  takeover: (t) => t.status === 'takeover',
};

const TAB_SORT = {
  trending: (a, b) => b.change24h - a.change24h,
};
const DEFAULT_SORT = (a, b) => a.ageMin - b.ageMin;

const QUICK_FILTERS = [
  { tab: 'new', title: 'New launches', note: 'Lobbies and tokens under 15 minutes old' },
  { tab: 'trending', title: 'Trending', note: 'Biggest 24h gainers' },
  { tab: 'near', title: 'Near breakout', note: `Curve ${NEAR_BREAKOUT}%+ filled` },
  { tab: 'takeover', title: 'Takeovers', note: 'Herd votes on abandoned tokens' },
];

function tokenHref(t) {
  if (t.status === 'lobby') return `/lobby/${t.symbol}`;
  if (t.status === 'takeover') return '/takeover';
  return `/token/${t.symbol}`;
}

function FairChip({ score }) {
  const tone = score >= 75 ? 'text-hc-navy-text border-hc-navy-text' : score >= 50 ? 'text-hc-body border-hc-strong' : 'text-hc-warn border-hc-warn';
  return <span className={`inline-block px-1.5 border-2 font-terminal text-lg leading-tight whitespace-nowrap ${tone}`}>{score}</span>;
}

function StageCell({ t, secsLeft }) {
  if (t.status === 'lobby') {
    return <Tag tone="orange">{secsLeft > 0 ? `Bell in ${formatClock(secsLeft)}` : 'Bell rang'}</Tag>;
  }
  if (t.status === 'takeover') return <Tag tone="outline">Takeover vote</Tag>;
  if (t.status === 'broke') return <Tag tone="outline">Broke out</Tag>;
  return (
    <div className="w-[100px]">
      <div className="font-terminal text-lg leading-none">{t.breakoutProgress}%</div>
      <Bar value={t.breakoutProgress} className="mt-1 h-3 border-2" />
    </div>
  );
}

function TokenRow({ t, secsLeft }) {
  const lobby = t.status === 'lobby';
  const cell = 'px-2.5 py-3 border-b-2 border-dashed border-hc-line align-middle';
  return (
    <tr className="hover:bg-hc-surface">
      <td className={cell}>
        <Link to={tokenHref(t)} className="flex items-center gap-3 no-underline text-hc-cream hover:text-hc-orange">
          <TokenAvatar initial={t.initial} size="w-11 h-11 text-xl" />
          <span className="min-w-0">
            <span className="block font-pixel font-bold text-sm whitespace-nowrap">{t.name}</span>
            <span className="flex items-center gap-2 font-terminal text-lg text-hc-muted leading-tight whitespace-nowrap">
              ${t.symbol} · {lobby ? 'now' : `${formatAge(t.ageMin)} ago`}
              {t.bonded && <span className="font-pixel text-[10px] px-1 bg-hc-navy text-hc-navy-text">BELL</span>}
            </span>
          </span>
        </Link>
      </td>
      <td className={`${cell} font-terminal text-xl whitespace-nowrap`}>{lobby ? <span title="SOL waiting in the Lobby">{t.pen} SOL</span> : formatUsd(t.marketCapUsd)}</td>
      <td className={`${cell} font-terminal text-xl`}>{lobby ? '—' : formatUsd(t.vol24hUsd)}</td>
      <td className={cell}>
        {lobby ? (
          <span className="text-hc-muted">—</span>
        ) : (
          <div className="flex items-center gap-2">
            <Change value={t.change24h} />
            <Sparkline points={t.spark} up={t.change24h >= 0} width={60} height={24} />
          </div>
        )}
      </td>
      <td className={cell}><FairChip score={t.fairScore} /></td>
      <td className={cell}><StageCell t={t} secsLeft={secsLeft} /></td>
      <td className={`${cell} text-right`}>
        <PixelButton to={tokenHref(t)} className="min-h-[36px] px-3 text-xs">
          {lobby ? 'Join' : t.status === 'takeover' ? 'Vote' : 'Buy'}
        </PixelButton>
      </td>
    </tr>
  );
}

function RevenuePanel() {
  const fees = feeBreakdown(MOCK_BOARD);
  const rows = [
    { label: 'Creators', usd: fees.creator, color: 'bg-hc-orange' },
    { label: 'Rekt-back pool', usd: fees.rektBack, color: 'bg-hc-navy-text' },
    { label: 'Interfaces & referrals', usd: fees.interface, color: 'bg-hc-cream' },
    { label: 'Protocol', usd: fees.protocol, color: 'bg-hc-strong' },
  ];
  return (
    <Card highlight aria-label="Protocol revenue">
      <div className="flex items-baseline justify-between gap-2">
        <CardTitle className="text-base">Protocol revenue</CardTitle>
        <span className="font-pixel text-[11px] text-hc-muted">24h</span>
      </div>
      <div className="mt-2 font-terminal text-[44px] leading-none text-hc-orange">{formatUsd(Math.round(fees.total))}</div>
      <div className="text-sm text-hc-muted">fees collected · 1% of {formatUsd(MOCK_BOARD.reduce((s, t) => s + t.vol24hUsd, 0))} volume</div>
      <div className="mt-3 flex h-4 border-2 border-hc-bg">
        {rows.map((r) => (
          <div key={r.label} className={r.color} style={{ flex: r.usd }} />
        ))}
      </div>
      <ul className="list-none mt-3 mb-0 p-0 flex flex-col gap-1.5 text-[15px]">
        {rows.map((r) => (
          <li key={r.label} className="flex items-center justify-between gap-2">
            <span className="inline-flex items-center gap-2"><span className={`w-2.5 h-2.5 ${r.color}`} />{r.label}</span>
            <span className="font-terminal text-xl leading-none">{formatUsd(Math.round(r.usd))}</span>
          </li>
        ))}
      </ul>
      <div className="mt-3 pt-3 border-t-[3px] border-dashed border-hc-line text-[14px] text-hc-muted">Protocol share, after the ops floor:</div>
      <ul className="list-none mt-1.5 mb-0 p-0 flex flex-col gap-1 text-[14px]">
        {fees.protocolParts.map((p) => (
          <li key={p.key} className="flex justify-between gap-2">
            <span>{p.pct}% {p.title}</span>
            <span className="font-terminal text-lg leading-none">{formatUsd(Math.round(p.usd))}</span>
          </li>
        ))}
      </ul>
      <div className="mt-3 pt-3 border-t-[3px] border-dashed border-hc-line grid grid-cols-2 gap-2">
        <div>
          <div className="text-[13px] text-hc-muted">Rekt-back pool</div>
          <div className="font-terminal text-2xl leading-none">{formatUsd(MOCK_POOLS.rektBackBalanceUsd)}</div>
        </div>
        <div>
          <div className="text-[13px] text-hc-muted">Paid to holders</div>
          <div className="font-terminal text-2xl leading-none">{formatUsd(MOCK_POOLS.rektBackPaidUsd)}</div>
        </div>
        <Link to="/rekt-back" className="col-span-2 no-underline text-hc-cream hover:text-hc-orange">
          <div className="text-[13px] text-hc-muted">Diamond Hooves budget · 1/{DIAMOND_HOOVES.revenueDivisor} of protocol revenue</div>
          <div className="font-terminal text-2xl leading-none">{formatUsd(Math.round(diamondHoovesBudget(fees.protocol)))}</div>
        </Link>
      </div>
      <p className="mt-3 mb-0 text-[13px] text-hc-muted">Example figures. Live numbers will be read straight from the program's fee vaults.</p>
    </Card>
  );
}

function MiniList({ title, items, render }) {
  return (
    <Card aria-label={title}>
      <CardTitle className="text-base">{title}</CardTitle>
      <ul className="list-none mt-3 mb-0 p-0 flex flex-col gap-2">
        {items.map((t) => (
          <li key={t.symbol}>
            <Link to={tokenHref(t)} className="flex items-center justify-between gap-2 no-underline text-hc-cream hover:text-hc-orange">
              <span className="flex items-center gap-2 min-w-0">
                <TokenAvatar initial={t.initial} size="w-8 h-8 text-sm border-2" />
                <span className="font-terminal text-xl truncate">${t.symbol}</span>
              </span>
              {render(t)}
            </Link>
          </li>
        ))}
      </ul>
    </Card>
  );
}

export default function Board() {
  const [elapsed, setElapsed] = useState(0);
  const [tab, setTab] = useState('all');
  const [bellwetherOnly, setBellwetherOnly] = useState(false);
  const [params] = useSearchParams();
  const query = (params.get('q') ?? '').trim().toLowerCase();

  useEffect(() => {
    const timer = setInterval(() => setElapsed((e) => e + 1), 1000);
    return () => clearInterval(timer);
  }, []);

  const secsLeft = (t) => Math.max(0, t.lobbySecs - elapsed);
  const nextBell = [...MOCK_BOARD].filter((t) => t.status === 'lobby').sort((a, b) => a.lobbySecs - b.lobbySecs)[0];

  const tokens = useMemo(
    () =>
      MOCK_BOARD.filter(TAB_FILTERS[tab])
        .filter((t) => !bellwetherOnly || t.bonded)
        .filter((t) => !query || t.name.toLowerCase().includes(query) || t.symbol.toLowerCase().includes(query))
        .sort(TAB_SORT[tab] ?? DEFAULT_SORT),
    [tab, bellwetherOnly, query],
  );

  const trending = [...MOCK_BOARD].filter((t) => t.change24h > 0).sort((a, b) => b.change24h - a.change24h).slice(0, 5);
  const topVolume = [...MOCK_BOARD].sort((a, b) => b.vol24hUsd - a.vol24hUsd).slice(0, 5);

  return (
    <main className="px-4 lg:px-6 py-6 flex flex-col xl:flex-row gap-6">
      <div className="flex-1 min-w-0 flex flex-col gap-6">
        <LiveLaunches />
        <section className="bg-hc-surface border-[3px] border-hc-line p-6 flex flex-wrap items-center justify-between gap-6">
          <div className="flex-[1_1_360px] min-w-0">
            <div className="font-pixel text-xs text-hc-navy-text">Solana launchpad</div>
            <h1 className="mt-3 mb-0 font-pixel font-bold uppercase leading-[1.05] text-[clamp(28px,3.6vw,48px)]">
              Get in before
              <br />
              <span className="text-hc-orange">the bell.</span>
            </h1>
            <p className="mt-3 mb-0 max-w-[520px] text-hc-body">
              Every launch opens with a countdown. When the bell rings the whole herd enters at one price, so bots camping the first block get nothing.
            </p>
          </div>
          {nextBell && (
            <Link
              to={`/lobby/${nextBell.symbol}`}
              className="flex-[0_1_300px] min-w-0 block no-underline text-hc-cream bg-hc-bg border-[3px] border-hc-orange shadow-px-navy p-4 hover:text-hc-cream"
            >
              <div className="font-pixel text-xs text-hc-muted">Next bell</div>
              <div className="mt-2 flex items-center justify-between gap-3">
                <span className="flex items-center gap-2 min-w-0">
                  <TokenAvatar initial={nextBell.initial} size="w-10 h-10 text-lg" />
                  <span className="font-pixel font-bold text-sm">{nextBell.name}</span>
                </span>
                <span className="font-terminal text-[52px] leading-none text-hc-orange">
                  {secsLeft(nextBell) > 0 ? formatClock(secsLeft(nextBell)) : 'IN'}
                </span>
              </div>
              <div className="mt-2 text-sm text-hc-muted">{nextBell.herd} in the herd · {nextBell.pen} SOL in the pen</div>
              <div className="mt-3 font-pixel text-xs text-hc-orange">Answer the call →</div>
            </Link>
          )}
        </section>

        <section aria-label="Token board" className="flex flex-col gap-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div role="group" aria-label="Filter tokens" className="flex flex-wrap gap-2">
              {TABS.map((t) => (
                <button
                  key={t.key}
                  type="button"
                  aria-pressed={tab === t.key}
                  onClick={() => setTab(t.key)}
                  className={`min-h-[40px] px-3 border-[3px] font-pixel text-xs cursor-pointer ${
                    tab === t.key ? 'bg-hc-orange text-hc-bg border-hc-bg font-bold' : 'bg-hc-bg text-hc-cream border-hc-line hover:border-hc-orange'
                  }`}
                >
                  {t.label}
                </button>
              ))}
            </div>
            <label className="inline-flex items-center gap-2 font-pixel text-xs cursor-pointer select-none">
              <input
                id="board-bellwether"
                type="checkbox"
                checked={bellwetherOnly}
                onChange={(e) => setBellwetherOnly(e.target.checked)}
                className="w-5 h-5 accent-[#FF7A1A] cursor-pointer"
              />
              Bellwether only
            </label>
          </div>

          {tokens.length > 0 ? (
            <div className="relative border-[3px] border-hc-line overflow-x-auto">
              <table className="w-full min-w-[760px] border-collapse text-left">
                <thead>
                  <tr className="font-pixel text-[11px] text-hc-muted">
                    <th scope="col" className="px-2.5 py-3 border-b-[3px] border-hc-line font-normal">Token</th>
                    <th scope="col" className="px-2.5 py-3 border-b-[3px] border-hc-line font-normal">Mkt cap / pen</th>
                    <th scope="col" className="px-2.5 py-3 border-b-[3px] border-hc-line font-normal">24h vol</th>
                    <th scope="col" className="px-2.5 py-3 border-b-[3px] border-hc-line font-normal">24h</th>
                    <th scope="col" className="px-2.5 py-3 border-b-[3px] border-hc-line font-normal" title="Fair Score">Fair</th>
                    <th scope="col" className="px-2.5 py-3 border-b-[3px] border-hc-line font-normal">Stage</th>
                    <th scope="col" className="px-2.5 py-3 border-b-[3px] border-hc-line font-normal"><span className="sr-only">Action</span></th>
                  </tr>
                </thead>
                <tbody>
                  {tokens.map((t) => (
                    <TokenRow key={t.symbol} t={t} secsLeft={secsLeft(t)} />
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <div className="border-[3px] border-dashed border-hc-line p-8 text-center">
              <p className="m-0 font-pixel text-sm">No tokens match these filters.</p>
              <p className="mt-2 mb-0 text-hc-muted">Clear the search or pick another tab. Or start the herd yourself.</p>
              <PixelButton to="/create" className="mt-4">Launch a token</PixelButton>
            </div>
          )}
          <p className="m-0 text-sm text-hc-muted">
            {tokens.length} tokens · example data. Live launches will appear here once the Solana program is deployed.
          </p>
        </section>

        <section className="grid grid-cols-[repeat(auto-fit,minmax(200px,1fr))] gap-4">
          {[
            { Icon: HLogo, title: 'Fair launch', body: 'One clearing price for everyone in the Lobby.', to: '/how-it-works' },
            { Icon: ChartIcon, title: 'Fair Score', body: 'Creator history, holder clusters and risk on every token.', to: '/how-it-works' },
            { Icon: CycleIcon, title: 'Rekt-back', body: 'Capped compensation when a creator breaks the rules.', to: '/rekt-back' },
            { Icon: HLogo, title: 'Diamond Hooves', body: 'Held 7+ days and sold at a loss? Get part of your trading fees back.', to: '/rekt-back' },
          ].map(({ Icon, title, body, to }) => (
            <Link key={title} to={to} className="flex gap-3 items-start no-underline text-hc-cream bg-hc-surface border-[3px] border-hc-line p-4 hover:border-hc-orange hover:text-hc-cream">
              <Icon size={28} />
              <span>
                <span className="block font-pixel font-bold text-sm">{title}</span>
                <span className="block mt-1 text-[15px] text-hc-muted">{body}</span>
              </span>
            </Link>
          ))}
        </section>
      </div>

      <aside className="xl:w-[300px] shrink-0 grid grid-cols-[repeat(auto-fit,minmax(280px,1fr))] xl:flex xl:flex-col gap-5 content-start">
        <RevenuePanel />

        <Card aria-label="Quick filters">
          <CardTitle className="text-base">Quick filters</CardTitle>
          <ul className="list-none mt-3 mb-0 p-0 flex flex-col gap-1">
            {QUICK_FILTERS.map((f) => (
              <li key={f.tab}>
                <button
                  type="button"
                  onClick={() => setTab(f.tab)}
                  className={`w-full text-left px-2 py-2 border-[3px] cursor-pointer bg-transparent text-hc-cream ${tab === f.tab ? 'border-hc-orange' : 'border-transparent hover:border-hc-line'}`}
                >
                  <span className="block font-pixel text-xs">{f.title}</span>
                  <span className="block text-sm text-hc-muted">{f.note}</span>
                </button>
              </li>
            ))}
          </ul>
        </Card>

        <MiniList title="Trending now" items={trending} render={(t) => <Change value={t.change24h} />} />
        <MiniList title="Top volume (24h)" items={topVolume} render={(t) => <span className="font-terminal text-xl">{formatUsd(t.vol24hUsd)}</span>} />

        <Card aria-label="Create your token" className="bg-hc-navy border-hc-navy">
          <CardTitle className="text-base">Ring your own bell</CardTitle>
          <p className="mt-2 mb-0 text-[15px] text-hc-body">Launch in under a minute. Bond SOL to become a Bellwether and earn 3× creator fees.</p>
          <PixelButton to="/create" className="mt-4">Launch a token</PixelButton>
        </Card>
      </aside>
    </main>
  );
}

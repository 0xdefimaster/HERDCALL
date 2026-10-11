import React, { useEffect, useMemo, useState } from 'react';
import { Link } from 'react-router-dom';
import { Bar, PixelButton, Tag, TokenAvatar } from '../components/ui';
import { MOCK_BOARD, formatAge, formatUsd } from '../data/mock';
import { formatClock } from './Lobby';

const TABS = [
  { key: 'all', label: 'All' },
  { key: 'new', label: 'New' },
  { key: 'lobby', label: 'Lobby open' },
  { key: 'near', label: 'Near breakout' },
  { key: 'broke', label: 'Broke out' },
  { key: 'takeover', label: 'Takeovers' },
];

const SORTS = [
  { key: 'newest', label: 'Newest' },
  { key: 'mcap', label: 'Market cap' },
  { key: 'fair', label: 'Fair Score' },
  { key: 'herd', label: 'Herd size' },
];

const NEAR_BREAKOUT = 80;

const TAB_FILTERS = {
  all: () => true,
  new: (t) => t.status === 'lobby' || t.ageMin <= 15,
  lobby: (t) => t.status === 'lobby',
  near: (t) => t.status === 'curve' && t.breakoutProgress >= NEAR_BREAKOUT,
  broke: (t) => t.status === 'broke',
  takeover: (t) => t.status === 'takeover',
};

const SORTERS = {
  newest: (a, b) => a.ageMin - b.ageMin,
  mcap: (a, b) => b.marketCapUsd - a.marketCapUsd,
  fair: (a, b) => b.fairScore - a.fairScore,
  herd: (a, b) => b.herd - a.herd,
};

function tokenHref(t) {
  if (t.status === 'lobby') return `/lobby/${t.symbol}`;
  if (t.status === 'takeover') return '/takeover';
  return `/token/${t.symbol}`;
}

function statusTag(t, secsLeft) {
  if (t.status === 'lobby') return secsLeft > 0 ? <Tag tone="orange">Bell in {formatClock(secsLeft)}</Tag> : <Tag tone="orange">Bell rang</Tag>;
  if (t.status === 'broke') return <Tag tone="outline">Broke out</Tag>;
  if (t.status === 'takeover') return <Tag tone="outline">Takeover vote</Tag>;
  if (t.breakoutProgress >= NEAR_BREAKOUT) return <Tag tone="outline">Near breakout</Tag>;
  return <Tag tone="outline">{formatAge(t.ageMin)} ago</Tag>;
}

function FairChip({ score }) {
  const tone = score >= 75 ? 'text-hc-navy-text border-hc-navy-text' : score >= 50 ? 'text-hc-body border-hc-strong' : 'text-hc-warn border-hc-warn';
  return (
    <span className={`inline-flex shrink-0 items-center gap-1 px-1.5 border-2 whitespace-nowrap font-terminal text-lg leading-tight ${tone}`} title="Fair Score">
      FS {score}
    </span>
  );
}

function TokenCard({ t, secsLeft }) {
  const inLobby = t.status === 'lobby';
  return (
    <Link
      to={tokenHref(t)}
      className="group block no-underline text-hc-cream bg-hc-surface border-[3px] border-hc-line p-4 hover:border-hc-orange hover:text-hc-cream hover:shadow-px-navy focus-visible:outline focus-visible:outline-[3px] focus-visible:outline-hc-orange transition-[border-color]"
    >
      <div className="flex items-start gap-3">
        <TokenAvatar initial={t.initial} size="w-14 h-14 text-2xl" />
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline justify-between gap-2">
            <span className="font-pixel font-bold text-[15px] leading-tight min-w-0 break-words">{t.name}</span>
            <FairChip score={t.fairScore} />
          </div>
          <div className="font-terminal text-xl text-hc-muted leading-tight">${t.symbol}</div>
          <div className="mt-1.5 flex flex-wrap gap-1.5">
            {statusTag(t, secsLeft)}
            {t.bonded ? <Tag>Bellwether</Tag> : <Tag tone="outline">Unbonded</Tag>}
          </div>
        </div>
      </div>
      <p className="mt-3 mb-0 text-[15px] text-hc-body truncate">{t.description}</p>
      <div className="mt-3 grid grid-cols-3 gap-2 text-sm">
        <div>
          <div className="text-hc-muted">{inLobby ? 'In the pen' : 'Mkt cap'}</div>
          <div className="font-terminal text-[22px] leading-none whitespace-nowrap">{inLobby ? `${t.pen} SOL` : formatUsd(t.marketCapUsd)}</div>
        </div>
        <div>
          <div className="text-hc-muted">Herd</div>
          <div className="font-terminal text-[22px] leading-none">{t.herd.toLocaleString('en-US')}</div>
        </div>
        <div>
          <div className="text-hc-muted">{inLobby ? 'Opened' : 'Age'}</div>
          <div className="font-terminal text-[22px] leading-none whitespace-nowrap">{inLobby ? 'now' : formatAge(t.ageMin)}</div>
        </div>
      </div>
      {!inLobby && (
        <div className="mt-3">
          <div className="flex justify-between text-[13px] text-hc-muted">
            <span>Breakout</span>
            <span className="font-terminal text-lg leading-none">{t.breakoutProgress}%</span>
          </div>
          <Bar value={t.breakoutProgress} className="mt-1 h-3 border-2" />
        </div>
      )}
    </Link>
  );
}

export default function Board() {
  const [elapsed, setElapsed] = useState(0);
  const [tab, setTab] = useState('all');
  const [sort, setSort] = useState('newest');
  const [bellwetherOnly, setBellwetherOnly] = useState(false);
  const [query, setQuery] = useState('');

  useEffect(() => {
    const timer = setInterval(() => setElapsed((e) => e + 1), 1000);
    return () => clearInterval(timer);
  }, []);

  const secsLeft = (t) => Math.max(0, t.lobbySecs - elapsed);
  const lobbies = MOCK_BOARD.filter((t) => t.status === 'lobby').sort((a, b) => a.lobbySecs - b.lobbySecs);

  const tokens = useMemo(() => {
    const q = query.trim().toLowerCase();
    return MOCK_BOARD.filter(TAB_FILTERS[tab])
      .filter((t) => !bellwetherOnly || t.bonded)
      .filter((t) => !q || t.name.toLowerCase().includes(q) || t.symbol.toLowerCase().includes(q))
      .sort(SORTERS[sort]);
  }, [tab, sort, bellwetherOnly, query]);

  return (
    <main className="max-w-[1240px] mx-auto px-6 pt-8 pb-14 flex flex-col gap-8">
      <section className="flex flex-wrap items-end justify-between gap-4">
        <div className="min-w-0">
          <h1 className="m-0 font-pixel font-bold uppercase leading-tight text-[clamp(26px,3.4vw,40px)]">
            No snipers. <span className="text-hc-orange">Just the herd.</span>
          </h1>
          <p className="mt-2 mb-0 text-hc-body">Every launch opens with a countdown. When the bell rings, everyone enters at one price.</p>
        </div>
        <div className="flex flex-wrap gap-3">
          <PixelButton to="/create">Launch a token</PixelButton>
          <PixelButton to="/how-it-works" variant="secondary">How it works</PixelButton>
        </div>
      </section>

      <section aria-label="Bell rings soon">
        <h2 className="m-0 font-pixel font-bold text-lg uppercase">Bell rings soon</h2>
        <div className="mt-3 grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-4">
          {lobbies.map((t) => {
            const left = secsLeft(t);
            return (
              <Link
                key={t.symbol}
                to={`/lobby/${t.symbol}`}
                className="block no-underline text-hc-cream bg-hc-surface border-[3px] border-hc-orange shadow-px-navy p-4 hover:text-hc-cream hover:bg-hc-bg focus-visible:outline focus-visible:outline-[3px] focus-visible:outline-hc-cream"
              >
                <div className="flex items-center justify-between gap-3">
                  <div className="flex items-center gap-3 min-w-0">
                    <TokenAvatar initial={t.initial} size="w-11 h-11 text-xl" />
                    <div className="min-w-0">
                      <div className="font-pixel font-bold text-sm leading-tight">{t.name}</div>
                      <div className="font-terminal text-lg text-hc-muted leading-tight">${t.symbol}</div>
                    </div>
                  </div>
                  <div className="font-terminal text-[44px] leading-none text-hc-orange" aria-live="off">
                    {left > 0 ? formatClock(left) : 'IN'}
                  </div>
                </div>
                <div className="mt-3 flex justify-between gap-2 text-sm text-hc-muted">
                  <span>{t.herd} in the herd · {t.pen} SOL</span>
                  <span className="font-pixel text-xs text-hc-orange">{left > 0 ? 'Join' : 'Bell rang'}</span>
                </div>
              </Link>
            );
          })}
        </div>
      </section>

      <section aria-label="Token board" className="flex flex-col gap-4">
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
          <div className="flex flex-wrap items-center gap-3">
            <label htmlFor="board-search" className="sr-only">Search tokens</label>
            <input
              id="board-search"
              type="search"
              placeholder="Search name or ticker"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              className="min-h-[40px] w-[200px] max-w-full px-3 border-[3px] border-hc-line bg-hc-bg text-hc-cream placeholder:text-hc-muted"
            />
            <label htmlFor="board-sort" className="sr-only">Sort tokens</label>
            <select
              id="board-sort"
              value={sort}
              onChange={(e) => setSort(e.target.value)}
              className="min-h-[40px] px-2 border-[3px] border-hc-line bg-hc-bg text-hc-cream font-pixel text-xs cursor-pointer"
            >
              {SORTS.map((s) => (
                <option key={s.key} value={s.key}>Sort: {s.label}</option>
              ))}
            </select>
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
        </div>

        {tokens.length > 0 ? (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-4">
            {tokens.map((t) => (
              <TokenCard key={t.symbol} t={t} secsLeft={secsLeft(t)} />
            ))}
          </div>
        ) : (
          <div className="border-[3px] border-dashed border-hc-line p-8 text-center">
            <p className="m-0 font-pixel text-sm">No tokens match these filters.</p>
            <p className="mt-2 mb-0 text-hc-muted">Clear the search or pick another tab. Or start the herd yourself.</p>
            <PixelButton to="/create" className="mt-4">Launch a token</PixelButton>
          </div>
        )}
        <p className="m-0 text-sm text-hc-muted">Example tokens for the preview. Live launches will appear here once the Solana program is deployed.</p>
      </section>
    </main>
  );
}

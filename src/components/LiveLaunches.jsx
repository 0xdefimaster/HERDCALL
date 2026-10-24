import React from 'react';
import { Link } from 'react-router-dom';
import { Card, CardTitle, PixelButton, Tag, TokenAvatar } from './ui';
import { useHerdCall, usePoll } from '../chain/HerdCallContext';
import { PHASES, big, phaseOf, sol, tokens } from '../chain/format';

// Launches on the configured cluster, newest first. The rest of the board below is preview data.
export default function LiveLaunches() {
  const { enabled, program, cluster, deployed } = useHerdCall();
  const { data, error } = usePoll(async () => {
    if (!program || !deployed) return [];
    const all = await program.account.launch.all();
    return all.map((a) => a.account).sort((a, b) => b.closeAt.toNumber() - a.closeAt.toNumber());
  }, [program, deployed], 6000);
  if (!enabled) return null;

  return (
    <Card aria-label="Live launches" highlight className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <CardTitle>Live on {cluster}</CardTitle>
        <PixelButton to="/create" className="text-[13px]">Launch</PixelButton>
      </div>
      {!deployed && <p className="m-0 text-hc-muted">The HerdCall program is not deployed on this cluster. See Setup.</p>}
      {error && <p role="alert" className="m-0 text-hc-down">{String(error.message ?? error)}</p>}
      {deployed && data?.length === 0 && <p className="m-0 text-hc-muted">No launches yet.</p>}
      <ul className="list-none m-0 p-0 flex flex-col gap-2">
        {(data ?? []).map((l) => {
          const phase = phaseOf(l);
          const progress = phase === 'lobby' ? `${sol(l.totalDeposits, 2)} SOL in the pen` : phase === 'cancelled' ? 'Cancelled' : `${(Number((big(l.sold) * 1000n) / big(l.params.curveSupply)) / 10).toFixed(1)}% of the curve · ${sol(l.reserve, 2)} SOL`;
          return (
            <li key={l.mint.toBase58()}>
              <Link to={`/launch/${l.mint.toBase58()}`} className="flex items-center gap-3 p-2.5 border-[3px] border-hc-line bg-hc-bg no-underline text-hc-cream hover:border-hc-orange">
                <TokenAvatar initial={l.ticker.slice(0, 1)} size="w-10 h-10 text-lg" />
                <div className="min-w-0 flex-1">
                  <div className="font-pixel text-sm truncate">{l.name}</div>
                  <div className="font-terminal text-lg text-hc-muted">${l.ticker} · {progress}</div>
                </div>
                <Tag tone={phase === 'lobby' ? 'orange' : 'outline'}>{PHASES[phase]}</Tag>
                {phase === 'migrated' && <span className="hidden sm:inline font-terminal text-lg text-hc-muted">{tokens(l.lpTokens, 0)} in pool</span>}
              </Link>
            </li>
          );
        })}
      </ul>
      <p className="m-0 text-sm text-hc-muted">The board below is preview data.</p>
    </Card>
  );
}

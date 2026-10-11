import React, { useState } from 'react';
import { CheckIcon } from '../components/icons';
import { Bar, Card, CardTitle, PixelButton, Tag } from '../components/ui';
import { MOCK_TAKEOVER } from '../data/mock';
import { FEE_SPLIT } from '../lib/fees';
import { useWallet } from '../wallet/WalletContext';

const TIMELINE = [
  { step: 'Creator trampled', when: 'Day 19' },
  { step: 'Takeover proposed', when: 'Day 20' },
  { step: 'Voting', when: 'Now', current: true },
  { step: 'Timelock', when: '[T]h after pass' },
  { step: 'Handover to multisig', when: 'Automatic' },
];

export default function Takeover() {
  const tk = MOCK_TAKEOVER;
  const { publicKey, connect } = useWallet();
  const [vote, setVote] = useState(null);
  const forPct = tk.forPct + (vote === 'for' ? 1 : 0);
  const againstPct = tk.againstPct + (vote === 'against' ? 1 : 0);

  return (
    <main className="max-w-[1240px] mx-auto px-6 pt-8 pb-14 flex flex-col gap-6">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <div className="flex flex-wrap items-center gap-2.5">
            <Tag>Community Takeover</Tag>
            <Tag tone="orange">Vote open</Tag>
          </div>
          <h1 className="mt-3.5 mb-0 font-pixel font-bold uppercase leading-[1.05] text-[clamp(30px,4.2vw,52px)]">
            ${tk.symbol}: The herd
            <br />
            <span className="text-hc-orange">takes the reins.</span>
          </h1>
        </div>
        <div className="text-right">
          <div className="text-sm text-hc-muted">Voting ends in</div>
          <div className="font-terminal text-[52px] leading-[0.9]">{tk.endsIn}</div>
        </div>
      </div>

      <div className="flex flex-wrap gap-6 items-start">
        <div className="flex-[999_1_560px] min-w-0 flex flex-col gap-6">
          <Card aria-label="Trigger">
            <CardTitle>Why this takeover is allowed</CardTitle>
            <ul className="list-none mt-3.5 mb-0 p-0 flex flex-col gap-3">
              <li className="flex gap-3 items-start">
                <CheckIcon />
                <span>
                  <strong>Creator trampled.</strong>{' '}
                  <span className="text-hc-body">Bonded creator sold 40% of their vesting bag on day 19. Bond (3 SOL) and escrowed fees (1.8 SOL) went to the Rekt-back pool.</span>
                </span>
              </li>
              <li className="flex gap-3 items-start">
                <CheckIcon />
                <span>
                  <strong>Proposal threshold met.</strong>{' '}
                  <span className="text-hc-body">Opened by holders with {tk.proposerShare} of supply (minimum [Y]%).</span>
                </span>
              </li>
            </ul>
          </Card>

          <Card aria-label="What moves">
            <CardTitle>What moves to the herd</CardTitle>
            <div className="mt-3.5 grid grid-cols-[repeat(auto-fit,minmax(200px,1fr))] gap-3.5">
              <div className="bg-hc-bg border-[3px] border-hc-line p-3.5">
                <div className="font-pixel font-bold text-sm">Creator fees</div>
                <div className="font-terminal text-2xl text-hc-orange">{FEE_SPLIT.bonded.creator.toFixed(2)}% of volume</div>
              </div>
              <div className="bg-hc-bg border-[3px] border-hc-line p-3.5">
                <div className="font-pixel font-bold text-sm">Metadata</div>
                <div className="text-[15px] text-hc-muted">Image, description, links</div>
              </div>
              <div className="bg-hc-bg border-[3px] border-hc-line p-3.5">
                <div className="font-pixel font-bold text-sm">Creator role</div>
                <div className="text-[15px] text-hc-muted">Shown as “Community-run”</div>
              </div>
            </div>
          </Card>

          <Card aria-label="Proposed multisig">
            <div className="flex flex-wrap justify-between gap-2 items-baseline">
              <CardTitle>Proposed multisig</CardTitle>
              <span className="font-terminal text-[26px] text-hc-orange">3 of {tk.signers.length}</span>
            </div>
            <ul className="list-none mt-3.5 mb-0 p-0 flex flex-col gap-2 text-base">
              {tk.signers.map((s) => (
                <li key={s.who} className="flex justify-between gap-3 pb-2 border-b-2 border-dashed border-hc-line">
                  <span className="font-terminal text-[22px]">{s.who}</span>
                  <span className="text-hc-muted">{s.note}</span>
                </li>
              ))}
            </ul>
          </Card>
        </div>

        <div className="flex-[1_1_340px] min-w-0 flex flex-col gap-6">
          <Card aria-label="Vote" highlight>
            <CardTitle className="text-xl">Cast your vote</CardTitle>
            <div className="mt-4 flex flex-col gap-3.5">
              <div>
                <div className="flex justify-between gap-2">
                  <span className="font-pixel text-sm">For</span>
                  <span className="font-terminal text-2xl leading-none">{forPct}%</span>
                </div>
                <Bar value={forPct} className="mt-1.5 h-[18px]" />
              </div>
              <div>
                <div className="flex justify-between gap-2">
                  <span className="font-pixel text-sm">Against</span>
                  <span className="font-terminal text-2xl leading-none">{againstPct}%</span>
                </div>
                <Bar value={againstPct} color="bg-hc-navy-text" className="mt-1.5 h-[18px]" />
              </div>
            </div>
            <div className="mt-4 flex justify-between gap-2 text-[15px] text-hc-muted">
              <span>Quorum</span>
              <span className="text-hc-cream">{tk.quorum} · min [Q]%</span>
            </div>
            <div className="mt-1 flex justify-between gap-2 text-[15px] text-hc-muted">
              <span>Your voting power</span>
              <span className="text-hc-cream">{publicKey ? tk.votingPower : '—'}</span>
            </div>
            {vote ? (
              <div className="mt-5 min-h-[54px] bg-hc-navy text-hc-navy-text font-pixel text-[13px] flex items-center justify-center text-center px-2.5">
                {vote === 'for' ? 'You voted for the takeover' : 'You voted against'}
              </div>
            ) : !publicKey ? (
              <PixelButton onClick={connect} className="mt-5 w-full min-h-[54px]">Connect wallet to vote</PixelButton>
            ) : (
              <div className="mt-5 grid grid-cols-2 gap-3">
                <PixelButton onClick={() => setVote('for')} className="min-h-[54px]">Vote for</PixelButton>
                <PixelButton variant="ghost" onClick={() => setVote('against')} className="min-h-[54px] text-[15px]">Against</PixelButton>
              </div>
            )}
          </Card>

          <Card aria-label="Timeline">
            <CardTitle>Timeline</CardTitle>
            <ol className="list-none mt-3.5 mb-0 p-0 flex flex-col gap-2.5 text-base">
              {TIMELINE.map((t) => (
                <li key={t.step} className="flex justify-between gap-2">
                  <span className={t.current ? 'text-hc-orange font-bold' : ''}>{t.step}</span>
                  <span className={t.current ? 'text-hc-orange' : 'text-hc-muted'}>{t.when}</span>
                </li>
              ))}
            </ol>
            <p className="mt-3.5 mb-0 text-sm text-hc-muted">Takeovers can't be opened against an active, bonded creator.</p>
          </Card>
        </div>
      </div>
    </main>
  );
}

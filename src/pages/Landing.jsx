import React from 'react';
import { Link } from 'react-router-dom';
import { ArrowIcon, ChartIcon, CycleIcon, FlagIcon, HLogo } from '../components/icons';
import { PixelButton, Tag } from '../components/ui';
import { FEE_SPLIT, PROTOCOL_SPLIT } from '../lib/fees';

const STEPS = [
  {
    label: '01 / The Lobby',
    title: 'Get in before the bell.',
    body: "See the rules, pick your size, wait for the call. No trades happen in the Lobby, so there's nothing to snipe.",
  },
  {
    label: '02 / The Stampede',
    title: 'The herd enters together.',
    body: 'The bell rings and the whole pen clears as one batch, at one price, under on-chain rules. Then the curve opens for instant trading.',
  },
  {
    label: '03 / The Breakout',
    title: 'Let the market decide.',
    body: 'Curve fills, liquidity migrates to a DEX and locks, mint and freeze authority are revoked by the protocol.',
  },
];

const FEATURES = [
  {
    Icon: ChartIcon,
    title: 'Fair Score',
    tagline: 'Holder concentration & risk',
    body: 'One number on every token: creator history, top holders, linked-wallet clusters. A warning, never a gate.',
  },
  {
    Icon: CycleIcon,
    title: 'Rekt-back',
    tagline: 'Capped pool compensation',
    body: 'When a creator breaks the rules, holders are paid pro-rata from the pool. Plus XP for coming back.',
  },
  {
    Icon: CycleIcon,
    title: 'Lucky Hoof',
    tagline: '1 in 100 losses refunded',
    body: 'Held a bag 7+ days and sold at a loss, or held it to zero? Every week one in every hundred tickets gets its loss back, up to 5 SOL.',
  },
  {
    Icon: HLogo,
    title: 'Bonded Creator',
    tagline: 'Bond, vesting & escrow',
    body: 'Creators who bond SOL and vest their bag become Bellwethers: 3× creator fees and a feed boost. Dump early, lose it all.',
  },
  {
    Icon: FlagIcon,
    title: 'Takeover',
    tagline: 'Herd governance',
    body: 'Creator trampled or gone silent? Holders vote to move creator rights and fees to a community multisig.',
  },
];

const TAKEOVER_STEPS = [
  { title: '1. Trigger', body: 'Creator gets trampled, or shows no on-chain activity for [N] days.' },
  { title: '2. Proposal', body: 'Holders with at least [Y]% of supply open a takeover and nominate multisig signers.' },
  { title: '3. Vote', body: 'The herd votes. If it passes, a timelock runs before anything moves.' },
  { title: '4. Handover', body: 'Creator rights and all future creator fees flow to the community multisig.', highlight: true },
];

function SectionHeading({ children, accent }) {
  return (
    <h2 className="m-0 font-pixel font-bold uppercase leading-tight text-[clamp(30px,3.8vw,48px)]">
      {children}
      {accent && (
        <>
          <br />
          <span className="text-hc-orange">{accent}</span>
        </>
      )}
    </h2>
  );
}

function FeeBar({ title, split }) {
  const segments = [
    { key: 'protocol', label: `.${Math.round(split.protocol * 100)} protocol`, className: 'bg-hc-line text-hc-cream' },
    { key: 'creator', label: `.${Math.round(split.creator * 100)}${split.creator >= 0.2 ? ' creator' : ''}`, className: 'bg-hc-orange text-hc-bg' },
    { key: 'rektBack', label: `.${Math.round(split.rektBack * 100)}${split.rektBack >= 0.3 ? ' Rekt-back pool' : ' pool'}`, className: 'bg-hc-navy-text text-hc-bg' },
    { key: 'interface', label: `.${Math.round(split.interface * 100)}`, className: 'bg-hc-cream text-hc-bg' },
  ];
  return (
    <div>
      <div className="flex justify-between gap-3 font-pixel font-bold text-[15px]">
        <span>{title}</span>
        <span>1.00%</span>
      </div>
      <div className="mt-3 flex h-[52px] border-[3px] border-hc-bg font-terminal text-[22px]">
        {segments.map((s) => (
          <div key={s.key} className={`flex items-center pl-2 overflow-hidden whitespace-nowrap ${s.className}`} style={{ flex: split[s.key] * 100 }}>
            {s.label}
          </div>
        ))}
      </div>
    </div>
  );
}

export default function Landing() {
  return (
    <main>
      <section className="max-w-[1240px] mx-auto px-6 pt-[72px] pb-16 flex flex-wrap items-center gap-14">
        <div className="flex-[1_1_520px] min-w-0">
          <Tag>Solana · Community launchpad</Tag>
          <h1 className="mt-6 mb-0 font-pixel font-bold uppercase leading-[1.05] text-[clamp(36px,4.6vw,60px)]">
            No snipers.
            <br />
            <span className="text-hc-orange">Just the herd.</span>
          </h1>
          <p className="mt-6 max-w-[540px] text-[21px] text-hc-body">
            Every launch opens with a countdown. When the bell rings, the whole herd enters together, at one price. Bots camping the first block get nothing.
          </p>
          <div className="mt-9 flex flex-wrap gap-4">
            <PixelButton to="/lobby" className="min-h-[56px] text-[17px] px-6">
              Answer the call <ArrowIcon />
            </PixelButton>
            <PixelButton to="/create" variant="secondary" className="min-h-[56px] text-[17px] px-6">
              Launch a token
            </PixelButton>
          </div>
          <p className="mt-8 font-pixel text-[13px] text-hc-muted">No paid calls. Just the herd call.</p>
        </div>
        <div className="flex-[1_1_440px] min-w-0 flex justify-center">
          <img
            src="/herdcall-hero.webp"
            alt="HerdCall mascot: a pixel-art bull in a navy cap, hoodie and shades, pointing forward in front of a cheering herd, with the HerdCall wordmark below"
            className="block w-full max-w-[540px] h-auto aspect-square object-cover border-[3px] border-hc-orange shadow-px-navy-lg [image-rendering:pixelated]"
          />
        </div>
      </section>

      <div className="bg-hc-orange text-hc-bg border-y-[3px] border-hc-bg">
        <div className="max-w-[1240px] mx-auto px-6 py-3.5 flex flex-wrap justify-between gap-x-8 gap-y-2 font-pixel font-bold text-[15px]">
          <span>Bell rings in 00:27</span>
          <span>$MOOVE lobby open</span>
          <span>412 in the pen</span>
          <span>0 snipers with an edge</span>
        </div>
      </div>

      <section className="max-w-[1240px] mx-auto px-6 pt-6">
        {STEPS.map((step, i) => (
          <div
            key={step.label}
            className={`flex flex-wrap items-baseline gap-x-10 gap-y-4 py-11 ${i < STEPS.length - 1 ? 'border-b-[3px] border-dashed border-hc-line' : ''}`}
          >
            <div className="min-w-[220px] font-pixel font-bold text-[15px] text-hc-orange">{step.label}</div>
            <h2 className="flex-[1_1_420px] min-w-0 m-0 font-pixel font-bold uppercase leading-tight text-[clamp(28px,3.4vw,44px)]">{step.title}</h2>
            <p className="flex-[1_1_300px] min-w-0 m-0 text-hc-body">{step.body}</p>
          </div>
        ))}
      </section>

      <section className="bg-hc-surface border-y-[3px] border-hc-line">
        <div className="max-w-[1240px] mx-auto px-6 py-20">
          <SectionHeading accent="Different rules.">Same degen fun.</SectionHeading>
          <div className="mt-11 grid grid-cols-[repeat(auto-fit,minmax(260px,1fr))] gap-6">
            {FEATURES.map(({ Icon, title, tagline, body }) => (
              <div key={title} className="bg-hc-bg border-[3px] border-hc-orange shadow-px-navy p-6">
                <Icon size={32} />
                <h3 className="mt-4 mb-0 font-pixel font-bold text-xl">{title}</h3>
                <div className="mt-1 font-pixel text-xs text-hc-navy-text">{tagline}</div>
                <p className="mt-3 mb-0 text-hc-body">{body}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      <section className="max-w-[1240px] mx-auto px-6 py-[88px]">
        <Tag>Community Takeover</Tag>
        <div className="mt-4">
          <SectionHeading accent="The herd takes the reins.">Dev ran?</SectionHeading>
        </div>
        <div className="mt-11 flex flex-wrap gap-5">
          {TAKEOVER_STEPS.map((step) => (
            <div
              key={step.title}
              className={`flex-[1_1_240px] min-w-0 border-[3px] p-5 ${step.highlight ? 'border-hc-orange bg-hc-surface shadow-px-navy' : 'border-hc-line'}`}
            >
              <div className="font-pixel font-bold text-hc-orange">{step.title}</div>
              <p className="mt-2.5 mb-0 text-hc-body">{step.body}</p>
            </div>
          ))}
        </div>
        <p className="mt-6 text-base text-hc-muted">
          A takeover can't be opened against an active, bonded creator. <Link to="/takeover">See a live takeover</Link>
        </p>
      </section>

      <section className="bg-hc-surface border-y-[3px] border-hc-line">
        <div className="max-w-[1240px] mx-auto px-6 py-20 flex flex-wrap gap-12 items-start">
          <div className="flex-[1_1_340px] min-w-0">
            <Tag>Where the 1% goes</Tag>
            <div className="mt-4">
              <SectionHeading>Fees you can read on-chain.</SectionHeading>
            </div>
            <p className="mt-5 max-w-[420px] text-hc-body">One flat 1% fee on the curve. Every split is enforced by the protocol and published live.</p>
          </div>
          <div className="flex-[1_1_560px] min-w-0 flex flex-col gap-8">
            <FeeBar title="Bonded creator" split={FEE_SPLIT.bonded} />
            <FeeBar title="Unbonded creator" split={FEE_SPLIT.unbonded} />
            <div className="flex flex-wrap gap-x-5 gap-y-2 text-[15px] text-hc-muted">
              <span className="inline-flex items-center gap-2"><span className="w-3 h-3 bg-hc-orange" />Creator (50% escrowed until breakout)</span>
              <span className="inline-flex items-center gap-2"><span className="w-3 h-3 bg-hc-navy-text" />Rekt-back pool</span>
              <span className="inline-flex items-center gap-2"><span className="w-3 h-3 bg-hc-cream" />Interface / referral</span>
            </div>
            <div className="border-t-[3px] border-dashed border-hc-line pt-6">
              <div className="font-pixel font-bold text-[15px]">Protocol share, after a fixed ops floor</div>
              <div className="mt-3.5 grid grid-cols-[repeat(auto-fit,minmax(160px,1fr))] gap-3.5">
                {PROTOCOL_SPLIT.map((p) => (
                  <div key={p.key} className="bg-hc-bg border-[3px] border-hc-line p-4">
                    <div className="font-terminal text-[44px] leading-none text-hc-orange">{p.pct}%</div>
                    <div className="mt-1.5 font-pixel font-bold text-[15px]">{p.title}</div>
                    <div className="text-[15px] text-hc-muted">{p.note}</div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>
      </section>

      <section className="max-w-[1240px] mx-auto px-6 py-24 text-center">
        <div className="flex justify-center">
          <HLogo size={80} title="HerdCall logo" />
        </div>
        <h2 className="mt-6 mb-0 font-pixel font-bold uppercase leading-[1.05] text-[clamp(38px,5.6vw,76px)]">Answer the call.</h2>
        <p className="mt-4 mx-auto max-w-[520px] text-[21px] text-hc-body">Everyone runs. Nobody front-runs.</p>
        <div className="mt-9 flex flex-wrap justify-center gap-4">
          <PixelButton to="/lobby" className="min-h-[56px] text-[17px] px-6">Enter the Lobby</PixelButton>
          <PixelButton to="/create" variant="secondary" className="min-h-[56px] text-[17px] px-6">Launch a token</PixelButton>
        </div>
      </section>
    </main>
  );
}

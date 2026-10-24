import React, { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import * as anchor from '@anchor-lang/core';
import { HLogo, ImageIcon } from '../components/icons';
import { Card, CardTitle, PixelButton, Row } from '../components/ui';
import { BOND_OPTIONS, FEE_SPLIT, LOBBY_WINDOWS, VEST_OPTIONS, creatorPayout, formatPct } from '../lib/fees';
import { canonicalName, canonicalTicker, createLaunchAccounts, launchKeys, nameHash } from '../../scripts/herdcall-client.mjs';
import { programError, useHerdCall } from '../chain/HerdCallContext';
import { sol } from '../chain/format';
import { useWallet } from '../wallet/WalletContext';

const { web3, BN } = anchor;
const MAX_URI = 200;

function PillGroup({ label, options, value, onChange, suffix }) {
  return (
    <div role="group" aria-label={label} className="flex flex-wrap gap-2.5">
      {options.map((option) => {
        const selected = option === value;
        return (
          <button
            key={option}
            type="button"
            aria-pressed={selected}
            onClick={() => onChange(option)}
            className={`min-h-[44px] px-4 border-[3px] font-terminal text-2xl cursor-pointer ${
              selected ? 'border-hc-bg bg-hc-orange text-hc-bg shadow-[3px_3px_0_#FFF0DC]' : 'border-hc-line bg-hc-bg text-hc-cream'
            }`}
          >
            {option}
            {suffix}
          </button>
        );
      })}
    </div>
  );
}

const INPUT = 'min-h-[48px] px-3 border-[3px] border-hc-line bg-hc-bg text-hc-cream';

export default function Create() {
  const [form, setForm] = useState({
    name: 'Moove',
    ticker: 'MOOVE',
    description: 'The cow that jumped over the moon, now on Solana.',
  });
  const [lobbyWindow, setLobbyWindow] = useState(30);
  const [bonded, setBonded] = useState(true);
  const [bond, setBond] = useState(3);
  const [vest, setVest] = useState(60);
  const [uri, setUri] = useState('');
  const [sending, setSending] = useState(false);
  const [error, setError] = useState(null);
  const hc = useHerdCall();
  const { connect } = useWallet();
  const navigate = useNavigate();
  const live = hc.enabled && hc.config;

  const split = bonded ? FEE_SPLIT.bonded : FEE_SPLIT.unbonded;
  const payout = creatorPayout(bonded);
  const update = (field) => (e) => setForm((f) => ({ ...f, [field]: e.target.value }));
  const ticker = canonicalTicker(form.ticker);
  const nameOk = canonicalName(form.name) !== null;
  const invalid = !ticker ? 'Ticker: 1–10 letters or digits' : !nameOk ? 'Name: 1–32 characters, one line' : uri.length > MAX_URI ? `Metadata URI: at most ${MAX_URI} characters` : null;

  const createLaunch = async () => {
    setError(null);
    setSending(true);
    try {
      const mint = web3.Keypair.generate();
      const name = form.name.trim();
      const k = launchKeys(mint.publicKey, ticker, name);
      await hc.send(
        hc.program.methods.createLaunch(bonded, new BN(lobbyWindow), ticker, name, nameHash(name), uri).accountsStrict(createLaunchAccounts(hc.user, k)),
        { signers: [mint] },
      );
      navigate(`/launch/${mint.publicKey.toBase58()}`);
    } catch (e) {
      setError(programError(e?.logs) ?? e?.message ?? String(e));
    } finally {
      setSending(false);
    }
  };

  return (
    <main className="max-w-[1240px] mx-auto px-6 pt-8 pb-14">
      <h1 className="m-0 font-pixel font-bold uppercase leading-[1.05] text-[clamp(34px,4.6vw,56px)]">Ring the bell.</h1>
      <p className="mt-3 mb-0 text-hc-body text-[19px]">Launch in under a minute. Your token opens with a Lobby, so no bot gets in before your herd.</p>

      <div className="mt-8 flex flex-wrap gap-6 items-start">
        <form className="flex-[999_1_560px] min-w-0 flex flex-col gap-6" onSubmit={(e) => e.preventDefault()}>
          <Card className="flex flex-col gap-4">
            <CardTitle>The basics</CardTitle>
            <div className="flex flex-wrap gap-4">
              <div className="flex-[2_1_240px] min-w-0 flex flex-col gap-1.5">
                <label htmlFor="name" className="text-[15px] text-hc-muted">Name</label>
                <input id="name" type="text" value={form.name} onChange={update('name')} className={`${INPUT} text-lg`} />
              </div>
              <div className="flex-[1_1_140px] min-w-0 flex flex-col gap-1.5">
                <label htmlFor="ticker" className="text-[15px] text-hc-muted">Ticker</label>
                <input id="ticker" type="text" value={form.ticker} onChange={update('ticker')} className={`${INPUT} font-terminal text-[26px] uppercase`} />
              </div>
            </div>
            <div className="flex flex-col gap-1.5">
              <label htmlFor="desc" className="text-[15px] text-hc-muted">Description</label>
              <textarea id="desc" rows={3} value={form.description} onChange={update('description')} className={`${INPUT} py-2.5 resize-y`} />
            </div>
            {live ? (
              <div className="flex flex-col gap-1.5">
                <label htmlFor="uri" className="text-[15px] text-hc-muted">Metadata URI (JSON with image and description, hosted by you)</label>
                <input id="uri" type="url" value={uri} onChange={(e) => setUri(e.target.value.trim())} placeholder="https://…/token.json" className={`${INPUT} text-base`} />
                <p className="m-0 text-sm text-hc-muted">Name, ticker and URI are written to immutable Metaplex metadata. The description above is not stored on chain.</p>
              </div>
            ) : (
              <div className="flex flex-wrap gap-4 items-center">
                <div className="w-[84px] h-[84px] border-[3px] border-dashed border-hc-strong flex items-center justify-center">
                  <ImageIcon />
                </div>
                <PixelButton variant="ghost">Upload image</PixelButton>
              </div>
            )}
          </Card>

          <Card className="flex flex-col gap-3.5">
            <CardTitle>Lobby window</CardTitle>
            <p className="m-0 text-hc-muted text-base">How long the Lobby stays open before the bell rings and everyone enters at one price.</p>
            <PillGroup label="Lobby window" options={LOBBY_WINDOWS} value={lobbyWindow} onChange={setLobbyWindow} suffix="s" />
          </Card>

          <Card highlight={bonded} className="flex flex-col gap-4">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex items-center gap-3">
                <HLogo size={32} />
                <div>
                  <CardTitle>Become a Bellwether</CardTitle>
                  <div className="font-pixel text-xs text-hc-navy-text">Bonded Creator</div>
                </div>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={bonded}
                aria-label="Become a Bonded Creator"
                onClick={() => setBonded((b) => !b)}
                className={`w-[72px] h-10 border-[3px] border-hc-cream p-[3px] cursor-pointer flex ${bonded ? 'bg-hc-orange justify-end' : 'bg-hc-line justify-start'}`}
              >
                <span className="block w-7 h-7 bg-hc-cream" />
              </button>
            </div>
            <p className="m-0 text-hc-body">
              Bond SOL and vest your bag. You earn 3× creator fees, rank higher, and lead the herd. Sell before your vest ends and you get{' '}
              <strong className="text-hc-cream">trampled</strong>: bond and escrowed fees go to your holders, and the herd can vote a Community Takeover.
            </p>
            {bonded && live && (
              <p className="m-0 text-hc-warn">Bond and vesting are not in the program yet. On chain, Bonded sets the fee split and holds half of your creator fees in escrow; the program has no release for the escrow yet.</p>
            )}
            {bonded && !live && (
              <div className="flex flex-wrap gap-5">
                <div className="flex-[1_1_220px] min-w-0">
                  <div className="text-[15px] text-hc-muted mb-2">Bond (returned at breakout)</div>
                  <PillGroup label="Bond size" options={BOND_OPTIONS} value={bond} onChange={setBond} suffix=" SOL" />
                </div>
                <div className="flex-[1_1_220px] min-w-0">
                  <div className="text-[15px] text-hc-muted mb-2">Creator bag vesting</div>
                  <PillGroup label="Vesting length" options={VEST_OPTIONS} value={vest} onChange={setVest} suffix=" days" />
                </div>
              </div>
            )}
          </Card>
        </form>

        <Card as="aside" aria-label="Launch summary" highlight className="flex-[1_1_340px] min-w-0 flex flex-col gap-3">
          <CardTitle>Your launch</CardTitle>
          <Row label="Badge">
            <span className={`font-pixel text-[13px] ${bonded ? 'text-hc-navy-text' : 'text-hc-muted'}`}>{bonded ? 'Bellwether' : 'Unbonded'}</span>
          </Row>
          <Row label="Your creator fee"><span className="font-terminal text-[22px]">{formatPct(split.creator)}</span></Row>
          <Row label="Paid now / escrowed"><span className="font-terminal text-[22px]">{formatPct(payout.now)} / {formatPct(payout.escrowed)}</span></Row>
          <Row label="Rekt-back pool"><span className="font-terminal text-[22px]">{formatPct(split.rektBack)}</span></Row>
          <Row label="Interface / referral"><span className="font-terminal text-[22px]">{formatPct(split.interface)}</span></Row>
          <Row label="Protocol"><span className="font-terminal text-[22px]">{formatPct(split.protocol)}</span></Row>
          <Row label="Feed placement">{bonded ? 'Boosted' : 'Standard'}</Row>
          {bonded && <Row label="Creator bag vests"><span className="font-terminal text-[22px]">{vest} days</span></Row>}
          <Row label="Lobby window"><span className="font-terminal text-[22px]">{lobbyWindow}s</span></Row>
          {live ? (
            <>
              <Row label="Name bond (back at settle)" className="border-t-[3px] border-dashed border-hc-line pt-3">
                <span className="font-terminal text-[30px] leading-none text-hc-orange">{sol(hc.config.params.nameBond)} SOL</span>
              </Row>
              <Row label="Minimum Lobby"><span className="font-terminal text-[22px]">{sol(hc.config.params.minTotal)} SOL</span></Row>
              <Row label="Create fee"><span className="font-terminal text-[22px]">None (network fees and rent)</span></Row>
              {!hc.user ? (
                <PixelButton onClick={connect} className="mt-2 min-h-[56px] text-base">Connect wallet</PixelButton>
              ) : (
                <PixelButton onClick={createLaunch} disabled={sending || !!invalid} className="mt-2 min-h-[56px] text-base">
                  {sending ? 'Sending…' : 'Open the Lobby'}
                </PixelButton>
              )}
              {invalid && <p className="m-0 text-hc-warn">{invalid}</p>}
              {error && <p role="alert" className="m-0 text-hc-down break-words">{error}</p>}
              <p className="mt-1.5 mb-0 text-sm text-hc-muted">
                On {hc.cluster}. If the Lobby reaches the minimum, the curve opens and the name and ticker are yours for good. If not, the launch is cancelled and you can release them and take the bond back.
              </p>
            </>
          ) : (
            <>
              <Row label="You lock now" className="border-t-[3px] border-dashed border-hc-line pt-3">
                <span className="font-terminal text-[30px] leading-none text-hc-orange">{bonded ? bond : 0} SOL</span>
              </Row>
              <Row label="Create fee"><span className="font-terminal text-[22px]">[CREATE FEE]</span></Row>
              <PixelButton to="/lobby" className="mt-2 min-h-[56px] text-base">Open the Lobby</PixelButton>
              <p className="mt-1.5 mb-0 text-sm text-hc-muted">
                Liquidity lock and mint/freeze revocation at breakout are automatic for every token. You can't opt out. Preview only — nothing is deployed yet.
              </p>
            </>
          )}
        </Card>
      </div>
    </main>
  );
}

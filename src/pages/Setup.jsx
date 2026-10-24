import React, { useState } from 'react';
import * as anchor from '@anchor-lang/core';
import { Card, CardTitle, PixelButton, Row, Tag } from '../components/ui';
import { LAUNCH_PRESETS, PROGRAM_ID, adminAccounts, globalKeys, initializeAccounts, launchParams } from '../../scripts/herdcall-client.mjs';
import { programError, useHerdCall, usePoll } from '../chain/HerdCallContext';
import { explorerTx } from '../chain/config';
import { big, short, sol, tokens } from '../chain/format';

const { web3, BN } = anchor;
const INPUT = 'min-h-[48px] px-3 border-[3px] border-hc-line bg-hc-bg text-hc-cream font-terminal text-xl w-full';

const PARAM_ROWS = [
  ['v0', 'V0 (virtual SOL)', sol, 'Not decided: simulation example'],
  ['minTotal', 'Minimum Lobby', sol],
  ['nameBond', 'Name and ticker bond', sol],
  ['minDeposit', 'Minimum deposit', sol],
  ['maxTotal', 'Lobby cap', sol],
  ['feeBps', 'Trade fee', (v) => `${Number(v) / 100}%`],
  ['totalSupply', 'Total supply', tokens],
  ['curveSupply', 'Curve supply', tokens],
  ['lpSupply', 'LP supply', tokens],
  ['migrationFee', 'Breakout fee (to the caller)', sol, 'Working value'],
  ['settleWindow', 'Settle window', (v) => `${v} s`],
];

/** Upgrade authority from a ProgramData account: tag u32, slot u64, Option<Pubkey>. */
function upgradeAuthority(data) {
  if (!data || data.length < 45 || data[12] !== 1) return null;
  return new web3.PublicKey(data.subarray(13, 45));
}

function Result({ result }) {
  if (!result) return null;
  if (result.error) return <p role="alert" className="m-0 text-hc-down break-words">{result.error}</p>;
  return (
    <p className="m-0 text-hc-up break-words">
      Done. <a href={explorerTx(result.signature)} target="_blank" rel="noreferrer" className="underline">View transaction</a>
    </p>
  );
}

function useAction() {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState(null);
  const run = async (fn) => {
    setBusy(true);
    setResult(null);
    try {
      setResult({ signature: await fn() });
    } catch (e) {
      setResult({ error: programError(e?.logs) ?? e?.message ?? String(e) });
    } finally {
      setBusy(false);
    }
  };
  return { busy, result, run };
}

function KeyInput({ id, label, value, onChange, placeholder }) {
  return (
    <div className="flex flex-col gap-1.5">
      <label htmlFor={id} className="text-[15px] text-hc-muted">{label}</label>
      <input id={id} value={value} onChange={(e) => onChange(e.target.value.trim())} placeholder={placeholder} className={INPUT} spellCheck={false} />
    </div>
  );
}

const asKey = (s) => {
  try {
    return s ? new web3.PublicKey(s) : null;
  } catch {
    return null;
  }
};

function Initialize({ authority }) {
  const { program, user, send } = useHerdCall();
  const [preset, setPreset] = useState('devnet');
  const [dexConfig, setDexConfig] = useState('');
  const { busy, result, run } = useAction();
  const params = launchParams(preset);
  const dex = asKey(dexConfig);
  const isAuthority = user && authority && user.equals(authority);
  const init = () => run(() => {
    const p = Object.fromEntries(Object.entries(params).map(([k, v]) => [k, typeof v === 'bigint' ? new BN(v.toString()) : v]));
    return send(program.methods.initialize(p, dex).accountsStrict(initializeAccounts(user)));
  });
  return (
    <Card highlight className="flex flex-col gap-4">
      <CardTitle>Initialize</CardTitle>
      <p className="m-0 text-hc-body">
        One time. Only the program's upgrade authority can sign it. Launch parameters are fixed here: there is no instruction to change them later.
      </p>
      <div role="group" aria-label="Parameter preset" className="flex flex-wrap gap-2.5">
        {Object.keys(LAUNCH_PRESETS).map((name) => (
          <button
            key={name}
            type="button"
            aria-pressed={preset === name}
            onClick={() => setPreset(name)}
            className={`min-h-[44px] px-4 border-[3px] font-terminal text-2xl cursor-pointer ${preset === name ? 'border-hc-bg bg-hc-orange text-hc-bg' : 'border-hc-line bg-hc-bg text-hc-cream'}`}
          >
            {name}
          </button>
        ))}
      </div>
      {preset === 'devnet' && <p className="m-0 text-hc-warn">Test scale: the SOL side is scaled down. For devnet, local runs and a controlled smoke test only.</p>}
      <div className="flex flex-col gap-1.5 text-base">
        {PARAM_ROWS.map(([key, label, fmt, note]) => (
          <Row key={key} label={label}>
            <span className="font-terminal text-xl">{fmt(params[key])}</span>
            {note && <span className="ml-2 text-sm text-hc-warn">{note}</span>}
          </Row>
        ))}
      </div>
      <KeyInput id="dex-config" label="Meteora DAMM v2 config for breakouts" value={dexConfig} onChange={setDexConfig} placeholder="Config address" />
      <p className="m-0 text-sm text-hc-muted">
        Mainnet needs a private config whose pool creator is the HerdCall migrator PDA ({short(globalKeys().migrator)}). The program refuses a public config at breakout unless it was built with the devnet feature.
      </p>
      {!user && <p className="m-0 text-hc-muted">Connect the upgrade authority's wallet.</p>}
      {user && !isAuthority && <p className="m-0 text-hc-warn">The connected wallet is not the upgrade authority ({authority ? short(authority) : 'none'}): the program will refuse.</p>}
      <PixelButton onClick={init} disabled={busy || !user || !dex}>{busy ? 'Sending…' : 'Initialize'}</PixelButton>
      <Result result={result} />
    </Card>
  );
}

function AdminActions({ config }) {
  const { program, user, send } = useHerdCall();
  const [keeper, setKeeper] = useState('');
  const [dexConfig, setDexConfig] = useState('');
  const keeperAction = useAction();
  const dexAction = useAction();
  if (!user || !user.equals(config.admin)) {
    return (
      <Card className="flex flex-col gap-2">
        <CardTitle>Admin</CardTitle>
        <p className="m-0 text-hc-muted">Connect the admin wallet ({short(config.admin)}) to change the keeper or the DAMM config.</p>
      </Card>
    );
  }
  const k = asKey(keeper);
  const d = asKey(dexConfig);
  return (
    <Card className="flex flex-col gap-4">
      <CardTitle>Admin</CardTitle>
      <KeyInput id="keeper" label="Reinvest keeper (its own key, not the admin's)" value={keeper} onChange={setKeeper} placeholder="Keeper address" />
      {k && k.equals(config.admin) && <p className="m-0 text-hc-warn">That is the admin key. The keeper service refuses to run with it.</p>}
      <PixelButton onClick={() => keeperAction.run(() => send(program.methods.setReinvestKeeper(k).accountsStrict(adminAccounts(user))))} disabled={keeperAction.busy || !k}>
        Set keeper
      </PixelButton>
      <Result result={keeperAction.result} />
      <KeyInput id="new-dex" label="DAMM v2 config for future breakouts" value={dexConfig} onChange={setDexConfig} placeholder="Config address" />
      <PixelButton variant="secondary" onClick={() => dexAction.run(() => send(program.methods.setDexConfig(d).accountsStrict(adminAccounts(user))))} disabled={dexAction.busy || !d}>
        Set DAMM config
      </PixelButton>
      <Result result={dexAction.result} />
    </Card>
  );
}

function Revenue() {
  const { program, connection } = useHerdCall();
  const g = globalKeys();
  const { data } = usePoll(async () => {
    const [ledger, vault] = await Promise.all([program.account.revenueLedger.fetch(g.revenue), connection.getAccountInfo(g.protocol)]);
    const rent = await connection.getMinimumBalanceForRentExemption(vault.data.length);
    return { ledger, held: BigInt(vault.lamports - rent) };
  }, [program]);
  if (!data) return null;
  const r = data.ledger;
  const revenue = big(r.curve) + big(r.stampede) + big(r.lp);
  const counted = revenue + big(r.rounding) + big(r.other);
  return (
    <Card aria-label="Protocol revenue" className="flex flex-col gap-2">
      <CardTitle>Protocol revenue</CardTitle>
      <p className="m-0 text-sm text-hc-muted">Cumulative counters (HERDCALL.md §7.1). R = curve + Stampede + LP. Rounding and breakout leftovers are not revenue.</p>
      <Row label="Curve"><span className="font-terminal text-xl">{sol(r.curve, 9)} SOL</span></Row>
      <Row label="Stampede"><span className="font-terminal text-xl">{sol(r.stampede, 9)} SOL</span></Row>
      <Row label="LP fees"><span className="font-terminal text-xl">{sol(r.lp, 9)} SOL</span></Row>
      <Row label="Revenue" className="border-t-[3px] border-dashed border-hc-line pt-2"><span className="font-terminal text-2xl text-hc-orange">{sol(revenue, 9)} SOL</span></Row>
      <Row label="Rounding (not revenue)"><span className="font-terminal text-xl">{sol(r.rounding, 9)} SOL</span></Row>
      <Row label="Other (not revenue)"><span className="font-terminal text-xl">{sol(r.other, 9)} SOL</span></Row>
      <Row label="Protocol vault above rent"><span className="font-terminal text-xl">{sol(data.held, 9)} SOL</span></Row>
      {data.held !== counted && (
        <Row label="Sent outside the program (uncounted)"><span className="font-terminal text-xl text-hc-warn">{sol(data.held - counted, 9)} SOL</span></Row>
      )}
      <p className="m-0 text-sm text-hc-muted">No distribution instruction exists yet: the vault has no outflow.</p>
    </Card>
  );
}

export default function Setup() {
  const hc = useHerdCall();
  const { data: authority } = usePoll(async () => {
    if (!hc.connection) return null;
    const info = await hc.connection.getAccountInfo(hc.programData);
    return upgradeAuthority(info?.data);
  }, [hc.connection]);

  return (
    <main className="max-w-[1240px] mx-auto px-6 pt-8 pb-14 flex flex-col gap-6">
      <h1 className="m-0 font-pixel font-bold uppercase leading-[1.05] text-[clamp(30px,4vw,48px)]">Setup</h1>
      {!hc.enabled ? (
        <Card><p className="m-0 text-hc-body">No cluster configured. Set VITE_RPC_URL (and VITE_CLUSTER) when building, or open the app on localhost with a local validator.</p></Card>
      ) : (
        <div className="flex flex-wrap gap-6 items-start">
          <div className="flex-[1_1_360px] min-w-0 flex flex-col gap-6">
            <Card className="flex flex-col gap-2">
              <CardTitle>Cluster</CardTitle>
              <Row label="Cluster"><Tag>{hc.cluster}</Tag></Row>
              <Row label="RPC"><span className="break-all">{hc.rpcUrl}</span></Row>
              <Row label="Program"><span className="font-terminal text-xl" title={PROGRAM_ID.toBase58()}>{short(PROGRAM_ID)}</span></Row>
              <Row label="Deployed">{hc.loading ? '…' : hc.deployed ? 'Yes' : 'No'}</Row>
              <Row label="Upgrade authority"><span className="font-terminal text-xl">{authority ? short(authority) : hc.deployed ? 'none (immutable)' : '—'}</span></Row>
              <Row label="Initialized">{hc.config ? 'Yes' : 'No'}</Row>
              {hc.error && <p role="alert" className="m-0 text-hc-down">{hc.error}</p>}
            </Card>
            {hc.config && (
              <Card className="flex flex-col gap-2">
                <CardTitle>Config</CardTitle>
                <Row label="Admin"><span className="font-terminal text-xl" title={hc.config.admin.toBase58()}>{short(hc.config.admin)}</span></Row>
                <Row label="Reinvest keeper"><span className="font-terminal text-xl" title={hc.config.reinvestKeeper.toBase58()}>{short(hc.config.reinvestKeeper)}</span></Row>
                {hc.config.reinvestKeeper.equals(hc.config.admin) && <p className="m-0 text-hc-warn">The keeper is still the admin. Appoint a separate keeper key.</p>}
                <Row label="DAMM config"><span className="font-terminal text-xl" title={hc.config.dexConfig.toBase58()}>{short(hc.config.dexConfig)}</span></Row>
                {PARAM_ROWS.map(([key, label, fmt]) => (
                  <Row key={key} label={label}><span className="font-terminal text-xl">{fmt(hc.config.params[key])}</span></Row>
                ))}
              </Card>
            )}
          </div>
          <div className="flex-[1_1_420px] min-w-0 flex flex-col gap-6">
            {hc.deployed && !hc.config && <Initialize authority={authority} />}
            {hc.config && <AdminActions config={hc.config} />}
            {hc.config && <Revenue />}
          </div>
        </div>
      )}
    </main>
  );
}

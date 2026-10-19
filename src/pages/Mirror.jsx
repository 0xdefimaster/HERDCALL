import React, { useEffect, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { Card, CardTitle, PixelButton, Tag } from '../components/ui';
import { SAMPLE_TICKER, sampleMirror } from '../data/mirrorSample';
import { shareText } from '../lib/mirror';
import { cardFileName, drawMirrorCard } from '../lib/mirrorCard';
import { DEFAULT_RPC, LIMITS, runMirror } from '../lib/mirrorRpc';
import { shortAddress, useWallet } from '../wallet/WalletContext';

const ADDRESS = /^[1-9A-HJ-NP-Za-km-z]{32,44}$/;
const SIGNATURE = /^[1-9A-HJ-NP-Za-km-z]{64,90}$/;

const ERRORS = {
  'too-active': `This token has more than ${(LIMITS.signaturePages * 1000).toLocaleString('en-US')} transactions. Browser mode only reads tokens with a shorter history for now.`,
  'no-history': 'No transactions found for this token address. Check that it is the token mint, not a pool or wallet.',
  'no-trades': 'Could not read any buys from the first transactions of this token.',
  'no-user-buy': 'No buy from this wallet was found. If you sold and closed the token account, paste your buy transaction under Advanced.',
};

const inputClass = 'w-full min-h-[48px] px-3 border-[3px] border-hc-line bg-hc-bg text-hc-cream font-terminal text-xl focus:border-hc-orange outline-none';
const sol = (v) => v.toLocaleString('en-US', { maximumFractionDigits: 2, minimumFractionDigits: 2 });

function progressText(p) {
  if (!p) return '';
  if (p.step === 'signatures') return 'Reading the token’s transaction history…';
  if (p.step === 'early') return `Decoding the first transactions… ${p.done ?? 0}/${p.total ?? LIMITS.earlyTransactions}`;
  return 'Finding your buy…';
}

export default function Mirror() {
  const [params] = useSearchParams();
  const { publicKey } = useWallet();
  const [mint, setMint] = useState(params.get('mint') ?? '');
  const [wallet, setWallet] = useState(params.get('wallet') ?? '');
  const [ticker, setTicker] = useState('');
  const [buySignature, setBuySignature] = useState('');
  const [rpcUrl, setRpcUrl] = useState('');
  const [progress, setProgress] = useState(null);
  const [error, setError] = useState(null);
  const [result, setResult] = useState(null);

  const busy = progress !== null;
  const walletValue = wallet || publicKey || '';

  const run = async () => {
    setError(null);
    setResult(null);
    if (!ADDRESS.test(mint.trim())) return setError('Enter a valid token mint address.');
    if (!ADDRESS.test(walletValue.trim())) return setError('Enter your wallet address or connect your wallet.');
    if (buySignature && !SIGNATURE.test(buySignature.trim())) return setError('That does not look like a transaction signature.');
    setProgress({ step: 'signatures' });
    try {
      const r = await runMirror({
        mint: mint.trim(),
        wallet: walletValue.trim(),
        buySignature: buySignature.trim() || undefined,
        rpcUrl: rpcUrl.trim() || undefined,
        onProgress: setProgress,
      });
      if (r.error) setError(ERRORS[r.error] ?? 'Something went wrong.');
      else setResult({ ...r, sample: false, ticker: ticker.trim().replace(/^\$/, '') || null, mint: mint.trim() });
    } catch (e) {
      setError(`The RPC did not answer (${e.message}). Public endpoints rate-limit; try again or use your own RPC URL under Advanced.`);
    } finally {
      setProgress(null);
    }
  };

  const runSample = () => {
    setError(null);
    setResult({ ...sampleMirror(), sample: true, ticker: SAMPLE_TICKER, mint: null });
  };

  return (
    <main className="max-w-[1240px] mx-auto px-4 sm:px-6 pt-8 pb-14 flex flex-col gap-6">
      <Card aria-label="Mirror Mode" highlight className="p-6 flex flex-wrap gap-6 items-center">
        <img
          src="/herdcall-hero.webp"
          alt="HerdCall mascot"
          className="shrink-0 w-[96px] h-[96px] object-cover object-[50%_28%] border-[3px] border-hc-line [image-rendering:pixelated]"
        />
        <div className="flex-[1_1_320px] min-w-0">
          <div className="flex flex-wrap items-center gap-2.5">
            <h1 className="m-0 font-pixel font-bold text-[32px] leading-none uppercase">Mirror Mode</h1>
            <Tag tone="outline">Read-only</Tag>
          </div>
          <p className="mt-3 mb-0 text-hc-body max-w-[680px]">
            Got sniped on another launchpad? Paste the token and your wallet. We read public chain data and show who got in before you, what they paid,
            and what one price for everyone would have looked like. No wallet signature, no transaction.
          </p>
        </div>
      </Card>

      <div className="flex flex-wrap gap-6 items-start">
        <Card aria-label="Run the mirror" className="flex-[1_1_340px] min-w-0 flex flex-col gap-4">
          <CardTitle className="text-[20px]">Run the mirror</CardTitle>
          <label className="flex flex-col gap-1.5 text-[15px] text-hc-muted">
            Token mint address
            <input className={inputClass} value={mint} onChange={(e) => setMint(e.target.value)} placeholder="Token address" spellCheck={false} autoComplete="off" />
          </label>
          <label className="flex flex-col gap-1.5 text-[15px] text-hc-muted">
            Your wallet
            <input
              className={inputClass}
              value={wallet}
              onChange={(e) => setWallet(e.target.value)}
              placeholder={publicKey ? `Connected: ${shortAddress(publicKey)}` : 'Wallet address'}
              spellCheck={false}
              autoComplete="off"
            />
          </label>
          <label className="flex flex-col gap-1.5 text-[15px] text-hc-muted">
            Ticker for your card (optional)
            <input className={inputClass} value={ticker} onChange={(e) => setTicker(e.target.value)} placeholder="BONK" maxLength={12} autoComplete="off" />
          </label>
          <details className="border-[3px] border-dashed border-hc-line p-3">
            <summary className="cursor-pointer font-pixel text-xs text-hc-muted">Advanced</summary>
            <div className="mt-3 flex flex-col gap-3">
              <label className="flex flex-col gap-1.5 text-[15px] text-hc-muted">
                Your buy transaction (if you closed the token account)
                <input className={inputClass} value={buySignature} onChange={(e) => setBuySignature(e.target.value)} placeholder="Transaction signature" spellCheck={false} autoComplete="off" />
              </label>
              <label className="flex flex-col gap-1.5 text-[15px] text-hc-muted">
                RPC URL
                <input className={inputClass} value={rpcUrl} onChange={(e) => setRpcUrl(e.target.value)} placeholder={DEFAULT_RPC} spellCheck={false} autoComplete="off" />
              </label>
              <p className="m-0 text-sm text-hc-muted">Public RPCs rate-limit. For busy tokens, use your own RPC URL. It stays in this page and is only used for read calls.</p>
            </div>
          </details>
          <div className="flex flex-wrap gap-3">
            <PixelButton onClick={run} disabled={busy} className="flex-1 min-h-[52px]">{busy ? 'Reading…' : 'Run the mirror'}</PixelButton>
            <PixelButton variant="secondary" onClick={runSample} disabled={busy} className="flex-1 min-h-[52px]">Try sample data</PixelButton>
          </div>
          <div aria-live="polite" className="min-h-[24px] text-hc-body">{progressText(progress)}</div>
          {error && <div role="alert" className="p-3 border-[3px] border-hc-down text-hc-body">{error}</div>}
        </Card>

        <div className="flex-[2_1_480px] min-w-0">
          {result ? <MirrorResult result={result} /> : <EmptyState />}
        </div>
      </div>
    </main>
  );
}

function EmptyState() {
  return (
    <Card aria-label="How it works" className="flex flex-col gap-3 text-hc-body">
      <CardTitle className="text-[20px]">What you will see</CardTitle>
      <ul className="m-0 pl-5 flex flex-col gap-2">
        <li>How many wallets bought before you, and how many of them in the very first block.</li>
        <li>Your price compared to their average price.</li>
        <li>The HerdCall version: everyone who wanted in by your buy enters at one price, in one Lobby.</li>
        <li>A card you can share.</li>
      </ul>
      <p className="m-0 text-sm text-hc-muted">Not sure? Press “Try sample data” to see a made-up launch.</p>
    </Card>
  );
}

function Stat({ label, value, tone = '' }) {
  return (
    <div className="bg-hc-bg border-[3px] border-hc-line p-3 min-w-0">
      <div className="text-sm text-hc-muted">{label}</div>
      <div className={`font-terminal text-[32px] leading-none break-words ${tone}`}>{value}</div>
    </div>
  );
}

function MirrorResult({ result }) {
  const [copied, setCopied] = useState(false);
  const [card, setCard] = useState(null); // { url, blob }
  const text = shareText(result, result.ticker);

  useEffect(() => {
    let cancelled = false;
    let url = null;
    drawMirrorCard(result).then((canvas) =>
      canvas.toBlob((blob) => {
        if (cancelled || !blob) return;
        url = URL.createObjectURL(blob);
        setCard({ url, blob });
      }, 'image/png'),
    );
    return () => {
      cancelled = true;
      if (url) URL.revokeObjectURL(url);
    };
  }, [result]);

  const file = card && typeof File !== 'undefined' ? new File([card.blob], cardFileName(result), { type: 'image/png' }) : null;
  const canShareFile = !!(file && navigator.canShare?.({ files: [file] }));
  const shareImage = async () => {
    try {
      await navigator.share({ files: [file], text });
    } catch {
      // cancelled or not allowed: nothing to do
    }
  };
  const intent = `https://twitter.com/intent/tweet?text=${encodeURIComponent(text)}`;
  const gain = result.herd?.tokenGain;
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      setCopied(false);
    }
  };
  const lowerBound = result.partial ? 'at least ' : '';
  return (
    <Card aria-label="Mirror result" highlight className="flex flex-col gap-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="font-pixel text-[13px] text-hc-muted">
          {result.ticker ? `$${result.ticker}` : shortAddress(result.mint)} · you: {shortAddress(result.wallet)}
        </div>
        {result.sample && <Tag tone="orange">Sample data · not a real token</Tag>}
      </div>

      <div>
        <div className="font-terminal leading-[0.85] text-hc-orange text-[clamp(80px,12vw,140px)]">{result.walletsBefore}</div>
        <div className="mt-2 font-pixel font-bold text-[20px] uppercase">
          {result.walletsBefore === 1 ? 'wallet' : 'wallets'} got in before you{result.partial ? ' (at least)' : ''}
        </div>
        <p className="mt-2 mb-0 text-hc-body">
          {result.firstBlockWallets > 0 && <>{result.firstBlockWallets} of them in the very first block. </>}
          You bought {result.slotsAfterLaunch} slot{result.slotsAfterLaunch === 1 ? '' : 's'} after launch
          {result.secondsAfterLaunch != null ? ` (about ${result.secondsAfterLaunch}s)` : ''}.
        </p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
        <Stat label="Your price vs. theirs" value={result.priceMultiple ? `${result.priceMultiple.toFixed(2)}x` : '—'} tone={result.priceMultiple > 1 ? 'text-hc-down' : ''} />
        <Stat label="SOL in before you" value={sol(result.beforeSol)} />
        <Stat label="Transactions before yours" value={result.signaturesBefore != null ? `${lowerBound}${result.signaturesBefore}` : '—'} />
      </div>

      {result.herd && (
        <div className="p-4 border-[3px] border-hc-orange bg-hc-bg flex flex-col gap-2">
          <div className="font-pixel text-[13px] text-hc-orange">The HerdCall version (estimate)</div>
          <p className="m-0 text-hc-body">
            Everyone who bought up to your buy ({sol(result.herd.pool)} SOL) sits in one Lobby. When the bell rings, all of you enter at{' '}
            <strong className="text-hc-cream">{sol(result.herd.mcapSol)} SOL mcap</strong>, the same price, and the curve opens there.
            {gain != null && (
              <>
                {' '}You would hold <strong className={gain >= 0 ? 'text-hc-up' : 'text-hc-down'}>{gain >= 0 ? '+' : ''}{(gain * 100).toFixed(0)}%</strong>{' '}
                {gain >= 0 ? 'more' : 'fewer'} tokens for the same SOL.
              </>
            )}
          </p>
        </div>
      )}

      {result.before.length > 0 && (
        <div className="relative overflow-x-auto">
          <table className="w-full border-collapse text-left">
            <caption className="text-left font-pixel text-[13px] text-hc-muted pb-2">Who got in before you{result.before.length > 10 ? ' (first 10)' : ''}</caption>
            <thead>
              <tr className="font-pixel text-xs text-hc-muted">
                <th className="py-2 pr-3 font-normal">Wallet</th>
                <th className="py-2 pr-3 font-normal">Slot</th>
                <th className="py-2 pr-3 font-normal text-right">SOL</th>
                <th className="py-2 font-normal text-right">You paid</th>
              </tr>
            </thead>
            <tbody className="font-terminal text-[22px]">
              {result.before.slice(0, 10).map((w) => (
                <tr key={w.wallet} className="border-t-[3px] border-hc-line">
                  <td className="py-1.5 pr-3">{shortAddress(w.wallet)}</td>
                  <td className="py-1.5 pr-3 text-hc-muted">+{w.slot - result.launchSlot}</td>
                  <td className="py-1.5 pr-3 text-right">{sol(w.sol)}</td>
                  <td className="py-1.5 text-right">{(result.userPrice / (w.sol / w.tokens)).toFixed(2)}x</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {card && (
        <figure className="m-0 flex flex-col gap-2">
          <img src={card.url} alt={`Mirror card: ${text}`} className="block w-full h-auto border-[3px] border-hc-line" />
          <figcaption className="text-sm text-hc-muted">Your card. Download it and attach it to your post.</figcaption>
        </figure>
      )}

      <div className="flex flex-wrap gap-3">
        <PixelButton onClick={() => window.open(intent, '_blank', 'noopener,noreferrer')} className="flex-1">Share on X</PixelButton>
        {card && (
          <a
            href={card.url}
            download={cardFileName(result)}
            className="flex-1 inline-flex items-center justify-center min-h-[44px] px-5 border-[3px] bg-transparent text-hc-cream border-hc-cream shadow-px-orange font-pixel font-bold text-[15px] no-underline hover:text-hc-cream"
          >
            Download card
          </a>
        )}
        {canShareFile && <PixelButton variant="secondary" onClick={shareImage} className="flex-1">Share image</PixelButton>}
        <PixelButton variant="secondary" onClick={copy} className="flex-1">{copied ? 'Copied' : 'Copy text'}</PixelButton>
      </div>

      <div className="border-t-[3px] border-dashed border-hc-line pt-3 flex flex-col gap-1.5 text-sm text-hc-muted">
        <p className="m-0">“Before you” means wallets that bought in an earlier slot, or earlier in the same slot.</p>
        <p className="m-0">
          Estimates from public chain data. SOL amounts include platform fees and account rent. The HerdCall version uses HerdCall&apos;s curve and Stampede
          formula; it is an illustration, not a promise of profit. Wallets are shown by shortened address; no one is labelled.
          {result.partial && ' You bought after the first transactions we decode, so counts are a lower bound.'}
        </p>
      </div>
    </Card>
  );
}

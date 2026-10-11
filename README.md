# HerdCall

**No snipers. Just the herd.** Solana community launchpad where every launch opens with a
countdown Lobby and the whole herd enters at one price.

Product design: [`document/HERDCALL.md`](document/HERDCALL.md)

## Frontend

```bash
npm install                      # .npmrc sets legacy-peer-deps (hardhat 3 / toolbox 6 conflict)
npm run dev                      # http://localhost:3000
npm run build
```

Pages: `/` landing, `/lobby`, `/token`, `/create`, `/rekt-back`, `/takeover`.
All data is mock (`src/data/mock.js`); fee splits live in `src/lib/fees.js`.
Wallet: minimal Phantom connection (`src/wallet/WalletContext.jsx`), no transactions are sent yet.

## Legacy

`contract/`, `scripts/` and `hardhat.config.js` are the old PyrePad EVM contracts. The on-chain
program will be rewritten for Solana with Anchor.

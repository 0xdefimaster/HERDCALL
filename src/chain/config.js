// Which cluster the web client talks to. VITE_RPC_URL decides; on localhost it defaults to a local validator
// (scripts/local-validator.sh). Without either, on-chain features stay off and the pages show previews only.
const local = typeof window !== 'undefined' && ['localhost', '127.0.0.1'].includes(window.location.hostname);

export const RPC_URL = import.meta.env.VITE_RPC_URL || (local ? 'http://127.0.0.1:8899' : '');
export const CLUSTER = import.meta.env.VITE_CLUSTER || (RPC_URL.includes('127.0.0.1') ? 'localnet' : 'custom');
// Optional: the owner of a registered trading interface that receives the interface share of curve fees. Without one
// the share goes to the protocol, as the program does when no interface is passed.
export const INTERFACE_OWNER = import.meta.env.VITE_INTERFACE_OWNER || '';
export const CHAIN_ENABLED = Boolean(RPC_URL);

export function explorerTx(signature) {
  const cluster = CLUSTER === 'mainnet' ? '' : CLUSTER === 'devnet' ? '?cluster=devnet' : `?cluster=custom&customUrl=${encodeURIComponent(RPC_URL)}`;
  return `https://explorer.solana.com/tx/${signature}${cluster}`;
}

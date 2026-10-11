import React, { createContext, useCallback, useContext, useEffect, useState } from 'react';

// Minimal Phantom (Solana) connection. Swap for @solana/wallet-adapter when
// more wallets and on-chain calls are needed.
const WalletContext = createContext(null);

function getProvider() {
  if (typeof window === 'undefined') return null;
  const provider = window.phantom?.solana ?? window.solana;
  return provider?.isPhantom ? provider : null;
}

function unsubscribe(provider, event, handler) {
  if (provider.removeListener) provider.removeListener(event, handler);
  else if (provider.off) provider.off(event, handler);
}

export function shortAddress(address) {
  return address ? `${address.slice(0, 4)}…${address.slice(-4)}` : '';
}

export function WalletProvider({ children }) {
  const [publicKey, setPublicKey] = useState(null);
  const [error, setError] = useState(null);

  useEffect(() => {
    const provider = getProvider();
    if (!provider) return undefined;

    provider
      .connect({ onlyIfTrusted: true })
      .then((res) => setPublicKey(res.publicKey.toString()))
      .catch(() => {});

    const onDisconnect = () => setPublicKey(null);
    const onAccountChanged = (key) => setPublicKey(key ? key.toString() : null);
    provider.on?.('disconnect', onDisconnect);
    provider.on?.('accountChanged', onAccountChanged);

    return () => {
      unsubscribe(provider, 'disconnect', onDisconnect);
      unsubscribe(provider, 'accountChanged', onAccountChanged);
    };
  }, []);

  const connect = useCallback(async () => {
    const provider = getProvider();
    if (!provider) {
      window.open('https://phantom.app/', '_blank', 'noopener,noreferrer');
      return;
    }
    try {
      const res = await provider.connect();
      setPublicKey(res.publicKey.toString());
      setError(null);
    } catch (err) {
      setError(err?.message ?? 'Connection rejected');
    }
  }, []);

  const disconnect = useCallback(async () => {
    await getProvider()?.disconnect();
    setPublicKey(null);
  }, []);

  return (
    <WalletContext.Provider value={{ publicKey, error, connect, disconnect }}>
      {children}
    </WalletContext.Provider>
  );
}

export function useWallet() {
  const context = useContext(WalletContext);
  if (!context) throw new Error('useWallet must be used within WalletProvider');
  return context;
}

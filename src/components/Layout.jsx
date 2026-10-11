import React from 'react';
import { Link, NavLink, Outlet } from 'react-router-dom';
import { HLogo } from './icons';
import { PixelButton } from './ui';
import { shortAddress, useWallet } from '../wallet/WalletContext';

const NAV_LINKS = [
  { to: '/lobby', label: 'Lobby' },
  { to: '/token', label: 'Tokens' },
  { to: '/takeover', label: 'Takeovers' },
  { to: '/rekt-back', label: 'Rekt-back' },
];

function WalletButton() {
  const { publicKey, connect, disconnect } = useWallet();
  if (publicKey) {
    return (
      <PixelButton variant="ghost" onClick={disconnect} title="Disconnect wallet">
        {shortAddress(publicKey)}
      </PixelButton>
    );
  }
  return (
    <PixelButton variant="ghost" onClick={connect}>
      Connect wallet
    </PixelButton>
  );
}

export default function Layout() {
  return (
    <div className="min-h-screen bg-hc-bg text-hc-cream font-body text-[17px] leading-normal">
      <header className="border-b-[3px] border-hc-line">
        <nav className="max-w-[1240px] mx-auto px-6 py-4 flex flex-wrap items-center justify-between gap-4">
          <Link to="/" className="flex items-center gap-3 no-underline">
            <HLogo size={32} />
            <span className="font-pixel font-bold text-2xl text-hc-orange">HERDCALL</span>
          </Link>
          <div className="flex flex-wrap items-center gap-x-5 gap-y-2 font-pixel text-[13px]">
            {NAV_LINKS.map((link) => (
              <NavLink
                key={link.to}
                to={link.to}
                className={({ isActive }) =>
                  `no-underline ${isActive ? 'text-hc-orange font-bold' : 'text-hc-cream hover:text-hc-orange'}`
                }
              >
                {link.label}
              </NavLink>
            ))}
            <PixelButton to="/create" className="text-[13px]">
              Launch a token
            </PixelButton>
            <WalletButton />
          </div>
        </nav>
      </header>

      <Outlet />

      <footer className="border-t-[3px] border-hc-line">
        <div className="max-w-[1240px] mx-auto px-6 py-7 flex flex-wrap justify-between gap-3 text-[15px] text-hc-muted">
          <span>HerdCall · Built on Solana · Official site: herdcall.fun only</span>
          <span>Product concept. Mechanism security and feasibility to be independently verified. Memecoins are high risk.</span>
        </div>
      </footer>
    </div>
  );
}

import React from 'react';
import { Link, NavLink, Outlet, useLocation, useNavigate, useSearchParams } from 'react-router-dom';
import { HLogo } from './icons';
import { PixelButton } from './ui';
import { shortAddress, useWallet } from '../wallet/WalletContext';
import { CHAIN_ENABLED, CLUSTER } from '../chain/config';

const NAV_LINKS = [
  { to: '/', label: 'Board', end: true },
  { to: '/lobby', label: 'Lobby' },
  { to: '/create', label: 'Launch' },
  { to: '/takeover', label: 'Takeovers' },
  { to: '/rekt-back', label: 'Rekt-back' },
  { to: '/mirror', label: 'Mirror' },
  { to: '/how-it-works', label: 'How it works' },
  { to: '/setup', label: 'Setup' },
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
  return <PixelButton onClick={connect} className="text-[13px]">Connect wallet</PixelButton>;
}

function Logo() {
  return (
    <Link to="/" className="flex items-center gap-3 no-underline">
      <HLogo size={30} />
      <span className="font-pixel font-bold text-[22px] text-hc-orange">HERDCALL</span>
    </Link>
  );
}

function navClass({ isActive }) {
  return `block px-3 py-2.5 font-pixel text-[13px] no-underline border-[3px] ${
    isActive ? 'bg-hc-orange text-hc-bg border-hc-bg font-bold hover:text-hc-bg' : 'text-hc-cream border-transparent hover:border-hc-line hover:text-hc-orange'
  }`;
}

function Sidebar() {
  return (
    <aside className="hidden lg:flex flex-col w-[220px] shrink-0 border-r-[3px] border-hc-line sticky top-0 h-screen p-5 gap-6 overflow-y-auto">
      <Logo />
      <nav aria-label="Main" className="flex flex-col gap-1">
        {NAV_LINKS.map((link) => (
          <NavLink key={link.to} to={link.to} end={link.end} className={navClass}>
            {link.label}
          </NavLink>
        ))}
      </nav>
      <figure className="m-0 border-[3px] border-hc-orange shadow-px-navy bg-hc-surface">
        <img
          src="/herdcall-hero.webp"
          alt="HerdCall mascot, a pixel-art bull in a cap and shades leading the herd"
          className="block w-full h-auto aspect-square object-cover [image-rendering:pixelated]"
        />
        <figcaption className="p-3 font-pixel text-xs leading-relaxed">
          No snipers.
          <br />
          <span className="text-hc-orange">Just the herd.</span>
        </figcaption>
      </figure>
      <div className="mt-auto flex items-center justify-between font-pixel text-xs text-hc-muted">
        <span>Solana</span>
        <span className="inline-flex items-center gap-2 text-hc-up"><span className="w-2 h-2 bg-hc-up" />{CHAIN_ENABLED ? CLUSTER : 'Preview'}</span>
      </div>
    </aside>
  );
}

function SearchBox() {
  const navigate = useNavigate();
  const location = useLocation();
  const [params, setParams] = useSearchParams();
  const onBoard = location.pathname === '/';
  const value = onBoard ? params.get('q') ?? '' : '';

  const onChange = (e) => {
    const q = e.target.value;
    if (onBoard) setParams(q ? { q } : {}, { replace: true });
    else navigate(q ? `/?q=${encodeURIComponent(q)}` : '/');
  };

  return (
    <div className="flex-1 min-w-[180px] max-w-[460px]">
      <label htmlFor="global-search" className="sr-only">Search tokens</label>
      <input
        id="global-search"
        type="search"
        value={value}
        onChange={onChange}
        placeholder="Search token or ticker"
        className="w-full min-h-[44px] px-3 border-[3px] border-hc-line bg-hc-surface text-hc-cream placeholder:text-hc-muted focus:border-hc-orange outline-none"
      />
    </div>
  );
}

function TopBar() {
  return (
    <header className="border-b-[3px] border-hc-line">
      <div className="px-4 lg:px-6 py-3 flex flex-wrap items-center gap-3">
        <div className="lg:hidden">
          <Logo />
        </div>
        <SearchBox />
        <div className="ml-auto flex flex-wrap items-center gap-3">
          <PixelButton to="/create" variant="secondary" className="text-[13px] hidden sm:inline-flex">Launch a token</PixelButton>
          <WalletButton />
        </div>
      </div>
      <nav aria-label="Main" className="lg:hidden px-4 pb-3 flex gap-1 overflow-x-auto">
        {NAV_LINKS.map((link) => (
          <NavLink key={link.to} to={link.to} end={link.end} className={(s) => `${navClass(s)} whitespace-nowrap`}>
            {link.label}
          </NavLink>
        ))}
      </nav>
    </header>
  );
}

export default function Layout() {
  return (
    <div className="min-h-screen bg-hc-bg text-hc-cream font-body text-[17px] leading-normal lg:flex">
      <Sidebar />
      <div className="flex-1 min-w-0 flex flex-col">
        <TopBar />
        <div className="flex-1">
          <Outlet />
        </div>
        <footer className="border-t-[3px] border-hc-line">
          <div className="px-4 lg:px-6 py-6 flex flex-wrap justify-between gap-3 text-[15px] text-hc-muted">
            <span>HerdCall · Built on Solana · Official site: herdcall.fun only</span>
            <span>Product concept. Mechanism security and feasibility to be independently verified. Memecoins are high risk.</span>
          </div>
        </footer>
      </div>
    </div>
  );
}

import React from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { WalletProvider } from './wallet/WalletContext';
import { HerdCallProvider } from './chain/HerdCallContext';
import Layout from './components/Layout';
import Board from './pages/Board';
import Landing from './pages/Landing';
import Lobby from './pages/Lobby';
import Token from './pages/Token';
import Create from './pages/Create';
import Rekt from './pages/Rekt';
import Takeover from './pages/Takeover';
import Mirror from './pages/Mirror';
import Setup from './pages/Setup';
import LiveLaunch from './pages/LiveLaunch';

function App() {
  return (
    <WalletProvider>
      <HerdCallProvider>
      <BrowserRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route path="/" element={<Board />} />
            <Route path="/how-it-works" element={<Landing />} />
            <Route path="/lobby" element={<Lobby />} />
            <Route path="/lobby/:symbol" element={<Lobby />} />
            <Route path="/token" element={<Token />} />
            <Route path="/token/:symbol" element={<Token />} />
            <Route path="/create" element={<Create />} />
            <Route path="/rekt-back" element={<Rekt />} />
            <Route path="/takeover" element={<Takeover />} />
            <Route path="/mirror" element={<Mirror />} />
            <Route path="/launch/:mint" element={<LiveLaunch />} />
            <Route path="/setup" element={<Setup />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Route>
        </Routes>
      </BrowserRouter>
      </HerdCallProvider>
    </WalletProvider>
  );
}

export default App;

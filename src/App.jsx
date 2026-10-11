import React from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { WalletProvider } from './wallet/WalletContext';
import Layout from './components/Layout';
import Landing from './pages/Landing';
import Lobby from './pages/Lobby';
import Token from './pages/Token';
import Create from './pages/Create';
import Rekt from './pages/Rekt';
import Takeover from './pages/Takeover';

function App() {
  return (
    <WalletProvider>
      <BrowserRouter>
        <Routes>
          <Route element={<Layout />}>
            <Route path="/" element={<Landing />} />
            <Route path="/lobby" element={<Lobby />} />
            <Route path="/token" element={<Token />} />
            <Route path="/create" element={<Create />} />
            <Route path="/rekt-back" element={<Rekt />} />
            <Route path="/takeover" element={<Takeover />} />
            <Route path="*" element={<Navigate to="/" replace />} />
          </Route>
        </Routes>
      </BrowserRouter>
    </WalletProvider>
  );
}

export default App;

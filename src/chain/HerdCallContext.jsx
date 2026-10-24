import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';
import * as anchor from '@anchor-lang/core';
import idl from '../../programs/idl/herdcall.json';
import { PROGRAM_ID, globalKeys, programDataKey } from '../../scripts/herdcall-client.mjs';
import { useWallet } from '../wallet/WalletContext';
import { CHAIN_ENABLED, CLUSTER, RPC_URL } from './config';

const { web3 } = anchor;
const HerdCallContext = createContext(null);

// Read-only wallet for anchor's provider: fetching accounts and building instructions need no signer.
const readOnlyWallet = (publicKey) => ({
  publicKey,
  signTransaction: () => Promise.reject(new Error('read only')),
  signAllTransactions: () => Promise.reject(new Error('read only')),
});

/** The program's own error message from simulation logs, if any. */
export function programError(logs = []) {
  const line = logs.find((l) => l.includes('Error Message:')) ?? logs.find((l) => /failed|insufficient/i.test(l));
  if (!line) return null;
  const m = /Error Code: (\w+)\. Error Number: \d+\. Error Message: (.*)$/.exec(line);
  return m ? `${m[2].replace(/\.$/, '')} (${m[1]})` : line.replace(/^Program log: /, '');
}

export function HerdCallProvider({ children }) {
  const wallet = useWallet();
  const connection = useMemo(() => (CHAIN_ENABLED ? new web3.Connection(RPC_URL, 'confirmed') : null), []);
  const user = useMemo(() => (wallet.publicKey ? new web3.PublicKey(wallet.publicKey) : null), [wallet.publicKey]);
  const program = useMemo(() => {
    if (!connection) return null;
    const provider = new anchor.AnchorProvider(connection, readOnlyWallet(user ?? web3.PublicKey.default), { commitment: 'confirmed' });
    return new anchor.Program(idl, provider);
  }, [connection, user]);

  const [status, setStatus] = useState({ loading: CHAIN_ENABLED, deployed: false, config: null, error: null });
  const [tick, setTick] = useState(0);
  const refresh = useCallback(() => setTick((t) => t + 1), []);

  useEffect(() => {
    if (!program) return undefined;
    let live = true;
    (async () => {
      try {
        const info = await connection.getAccountInfo(PROGRAM_ID);
        const config = info?.executable ? await program.account.config.fetchNullable(globalKeys().config) : null;
        if (live) setStatus({ loading: false, deployed: Boolean(info?.executable), config, error: null });
      } catch (e) {
        if (live) setStatus({ loading: false, deployed: false, config: null, error: `Cannot reach ${RPC_URL}: ${e.message ?? e}` });
      }
    })();
    return () => { live = false; };
  }, [program, connection, tick]);

  /**
   * Builds the transaction from an anchor method builder, simulates it (so a program error shows before the wallet
   * opens), has the wallet sign it and waits for confirmation. `signers`: extra keypairs, such as a new mint.
   */
  const send = useCallback(async (builder, { signers = [], computeUnits } = {}) => {
    if (!user) throw new Error('Connect a wallet first');
    if (computeUnits) builder = builder.preInstructions([web3.ComputeBudgetProgram.setComputeUnitLimit({ units: computeUnits })]);
    const tx = await builder.transaction();
    const { blockhash, lastValidBlockHeight } = await connection.getLatestBlockhash('confirmed');
    tx.feePayer = user;
    tx.recentBlockhash = blockhash;
    const sim = await connection.simulateTransaction(tx);
    if (sim.value.err) {
      throw new Error(programError(sim.value.logs ?? []) ?? `Simulation failed: ${JSON.stringify(sim.value.err)}`);
    }
    if (signers.length) tx.partialSign(...signers);
    const signed = await wallet.signTransaction(tx);
    const signature = await connection.sendRawTransaction(signed.serialize());
    const res = await connection.confirmTransaction({ signature, blockhash, lastValidBlockHeight }, 'confirmed');
    if (res.value.err) throw new Error(`Transaction failed: ${JSON.stringify(res.value.err)}`);
    refresh();
    return signature;
  }, [connection, user, wallet, refresh]);

  const value = { enabled: CHAIN_ENABLED, cluster: CLUSTER, rpcUrl: RPC_URL, connection, program, user, send, refresh, tick, programData: programDataKey(), ...status };
  return <HerdCallContext.Provider value={value}>{children}</HerdCallContext.Provider>;
}

export function useHerdCall() {
  const c = useContext(HerdCallContext);
  if (!c) throw new Error('useHerdCall must be used within HerdCallProvider');
  return c;
}

/** Polls a loader every `ms` and when the chain context refreshes (after a transaction). */
export function usePoll(load, deps, ms = 4000) {
  const { tick } = useHerdCall();
  const [state, setState] = useState({ data: null, error: null, loading: true });
  useEffect(() => {
    let live = true;
    const run = () => load().then(
      (data) => live && setState({ data, error: null, loading: false }),
      (error) => live && setState((s) => ({ ...s, error, loading: false })),
    );
    run();
    const id = setInterval(run, ms);
    return () => { live = false; clearInterval(id); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, tick]);
  return state;
}

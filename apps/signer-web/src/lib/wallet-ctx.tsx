/**
 * Global wallet context. Wraps Phantom (the only adapter we ship) and exposes
 * a typed connect / disconnect / signing API to every page.
 *
 * Usage:
 *   const { wallet, connect, disconnect, connecting, error } = useWallet();
 *   // For Solana transfers — Phantom's signMessage refuses tx-shaped bytes:
 *   if (wallet) { const sig = await wallet.signTransaction(tx); }
 *   // For arbitrary non-Solana payloads:
 *   if (wallet) { const sig = await wallet.signMessageBytes(bytes); }
 *
 * Design notes:
 *  - We only auto-reconnect via Phantom's `onlyIfTrusted: true` on mount, so
 *    the user keeps the same session across tabs.
 *  - We do NOT cache the secret key anywhere; signing always defers to Phantom.
 *  - Any RPC interaction (balance, broadcast, blockhash) is the caller's job —
 *    the context only owns wallet identity + signing.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  connectPhantom,
  isPhantomInstalled,
  type PhantomConnection,
} from "./phantom.js";

interface WalletState {
  wallet: PhantomConnection | null;
  connecting: boolean;
  error: string | null;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  phantomInstalled: boolean;
}

const WalletContext = createContext<WalletState | null>(null);

export function WalletProvider({ children }: { children: ReactNode }) {
  const [wallet, setWallet] = useState<PhantomConnection | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const connect = useCallback(async () => {
    setError(null);
    setConnecting(true);
    try {
      const conn = await connectPhantom();
      setWallet(conn);
    } catch (e: unknown) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setConnecting(false);
    }
  }, []);

  const disconnect = useCallback(async () => {
    if (wallet) await wallet.disconnect();
    setWallet(null);
  }, [wallet]);

  // Try to silently restore Phantom's existing session on mount.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (!isPhantomInstalled()) return;
      try {
        const provider = (window as unknown as {
          phantom?: { solana?: { connect(opts?: { onlyIfTrusted?: boolean }): Promise<unknown> } };
          solana?: { connect(opts?: { onlyIfTrusted?: boolean }): Promise<unknown> };
        }).phantom?.solana ?? (window as unknown as {
          solana?: { connect(opts?: { onlyIfTrusted?: boolean }): Promise<unknown> };
        }).solana;
        await provider?.connect({ onlyIfTrusted: true });
        if (cancelled) return;
        const conn = await connectPhantom();
        if (!cancelled) setWallet(conn);
      } catch {
        /* user has not previously trusted this site */
      }
    })();
    return () => { cancelled = true; };
  }, []);

  const value = useMemo<WalletState>(() => ({
    wallet,
    connecting,
    error,
    connect,
    disconnect,
    phantomInstalled: isPhantomInstalled(),
  }), [wallet, connecting, error, connect, disconnect]);

  return <WalletContext.Provider value={value}>{children}</WalletContext.Provider>;
}

export function useWallet(): WalletState {
  const ctx = useContext(WalletContext);
  if (!ctx) throw new Error("useWallet must be used inside <WalletProvider>");
  return ctx;
}

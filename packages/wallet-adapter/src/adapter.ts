/**
 * AirSignWalletAdapter
 *
 * Implements @solana/wallet-adapter-base BaseSignerWalletAdapter.
 * Works with every dApp using @solana/wallet-adapter-react (Raydium, Jupiter,
 * Magic Eden, Tensor, …) as well as Phantom / Solflare — because
 * @solana/wallet-adapter-react already wraps each adapter in the Wallet
 * Standard protocol automatically.
 *
 * Signing flow (air-gapped, zero-backend):
 *   dApp calls signTransaction(tx)
 *     → serialises tx
 *     → dispatches "airsign:request" DOM event
 *     → AirSignProvider (mounted once in the app) renders the modal
 *     → modal fountain-encodes the payload → animated QR
 *     → user scans with AirSign mobile (offline)
 *     → mobile signs → shows response QR
 *     → modal scanner reads response → resolves the Promise
 *     → signed transaction returned to dApp
 */

import {
  BaseSignerWalletAdapter,
  WalletName,
  WalletReadyState,
  WalletNotConnectedError,
  WalletSignTransactionError,
  WalletSignMessageError,
  WalletError,
} from "@solana/wallet-adapter-base";
import {
  Transaction,
  VersionedTransaction,
  PublicKey,
} from "@solana/web3.js";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface AirSignWalletAdapterConfig {
  /**
   * Modal element id. The adapter mounts the signing modal into this DOM
   * element. If omitted, a new <div> is appended to document.body.
   */
  modalContainerId?: string;

  /**
   * Timeout (ms) to wait for the user to complete a QR signing round.
   * Defaults to 5 minutes (300_000 ms).
   */
  timeoutMs?: number;
}

// ─── Constants ────────────────────────────────────────────────────────────────

export const AirSignWalletName = "AirSign" as WalletName<"AirSign">;

// Minimal SVG icon encoded as data-URI (no external deps).
const AIRSIGN_ICON =
  "data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMTI4IiBoZWlnaHQ9IjEyOCIgdmlld0JveD0iMCAwIDEyOCAxMjgiIGZpbGw9Im5vbmUiIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyI+PHJlY3Qgd2lkdGg9IjEyOCIgaGVpZ2h0PSIxMjgiIHJ4PSIyNCIgZmlsbD0iIzE0MTQxNCIvPjx0ZXh0IHg9IjY0IiB5PSI4NCIgZm9udC1zaXplPSI3MiIgdGV4dC1hbmNob3I9Im1pZGRsZSIgZmlsbD0id2hpdGUiPuKciTwvdGV4dD48L3N2Zz4=" as `data:image/svg+xml;base64,${string}`;

const DEFAULT_TIMEOUT_MS = 5 * 60 * 1000; // 5 min

// ─── Pending-request bus ──────────────────────────────────────────────────────
// The adapter posts requests here; the AirSignModal (rendered by AirSignProvider)
// resolves or rejects them.

export type AirSignRequest =
  | { kind: "connect" }
  | { kind: "signTransaction"; tx: Uint8Array }
  | { kind: "signAllTransactions"; txs: Uint8Array[] }
  | { kind: "signMessage"; message: Uint8Array };

export type AirSignResponse =
  | { kind: "connected"; pubkeyBase58: string }
  | { kind: "signedTransaction"; signed: Uint8Array }
  | { kind: "signedAllTransactions"; signed: Uint8Array[] }
  | { kind: "signedMessage"; signature: Uint8Array }
  | { kind: "cancelled" }
  | { kind: "error"; message: string };

type PendingRequest = {
  request: AirSignRequest;
  resolve: (res: AirSignResponse) => void;
  reject: (err: Error) => void;
  timeoutHandle: ReturnType<typeof setTimeout>;
};

// Singleton bus — one adapter instance per page.
let _pendingRequest: PendingRequest | null = null;

/**
 * Called by AirSignModal when the user completes or cancels a signing round.
 * Bridge between the React modal and the imperative adapter class.
 */
export function resolveAirSignRequest(response: AirSignResponse): void {
  if (!_pendingRequest) return;
  const { resolve, timeoutHandle } = _pendingRequest;
  clearTimeout(timeoutHandle);
  _pendingRequest = null;
  resolve(response);
}

/**
 * Returns the currently pending request so AirSignModal knows what to render.
 * Returns null when no request is in flight.
 */
export function getPendingRequest(): AirSignRequest | null {
  return _pendingRequest?.request ?? null;
}

// ─── Adapter ──────────────────────────────────────────────────────────────────

export class AirSignWalletAdapter extends BaseSignerWalletAdapter {
  readonly name = AirSignWalletName;
  readonly url = "https://github.com/nzengi/AirSign";
  readonly icon = AIRSIGN_ICON;
  readonly supportedTransactionVersions = new Set<"legacy" | 0>([
    "legacy",
    0,
  ]);

  private _config: Required<AirSignWalletAdapterConfig>;
  private _publicKey: PublicKey | null = null;
  private _connecting = false;

  constructor(config: AirSignWalletAdapterConfig = {}) {
    super();
    this._config = {
      modalContainerId: config.modalContainerId ?? "__airsign_modal",
      timeoutMs: config.timeoutMs ?? DEFAULT_TIMEOUT_MS,
    };
  }

  // ── PublicKey ──────────────────────────────────────────────────────────────

  get publicKey(): PublicKey | null {
    return this._publicKey;
  }

  // ── ReadyState ─────────────────────────────────────────────────────────────

  get readyState(): WalletReadyState {
    return WalletReadyState.Installed;
  }

  // ── Connecting ─────────────────────────────────────────────────────────────

  get connecting(): boolean {
    return this._connecting;
  }

  // ── connect() ──────────────────────────────────────────────────────────────

  async connect(): Promise<void> {
    if (this._publicKey) return;
    if (this._connecting) return;

    this._connecting = true;

    try {
      const res = await this._sendRequest({ kind: "connect" });

      if (res.kind === "cancelled") {
        throw new WalletNotConnectedError("User cancelled connection");
      }
      if (res.kind === "error") {
        throw new WalletNotConnectedError(res.message);
      }
      if (res.kind !== "connected") {
        throw new WalletNotConnectedError("Unexpected response from modal");
      }

      this._publicKey = new PublicKey(res.pubkeyBase58);
      this.emit("connect", this._publicKey);
    } catch (err: unknown) {
      this._publicKey = null;
      const walletErr =
        err instanceof WalletError
          ? err
          : new WalletNotConnectedError(
              err instanceof Error ? err.message : String(err)
            );
      this.emit("error", walletErr);
      throw walletErr;
    } finally {
      this._connecting = false;
    }
  }

  // ── disconnect() ───────────────────────────────────────────────────────────

  async disconnect(): Promise<void> {
    this._publicKey = null;
    this.emit("disconnect");
  }

  // ── signTransaction() ──────────────────────────────────────────────────────

  async signTransaction<T extends Transaction | VersionedTransaction>(
    tx: T
  ): Promise<T> {
    if (!this._publicKey) throw new WalletNotConnectedError();

    const serialised =
      tx instanceof VersionedTransaction
        ? tx.serialize()
        : tx.serialize({ requireAllSignatures: false, verifySignatures: false });

    const res = await this._sendRequest({
      kind: "signTransaction",
      tx: serialised,
    }).catch((err: unknown) => {
      throw new WalletSignTransactionError(
        err instanceof Error ? err.message : String(err)
      );
    });

    if (res.kind === "cancelled") throw new WalletSignTransactionError("User cancelled signing");
    if (res.kind === "error") throw new WalletSignTransactionError(res.message);
    if (res.kind !== "signedTransaction") throw new WalletSignTransactionError("Unexpected response");

    return (
      tx instanceof VersionedTransaction
        ? VersionedTransaction.deserialize(res.signed)
        : Transaction.from(res.signed)
    ) as T;
  }

  // ── signAllTransactions() ──────────────────────────────────────────────────

  async signAllTransactions<T extends Transaction | VersionedTransaction>(
    txs: T[]
  ): Promise<T[]> {
    if (!this._publicKey) throw new WalletNotConnectedError();

    const serialised = txs.map((tx) =>
      tx instanceof VersionedTransaction
        ? tx.serialize()
        : tx.serialize({ requireAllSignatures: false, verifySignatures: false })
    );

    const res = await this._sendRequest({
      kind: "signAllTransactions",
      txs: serialised,
    }).catch((err: unknown) => {
      throw new WalletSignTransactionError(
        err instanceof Error ? err.message : String(err)
      );
    });

    if (res.kind === "cancelled") throw new WalletSignTransactionError("User cancelled signing");
    if (res.kind === "error") throw new WalletSignTransactionError(res.message);
    if (res.kind !== "signedAllTransactions") throw new WalletSignTransactionError("Unexpected response");

    return res.signed.map((bytes, i) =>
      (txs[i] instanceof VersionedTransaction
        ? VersionedTransaction.deserialize(bytes)
        : Transaction.from(bytes)) as T
    );
  }

  // ── signMessage() ──────────────────────────────────────────────────────────

  async signMessage(message: Uint8Array): Promise<Uint8Array> {
    if (!this._publicKey) throw new WalletNotConnectedError();

    const res = await this._sendRequest({ kind: "signMessage", message }).catch(
      (err: unknown) => {
        throw new WalletSignMessageError(
          err instanceof Error ? err.message : String(err)
        );
      }
    );

    if (res.kind === "cancelled") throw new WalletSignMessageError("User cancelled signing");
    if (res.kind === "error") throw new WalletSignMessageError(res.message);
    if (res.kind !== "signedMessage") throw new WalletSignMessageError("Unexpected response");

    return res.signature;
  }

  // ── Internal ──────────────────────────────────────────────────────────────

  private _sendRequest(request: AirSignRequest): Promise<AirSignResponse> {
    if (_pendingRequest) {
      _pendingRequest.reject(new Error("Superseded by a new request"));
      clearTimeout(_pendingRequest.timeoutHandle);
      _pendingRequest = null;
    }

    return new Promise<AirSignResponse>((resolve, reject) => {
      const timeoutHandle = setTimeout(() => {
        _pendingRequest = null;
        reject(new Error("AirSign: signing timed out"));
      }, this._config.timeoutMs);

      _pendingRequest = { request, resolve, reject, timeoutHandle };

      if (typeof window !== "undefined") {
        window.dispatchEvent(
          new CustomEvent("airsign:request", { detail: request })
        );
      }
    });
  }
}
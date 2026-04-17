/**
 * useRecvSession — unit tests
 *
 * Tests the receive side hook by injecting a stub WasmRecvSession.  A minimal
 * "fountain" is simulated by feeding frames one at a time and checking that
 * the hook signals completion once the stub session reports isDone.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRecvSession } from "../hooks/useRecvSession.js";

// ─── Stub WASM receive session ────────────────────────────────────────────────

function makeStubRecvSession(targetFrames = 3) {
  let received = 0;
  const limit = targetFrames;
  const payload = new Uint8Array([10, 20, 30, 40, 50]);

  return {
    // Called by ingest() in the hook — returns true when complete
    ingest_frame: vi.fn((_frame: Uint8Array) => {
      received++;
      return received >= limit;
    }),
    progress: vi.fn(() => Math.min(1, received / limit)),
    received_count: vi.fn(() => BigInt(received)),
    filename: vi.fn(() => (received >= limit ? "signed_tx.bin" : undefined)),
    original_size: vi.fn(() => (received >= limit ? BigInt(payload.length) : undefined)),
    get_data: vi.fn(() => payload),
    free: vi.fn(),
  };
}

// ─── Setup / teardown ─────────────────────────────────────────────────────────

let stubSession: ReturnType<typeof makeStubRecvSession>;

beforeEach(() => {
  stubSession = makeStubRecvSession(3);

  (globalThis as Record<string, unknown>).__airsign_wasm__ = {
    WasmRecvSession: vi.fn(() => stubSession),
  };
});

afterEach(() => {
  delete (globalThis as Record<string, unknown>).__airsign_wasm__;
  vi.restoreAllMocks();
});

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("useRecvSession", () => {
  const OPTS = { password: "recv-test" };

  it("starts in idle state", () => {
    const { result } = renderHook(() => useRecvSession(OPTS));
    expect(result.current.isComplete).toBe(false);
    expect(result.current.progress).toBe(0);
    expect(result.current.error).toBeNull();
  });

  it("ingest() increases progress", () => {
    const { result } = renderHook(() => useRecvSession(OPTS));
    act(() => result.current.ingest(new Uint8Array([1, 2, 3])));
    expect(result.current.progress).toBeGreaterThan(0);
  });

  it("isComplete becomes true after sufficient frames", () => {
    const { result } = renderHook(() => useRecvSession(OPTS));
    act(() => result.current.ingest(new Uint8Array([1])));
    act(() => result.current.ingest(new Uint8Array([2])));
    act(() => result.current.ingest(new Uint8Array([3])));
    expect(result.current.isComplete).toBe(true);
  });

  it("calls onComplete with the assembled payload", () => {
    const onComplete = vi.fn();
    const { result } = renderHook(() =>
      useRecvSession({ ...OPTS, onComplete }),
    );
    act(() => result.current.ingest(new Uint8Array([1])));
    act(() => result.current.ingest(new Uint8Array([2])));
    act(() => result.current.ingest(new Uint8Array([3])));
    expect(onComplete).toHaveBeenCalledOnce();
    const [data, filename] = onComplete.mock.calls[0] as [Uint8Array, string | undefined];
    expect(data).toBeInstanceOf(Uint8Array);
    expect(data.length).toBeGreaterThan(0);
    expect(filename).toBe("signed_tx.bin");
  });

  it("calls onProgress on each ingested frame", () => {
    const onProgress = vi.fn();
    const { result } = renderHook(() =>
      useRecvSession({ ...OPTS, onProgress }),
    );
    act(() => result.current.ingest(new Uint8Array([1])));
    expect(onProgress).toHaveBeenCalledOnce();
    expect(onProgress.mock.calls[0]?.[0]).toBeGreaterThan(0);
  });

  it("reset() clears progress and isComplete", () => {
    const { result } = renderHook(() => useRecvSession(OPTS));
    act(() => result.current.ingest(new Uint8Array([1])));
    act(() => result.current.ingest(new Uint8Array([2])));
    act(() => result.current.ingest(new Uint8Array([3])));
    expect(result.current.isComplete).toBe(true);
    act(() => result.current.reset());
    expect(result.current.isComplete).toBe(false);
    expect(result.current.progress).toBe(0);
  });

  it("reports error when WASM module is missing", () => {
    delete (globalThis as Record<string, unknown>).__airsign_wasm__;
    const { result } = renderHook(() => useRecvSession(OPTS));
    act(() => result.current.ingest(new Uint8Array([1])));
    expect(result.current.error).toBeTruthy();
  });

  it("frees the WASM session on unmount", () => {
    const { result, unmount } = renderHook(() => useRecvSession(OPTS));
    act(() => result.current.ingest(new Uint8Array([1])));
    unmount();
    expect(stubSession.free).toHaveBeenCalled();
  });

  it("does not call onComplete twice for duplicate frames", () => {
    const onComplete = vi.fn();
    const { result } = renderHook(() =>
      useRecvSession({ ...OPTS, onComplete }),
    );
    // Complete the session
    act(() => result.current.ingest(new Uint8Array([1])));
    act(() => result.current.ingest(new Uint8Array([2])));
    act(() => result.current.ingest(new Uint8Array([3])));
    // Feed extra frames after completion
    act(() => result.current.ingest(new Uint8Array([4])));
    act(() => result.current.ingest(new Uint8Array([5])));
    expect(onComplete).toHaveBeenCalledOnce();
  });
});
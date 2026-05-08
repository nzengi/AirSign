/**
 * useSendSession — unit tests
 *
 * We test the hook in isolation by injecting a stub WasmSendSession into
 * `globalThis.__airsign_wasm__` so no real WASM binary is needed.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useSendSession } from "../hooks/useSendSession.js";

// ─── Stub WASM session ────────────────────────────────────────────────────────

function makeStubSession(totalFrames = 10) {
  let emitted = 0;
  const limit = totalFrames;

  return {
    has_next: vi.fn(() => emitted < limit),
    next_frame: vi.fn(() => {
      if (emitted >= limit) return null;
      emitted++;
      return new Uint8Array([emitted]);
    }),
    progress: vi.fn(() => Math.min(1, emitted / limit)),
    frame_index: vi.fn(() => emitted),
    total_frames: vi.fn(() => limit),
    droplet_count: vi.fn(() => limit),
    recommended_droplet_count: vi.fn(() => limit),
    set_limit: vi.fn(),
    free: vi.fn(),
  };
}

// ─── Setup / teardown ─────────────────────────────────────────────────────────

let stubSession: ReturnType<typeof makeStubSession>;

beforeEach(() => {
  vi.useFakeTimers();
  stubSession = makeStubSession(10);

  // Inject stub WASM module into globalThis
  (globalThis as Record<string, unknown>).__airsign_wasm__ = {
    WasmSendSession: vi.fn(() => stubSession),
  };
});

afterEach(() => {
  vi.useRealTimers();
  delete (globalThis as Record<string, unknown>).__airsign_wasm__;
});

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("useSendSession", () => {
  const DATA = new Uint8Array([1, 2, 3, 4]);
  const OPTS = { data: DATA, password: "test-pw", fps: 10 };

  it("starts in idle state", () => {
    const { result } = renderHook(() => useSendSession(OPTS));
    expect(result.current.isRunning).toBe(false);
    expect(result.current.isDone).toBe(false);
    expect(result.current.progress).toBe(0);
    expect(result.current.error).toBeNull();
  });

  it("start() sets isRunning to true", () => {
    const { result } = renderHook(() => useSendSession(OPTS));
    act(() => result.current.start());
    expect(result.current.isRunning).toBe(true);
  });

  it("stop() clears isRunning", () => {
    const { result } = renderHook(() => useSendSession(OPTS));
    act(() => result.current.start());
    act(() => result.current.stop());
    expect(result.current.isRunning).toBe(false);
  });

  it("advances frameIndex after ticking", () => {
    const { result } = renderHook(() => useSendSession(OPTS));
    act(() => result.current.start());
    // Advance 5 frames (fps=10 → 100ms per frame)
    act(() => vi.advanceTimersByTime(500));
    expect(result.current.frameIndex).toBeGreaterThan(0);
  });

  it("reaches isDone after recommended frames", () => {
    const { result } = renderHook(() => useSendSession(OPTS));
    act(() => result.current.start());
    // Advance enough time for all 10 frames (100ms each)
    act(() => vi.advanceTimersByTime(1100));
    expect(result.current.isDone).toBe(true);
    expect(result.current.progress).toBeGreaterThanOrEqual(1);
  });

  it("reset() clears progress and frameIndex", () => {
    const { result } = renderHook(() => useSendSession(OPTS));
    act(() => result.current.start());
    act(() => vi.advanceTimersByTime(300));
    act(() => result.current.reset());
    expect(result.current.progress).toBe(0);
    expect(result.current.frameIndex).toBe(0);
    expect(result.current.isRunning).toBe(false);
  });

  it("calls onComplete when isDone", () => {
    const onComplete = vi.fn();
    const { result } = renderHook(() =>
      useSendSession({ ...OPTS, onComplete }),
    );
    act(() => result.current.start());
    act(() => vi.advanceTimersByTime(1100));
    expect(onComplete).toHaveBeenCalledOnce();
  });

  it("calls onProgress on each frame", () => {
    const onProgress = vi.fn();
    const { result } = renderHook(() =>
      useSendSession({ ...OPTS, onProgress }),
    );
    act(() => result.current.start());
    act(() => vi.advanceTimersByTime(300)); // ~3 frames
    expect(onProgress).toHaveBeenCalled();
  });

  it("reports error when WASM module is missing", () => {
    delete (globalThis as Record<string, unknown>).__airsign_wasm__;
    const { result } = renderHook(() => useSendSession(OPTS));
    act(() => result.current.start());
    expect(result.current.error).toBeTruthy();
  });

  it("frees the WASM session on unmount", () => {
    const { result, unmount } = renderHook(() => useSendSession(OPTS));
    act(() => result.current.start());
    // Force session creation by advancing one tick
    act(() => vi.advanceTimersByTime(50));
    unmount();
    expect(stubSession.free).toHaveBeenCalled();
  });
});
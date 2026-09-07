import { afterEach, describe, expect, it, vi } from "vitest";
import { networkStatus } from "../src/data/network-status";

describe("networkStatus", () => {
  let originalOnLine;

  afterEach(() => {
    Object.defineProperty(navigator, "onLine", { value: originalOnLine, configurable: true });
  });

  it("reports offline as false when navigator.onLine is true", () => {
    originalOnLine = navigator.onLine;
    Object.defineProperty(navigator, "onLine", { value: true, configurable: true });

    expect(networkStatus.offline).toBe(false);
  });

  it("reports offline as true when navigator.onLine is false", () => {
    originalOnLine = navigator.onLine;
    Object.defineProperty(navigator, "onLine", { value: false, configurable: true });

    expect(networkStatus.offline).toBe(true);
  });

  it("onChange binds to both online and offline events and returns an unbinder", () => {
    originalOnLine = navigator.onLine;
    const handler = vi.fn();

    const unbind = networkStatus.onChange(handler);
    window.dispatchEvent(new Event("offline"));
    window.dispatchEvent(new Event("online"));
    expect(handler).toHaveBeenCalledTimes(2);

    unbind();
    window.dispatchEvent(new Event("offline"));
    expect(handler).toHaveBeenCalledTimes(2);
  });
});

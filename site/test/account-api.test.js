import { beforeEach, describe, expect, it, vi } from "vitest";
import { accountApi } from "../src/data/account-api";
import { accountStorage } from "../src/data/account-storage";

describe("accountApi", () => {
  beforeEach(() => {
    globalThis.fetch = vi.fn();
    accountStorage.clearAccountToken();
  });

  it("exposes account auth urls", () => {
    expect(accountApi.registerUrl).toBe("/api/account/register");
    expect(accountApi.loginUrl).toBe("/api/account/login");
    expect(accountApi.discordRedirectUrl).toBe("/api/account/discord/redirect");
  });

  it("register stores the session token on success", async () => {
    globalThis.fetch.mockResolvedValue({
      ok: true,
      status: 201,
      json: () => Promise.resolve({ account: { id: 1, email: "a@b.com" }, token: "new-token" }),
    });

    const response = await accountApi.register("a@b.com", "password123");

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/register",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ email: "a@b.com", password: "password123" }),
      }),
    );
    expect(response.ok).toBe(true);
    expect(accountStorage.getAccountToken()).toBe("new-token");
  });

  it("register does not store a token on failure", async () => {
    globalThis.fetch.mockResolvedValue({ ok: false, status: 409 });

    await accountApi.register("a@b.com", "password123");

    expect(accountStorage.getAccountToken()).toBeNull();
  });

  it("login stores the session token on success", async () => {
    globalThis.fetch.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve({ account: { id: 1, email: "a@b.com" }, token: "session-token" }),
    });

    const response = await accountApi.login("a@b.com", "password123");

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/login",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ email: "a@b.com", password: "password123" }),
      }),
    );
    expect(response.ok).toBe(true);
    expect(accountStorage.getAccountToken()).toBe("session-token");
  });

  it("login does not store a token on failure", async () => {
    globalThis.fetch.mockResolvedValue({ ok: false, status: 401 });

    await accountApi.login("a@b.com", "wrong-password");

    expect(accountStorage.getAccountToken()).toBeNull();
  });
});

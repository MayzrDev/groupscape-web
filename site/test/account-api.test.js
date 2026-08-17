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

  it("listCharacters returns unauthenticated without a stored token", async () => {
    const response = await accountApi.listCharacters();

    expect(response).toEqual({ ok: false, status: 401 });
    expect(globalThis.fetch).not.toHaveBeenCalled();
  });

  it("listCharacters sends the stored token", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve([{ id: 1, display_rsn: "Zezima" }]),
    });

    const response = await accountApi.listCharacters();

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/characters",
      expect.objectContaining({
        headers: { Authorization: "session-token" },
      }),
    );
    expect(response.ok).toBe(true);
  });

  it("exposes account settings urls", () => {
    expect(accountApi.emailUrl).toBe("/api/account/email");
    expect(accountApi.passwordUrl).toBe("/api/account/password");
    expect(accountApi.deleteAccountUrl).toBe("/api/account");
  });

  it("updateEmail sends the stored token and new email", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve({ id: 1, email: "new@b.com" }),
    });

    const response = await accountApi.updateEmail("new@b.com");

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/email",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({ email: "new@b.com" }),
        headers: { "Content-Type": "application/json", Authorization: "session-token" },
      }),
    );
    expect(response.ok).toBe(true);
  });

  it("changePassword sends the stored token and both passwords", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({ ok: true, status: 204 });

    const response = await accountApi.changePassword("oldpassword", "newpassword123");

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/password",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({ current_password: "oldpassword", new_password: "newpassword123" }),
        headers: { "Content-Type": "application/json", Authorization: "session-token" },
      }),
    );
    expect(response.ok).toBe(true);
  });

  it("deleteAccount clears the stored token on success", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({ ok: true, status: 204 });

    const response = await accountApi.deleteAccount();

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account",
      expect.objectContaining({
        method: "DELETE",
        headers: { Authorization: "session-token" },
      }),
    );
    expect(response.ok).toBe(true);
    expect(accountStorage.getAccountToken()).toBeNull();
  });

  it("deleteAccount keeps the stored token on failure", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({ ok: false, status: 401 });

    await accountApi.deleteAccount();

    expect(accountStorage.getAccountToken()).toBe("session-token");
  });
});

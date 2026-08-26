import { beforeEach, describe, expect, it, vi } from "vitest";
import { accountApi } from "../src/data/account-api";
import { accountStorage } from "../src/data/account-storage";

describe("accountApi", () => {
  beforeEach(() => {
    globalThis.fetch = vi.fn();
    accountStorage.clearAccountToken();
    sessionStorage.removeItem("freshApiKey");
    window.history.replaceState("", "", "/");
  });

  it("exposes account auth urls", () => {
    expect(accountApi.registerUrl).toBe("/api/account/register");
    expect(accountApi.loginUrl).toBe("/api/account/login");
    expect(accountApi.discordRedirectUrl).toBe("/api/account/discord/redirect");
  });

  it("consumes a Discord callback and routes to onboarding", () => {
    window.location.hash = "#token=discord-session&api_key=discord-key";

    expect(accountApi.handleDiscordCallback()).toEqual({ status: "logged_in" });
    expect(accountStorage.getAccountToken()).toBe("discord-session");
    expect(sessionStorage.getItem("freshApiKey")).toBe("discord-key");
    expect(window.location.pathname).toBe("/welcome");
    expect(window.location.hash).toBe("");
  });

  it("surfaces a Discord error and routes back to login", () => {
    window.location.hash = "#error=discord_failed";

    expect(accountApi.handleDiscordCallback()).toEqual({ status: "error", error: "discord_failed" });
    expect(accountStorage.getAccountToken()).toBeNull();
    expect(window.location.pathname).toBe("/account/login");
    expect(window.location.hash).toBe("");
  });

  it("reports a completed Discord link and routes to the account page", () => {
    window.location.hash = "#discord_linked=1";

    expect(accountApi.handleDiscordCallback()).toEqual({ status: "linked" });
    expect(window.location.pathname).toBe("/account");
    expect(window.location.hash).toBe("");
  });

  it("ignores callback fragments with none of token/error/discord_linked", () => {
    window.location.hash = "#foo=bar";

    expect(accountApi.handleDiscordCallback()).toEqual({ status: "none" });
    expect(window.location.hash).toBe("#foo=bar");
  });

  it("discordLinkRedirect fetches a signed authorize url and navigates to it", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve({ redirect_url: "https://discord.com/oauth2/authorize?state=abc" }),
    });

    await accountApi.discordLinkRedirect();

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/discord/link/redirect",
      expect.objectContaining({ headers: { Authorization: "session-token" } }),
    );
    expect(window.location.href).toContain("https://discord.com/oauth2/authorize");
  });

  it("register stores the session token on success", async () => {
    globalThis.fetch.mockResolvedValue({
      ok: true,
      status: 201,
      json: () => Promise.resolve({ account: { id: 1, username: "player" }, token: "new-token" }),
    });

    const response = await accountApi.register("player", "password123");

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/register",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ username: "player", password: "password123" }),
      }),
    );
    expect(response.ok).toBe(true);
    expect(accountStorage.getAccountToken()).toBe("new-token");
  });

  it("register does not store a token on failure", async () => {
    globalThis.fetch.mockResolvedValue({ ok: false, status: 409 });

    await accountApi.register("player", "password123");

    expect(accountStorage.getAccountToken()).toBeNull();
  });

  it("register stashes the fresh api key in sessionStorage on success", async () => {
    globalThis.fetch.mockResolvedValue({
      ok: true,
      status: 201,
      json: () => Promise.resolve({ account: { id: 1, username: "player" }, token: "new-token", api_key: "gsk_abc" }),
    });

    await accountApi.register("player", "password123");

    expect(sessionStorage.getItem("freshApiKey")).toBe("gsk_abc");
  });

  it("login stores the session token on success", async () => {
    globalThis.fetch.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve({ account: { id: 1, username: "player" }, token: "session-token" }),
    });

    const response = await accountApi.login("player", "password123");

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/login",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ username: "player", password: "password123" }),
      }),
    );
    expect(response.ok).toBe(true);
    expect(accountStorage.getAccountToken()).toBe("session-token");
  });

  it("login does not store a token on failure", async () => {
    globalThis.fetch.mockResolvedValue({ ok: false, status: 401 });

    await accountApi.login("player", "wrong-password");

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

  it("unlinkCharacter sends the stored token to the character's url", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({ ok: true, status: 204 });

    const response = await accountApi.unlinkCharacter(42);

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/characters/42",
      expect.objectContaining({
        method: "DELETE",
        headers: { Authorization: "session-token" },
      }),
    );
    expect(response.ok).toBe(true);
  });

  it("linkCharacterToGroup posts the character id and group credentials", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({ ok: true, status: 201 });

    const response = await accountApi.linkCharacterToGroup("42", "Iron Legion", "group-token");

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/characters/link-group",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ character_id: 42, group_name: "Iron Legion", group_token: "group-token" }),
        headers: {
          "Content-Type": "application/json",
          Authorization: "session-token",
        },
      }),
    );
    expect(response.ok).toBe(true);
  });

  it("leaveGroup posts to the character's leave-group url", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({ ok: true, status: 200 });

    const response = await accountApi.leaveGroup(42);

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/characters/42/leave-group",
      expect.objectContaining({
        method: "POST",
        headers: { Authorization: "session-token" },
      }),
    );
    expect(response.ok).toBe(true);
  });

  it("confirmCharacter posts to the character's confirm url", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({ ok: true, status: 204 });

    const response = await accountApi.confirmCharacter(42);

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/characters/42/confirm",
      expect.objectContaining({
        method: "POST",
        headers: { Authorization: "session-token" },
      }),
    );
    expect(response.ok).toBe(true);
  });

  it("removePendingCharacter deletes the character's pending url", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({ ok: true, status: 204 });

    const response = await accountApi.removePendingCharacter(42);

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/characters/42/pending",
      expect.objectContaining({
        method: "DELETE",
        headers: { Authorization: "session-token" },
      }),
    );
    expect(response.ok).toBe(true);
  });

  it("getCharacterPortrait returns the array buffer on success", async () => {
    accountStorage.storeAccountToken("session-token");
    const buffer = new ArrayBuffer(8);
    globalThis.fetch.mockResolvedValue({ ok: true, status: 200, arrayBuffer: () => Promise.resolve(buffer) });

    const result = await accountApi.getCharacterPortrait(42);

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/characters/42/portrait",
      expect.objectContaining({ headers: { Authorization: "session-token" } }),
    );
    expect(result).toBe(buffer);
  });

  it("getCharacterPortrait returns null on failure", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({ ok: false, status: 404 });

    const result = await accountApi.getCharacterPortrait(42);

    expect(result).toBeNull();
  });

  it("regenerateApiKey posts to the api-key url", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve({ api_key: "gsk_new" }),
    });

    const response = await accountApi.regenerateApiKey();

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/api-key",
      expect.objectContaining({
        method: "POST",
        headers: { Authorization: "session-token" },
      }),
    );
    expect(response.ok).toBe(true);
  });

  it("exposes account settings urls", () => {
    expect(accountApi.usernameUrl).toBe("/api/account/username");
    expect(accountApi.passwordUrl).toBe("/api/account/password");
    expect(accountApi.deleteAccountUrl).toBe("/api/account");
  });

  it("updateUsername sends the stored token and new username", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({
      ok: true,
      status: 200,
      json: () => Promise.resolve({ id: 1, username: "new-name" }),
    });

    const response = await accountApi.updateUsername("new-name");

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/username",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({ username: "new-name" }),
        headers: { "Content-Type": "application/json", Authorization: "session-token" },
      }),
    );
    expect(response.ok).toBe(true);
  });

  it("changePassword sends the stored token and the new password only - no current-password re-entry", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({ ok: true, status: 204 });

    const response = await accountApi.changePassword("newpassword123");

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/password",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({ new_password: "newpassword123" }),
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

  it("vapidPublicKey fetches the unauthed vapid-public-key url", async () => {
    globalThis.fetch.mockResolvedValue({ ok: true, status: 200, json: () => Promise.resolve({ publicKey: "abc" }) });

    const response = await accountApi.vapidPublicKey();

    expect(globalThis.fetch).toHaveBeenCalledWith("/api/account/push/vapid-public-key");
    expect(response.ok).toBe(true);
  });

  it("subscribePush posts the subscription's JSON with the stored token", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({ ok: true, status: 201, json: () => Promise.resolve({ id: 1 }) });
    const subscription = { toJSON: () => ({ endpoint: "https://push.example/x", keys: { p256dh: "p", auth: "a" } }) };

    const response = await accountApi.subscribePush(subscription);

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/push/subscribe",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ endpoint: "https://push.example/x", keys: { p256dh: "p", auth: "a" } }),
        headers: { "Content-Type": "application/json", Authorization: "session-token" },
      }),
    );
    expect(response.ok).toBe(true);
  });

  it("unsubscribePush sends the endpoint with the stored token", async () => {
    accountStorage.storeAccountToken("session-token");
    globalThis.fetch.mockResolvedValue({ ok: true, status: 204 });

    const response = await accountApi.unsubscribePush("https://push.example/x");

    expect(globalThis.fetch).toHaveBeenCalledWith(
      "/api/account/push/subscribe",
      expect.objectContaining({
        method: "DELETE",
        body: JSON.stringify({ endpoint: "https://push.example/x" }),
        headers: { "Content-Type": "application/json", Authorization: "session-token" },
      }),
    );
    expect(response.ok).toBe(true);
  });
});

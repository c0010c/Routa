import type { ManualNode } from "../bindings";
import { findDuplicateNode, isRealityVision, isSupportedFingerprint, publicKeyError, shortIdError, splitHostPort } from "./vlessReality";

const reality = {
  name: "a",
  type: "vless",
  server: "s",
  port: 443,
  uuid: "u",
  tls: true,
  network: "tcp",
  "reality-opts": { "public-key": "k", "short-id": "" },
};

describe("isRealityVision", () => {
  it("accepts vless over tcp with reality-opts", () => {
    expect(isRealityVision(reality)).toBe(true);
    expect(isRealityVision({ ...reality, network: undefined })).toBe(true);
  });
  it("rejects other transports, plain tls and other types", () => {
    expect(isRealityVision({ ...reality, network: "ws" })).toBe(false);
    expect(isRealityVision({ ...reality, "reality-opts": undefined })).toBe(false);
    expect(isRealityVision({ ...reality, type: "trojan" })).toBe(false);
    expect(isRealityVision(undefined)).toBe(false);
  });
});

describe("publicKeyError", () => {
  it("accepts a 43-char base64url key and empty input", () => {
    expect(publicKeyError("Gq9XwR0oZ3nLk2bYp7VtHc5mJdE1sAfU8iQyN4xKr0M")).toBeUndefined();
    expect(publicKeyError("")).toBeUndefined();
  });
  it("rejects wrong length or alphabet", () => {
    expect(publicKeyError("abc")).toMatch(/43/);
    expect(publicKeyError("Gq9XwR0oZ3nLk2bYp7VtHc5mJdE1sAfU8iQyN4xKr0+")).toMatch(/43/);
  });
});

describe("shortIdError", () => {
  it("accepts empty and even-length hex up to 16", () => {
    expect(shortIdError("")).toBeUndefined();
    expect(shortIdError("6ba85179")).toBeUndefined();
    expect(shortIdError("0123456789abcdef")).toBeUndefined();
  });
  it("rejects odd length, non-hex and too long", () => {
    expect(shortIdError("abc")).toBeDefined();
    expect(shortIdError("zz")).toBeDefined();
    expect(shortIdError("0123456789abcdef00")).toBeDefined();
  });
});

describe("splitHostPort", () => {
  it("splits host:port and [v6]:port", () => {
    expect(splitHostPort("1.2.3.4:443")).toEqual({ host: "1.2.3.4", port: "443" });
    expect(splitHostPort("a.example.com:8443")).toEqual({ host: "a.example.com", port: "8443" });
    expect(splitHostPort("[2001:db8::1]:443")).toEqual({ host: "2001:db8::1", port: "443" });
  });
  it("leaves plain hosts and bare IPv6 alone", () => {
    expect(splitHostPort("example.com")).toBeNull();
    expect(splitHostPort("2001:db8::1")).toBeNull();
  });
});

describe("findDuplicateNode", () => {
  const node = (id: string, raw: object): ManualNode => ({ id, name: `n-${id}`, groups: [], kind: { Proxy: { raw: raw as never } } });
  const nodes = [node("1", { ...reality, server: "x", uuid: "u1" }), node("2", { type: "socks5", server: "x", port: 443 })];
  it("matches server, port and uuid, skipping the node being edited", () => {
    expect(findDuplicateNode(nodes, "new", "x", 443, "u1")?.name).toBe("n-1");
    expect(findDuplicateNode(nodes, "1", "x", 443, "u1")).toBeUndefined();
    expect(findDuplicateNode(nodes, "new", "x", 443, "u2")).toBeUndefined();
  });
});

describe("isSupportedFingerprint", () => {
  it("accepts mihomo uTLS names", () => {
    for (const fp of ["chrome", "chrome120", "chrome_pq", "randomized", "360"]) expect(isSupportedFingerprint(fp)).toBe(true);
  });
  it("rejects names mihomo warns about", () => {
    expect(isSupportedFingerprint("chrome140")).toBe(false);
    expect(isSupportedFingerprint("Chrome")).toBe(false);
  });
});

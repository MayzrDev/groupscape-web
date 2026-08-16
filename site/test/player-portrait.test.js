import { describe, expect, it } from "vitest";
import { PlayerPortrait } from "../src/player-portrait/player-portrait";

describe("PlayerPortrait.fitCameraDistance", () => {
  it("scales distance with bounding-sphere radius", () => {
    const distanceSmall = PlayerPortrait.fitCameraDistance(50, 30);
    const distanceLarge = PlayerPortrait.fitCameraDistance(100, 30);
    expect(distanceLarge).toBeCloseTo(distanceSmall * 2);
  });

  it("matches the padded bounding-sphere fit formula", () => {
    const radius = 80;
    const fovDegrees = 30;
    const fovRadians = (fovDegrees * Math.PI) / 180;
    const expected = (radius * 1.22) / Math.sin(fovRadians / 2);
    expect(PlayerPortrait.fitCameraDistance(radius, fovDegrees)).toBeCloseTo(expected);
  });

  it("increases distance for a narrower field of view", () => {
    const wideFov = PlayerPortrait.fitCameraDistance(50, 60);
    const narrowFov = PlayerPortrait.fitCameraDistance(50, 20);
    expect(narrowFov).toBeGreaterThan(wideFov);
  });
});

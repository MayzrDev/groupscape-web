import { describe, expect, it } from "vitest";
import * as THREE from "three";
import { PlayerPortrait } from "../src/player-portrait/player-portrait";

// Builds a synthetic geometry from per-band (minX, maxX, minZ, maxZ) extents, using two vertices
// per band placed at that band's y-center. This reproduces findBodyTop's per-band girth scan
// exactly without needing a full mesh — each band gets precisely the width/depth given.
function geometryFromBandExtents(bandExtents, bandHeight = 10) {
  const positions = [];
  bandExtents.forEach(([minX, maxX, minZ, maxZ], band) => {
    const y = band * bandHeight + bandHeight / 2;
    positions.push(minX, y, minZ);
    positions.push(maxX, y, maxZ);
  });
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.Float32BufferAttribute(positions, 3));
  geometry.computeBoundingBox();
  return geometry;
}

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

describe("PlayerPortrait.findBodyTop", () => {
  it("anchors on the shoulders, not a one-handed weapon raised just above them", () => {
    // Real per-band (minX, maxX, minZ, maxZ) extents captured from a character mid-swing with a
    // sword raised at an angle — bands 0-21 are the body (wide), band 22 is the sword blade (a
    // thin sliver only 2 bands above the shoulders), band 23 is the sword tip (near-empty).
    const bandExtents = [
      [-33.6, 17.7, -54.9, 9.5],
      [-27.3, 19, -56, 4.9],
      [-29.3, 17.6, -50.8, -2.5],
      [-23.7, 13.2, -52.7, -6.2],
      [-27.6, 24.3, -50.4, -5.4],
      [-32.2, 24.4, -46.9, -1.8],
      [-35.6, 32.3, -51, -3.1],
      [-27.5, 21.9, -42.7, -1.6],
      [-33.7, 30.3, -47.1, -9.6],
      [-21.8, 30, -35.3, -7.2],
      [-35.1, 37.9, -34.3, 2.2],
      [-36.4, 35.6, -31.3, 17.4],
      [-35.2, 31, -37.5, 16.4],
      [-37.9, 30.8, -35.2, 17.2],
      [-37, 29.9, -35.6, 14.9],
      [-34, 29.1, -33.6, 16.1],
      [-32.1, 26, -38.4, 25.7],
      [-30.1, 7.8, -36.4, 18.5],
      [-31.2, 8.8, -35.3, 29],
      [-26, 9.8, -25.4, 42.8],
      [-28.7, 10.9, -21.6, 39.6],
      [-27.4, 10.1, -30.5, 45.1],
      [-26.9, -24, 40.5, 52.6],
      [-24.3, -24.3, 56, 56],
    ];
    const bandHeight = 10;
    const geometry = geometryFromBandExtents(bandExtents, bandHeight);
    const box = geometry.boundingBox;

    const bodyTop = PlayerPortrait.findBodyTop(geometry, box);

    // Band 21 (the true shoulders/head boundary) sits at y = 21 * bandHeight = 210, well below
    // the sword tip at band 23 (y = 230, ~= box.max.y). Assert we land at the shoulders, not the
    // sword.
    expect(bodyTop).toBeLessThan(21.5 * bandHeight);
    expect(bodyTop).toBeGreaterThan(19 * bandHeight);
    expect(bodyTop).toBeLessThan(box.max.y - bandHeight);
  });

  it("still anchors on a data-sparse head/hood band, not the shoulders below it", () => {
    // Ordinary (unarmed) character: shoulders are the widest band, and the topmost band (head)
    // is data-sparse (few low-poly vertices landed there) but still a real, non-degenerate
    // cross-section — nowhere near as thin as a weapon blade's sliver.
    const bandExtents = [];
    for (let band = 0; band < 23; band++) {
      bandExtents.push([-20, 20, -15, 15]); // width 40, depth 30, girth 70 throughout the torso
    }
    bandExtents.push([-8, 8, -6, 6]); // topmost band (head): girth 28, ~40% of max — sparse but real

    const bandHeight = 10;
    const geometry = geometryFromBandExtents(bandExtents, bandHeight);
    const box = geometry.boundingBox;

    const bodyTop = PlayerPortrait.findBodyTop(geometry, box);

    // Must anchor at the topmost (head) band, not fall through to the shoulders below it.
    expect(bodyTop).toBeGreaterThan(21.5 * bandHeight);
  });
});

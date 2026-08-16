import { test, expect } from "@playwright/test";

// A minimal binary-little-endian PLY: one triangle with per-vertex RGB, matching what
// ModelExporter.java produces (per-triangle-corner vertices, no shared indexing beyond the face).
function buildTrianglePly(): Buffer {
  const header = [
    "ply",
    "format binary_little_endian 1.0",
    "element vertex 3",
    "property float x",
    "property float y",
    "property float z",
    "property uchar red",
    "property uchar green",
    "property uchar blue",
    "element face 1",
    "property list uchar int vertex_indices",
    "end_header",
    "",
  ].join("\n");

  const vertices: [number, number, number, number, number, number][] = [
    [0, 10, 0, 255, 0, 0],
    [-10, -10, 0, 0, 255, 0],
    [10, -10, 0, 0, 0, 255],
  ];

  const vertexBuffer = Buffer.alloc(vertices.length * 15);
  vertices.forEach(([x, y, z, r, g, b], i) => {
    const offset = i * 15;
    vertexBuffer.writeFloatLE(x, offset);
    vertexBuffer.writeFloatLE(y, offset + 4);
    vertexBuffer.writeFloatLE(z, offset + 8);
    vertexBuffer.writeUInt8(r, offset + 12);
    vertexBuffer.writeUInt8(g, offset + 13);
    vertexBuffer.writeUInt8(b, offset + 14);
  });

  const faceBuffer = Buffer.alloc(13);
  faceBuffer.writeUInt8(3, 0);
  faceBuffer.writeInt32LE(0, 1);
  faceBuffer.writeInt32LE(1, 5);
  faceBuffer.writeInt32LE(2, 9);

  return Buffer.concat([Buffer.from(header, "ascii"), vertexBuffer, faceBuffer]);
}

test("player-portrait renders a canvas mesh for a member with a captured portrait", async ({ page }) => {
  const mesh = buildTrianglePly();
  await page.route("**/portrait/**", (route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: mesh })
  );

  await page.goto("/");
  await page.evaluate(() => {
    const el = document.createElement("player-portrait");
    el.setAttribute("player-name", "TestPlayer");
    el.style.width = "200px";
    document.body.appendChild(el);
  });

  const portrait = page.locator("player-portrait");
  const canvas = portrait.locator("canvas");
  await expect(canvas).toBeVisible();
  await expect(portrait).not.toHaveClass(/player-portrait--no-portrait/);

  await expect
    .poll(() => portrait.evaluate((el: any) => !!el.mesh))
    .toBe(true);

  const canvasSize = await canvas.evaluate((el: HTMLCanvasElement) => ({ width: el.width, height: el.height }));
  expect(canvasSize.width).toBeGreaterThan(0);
  expect(canvasSize.height).toBeGreaterThan(0);
});

test("player-portrait shows the empty state when no portrait has been captured yet", async ({ page }) => {
  await page.route("**/portrait/**", (route) => route.fulfill({ status: 404 }));

  await page.goto("/");
  await page.evaluate(() => {
    const el = document.createElement("player-portrait");
    el.setAttribute("player-name", "NoPortraitPlayer");
    document.body.appendChild(el);
  });

  const portrait = page.locator("player-portrait");
  await expect(portrait).toHaveClass(/player-portrait--no-portrait/);
  await expect(portrait.locator(".player-portrait__empty")).toBeVisible();
});

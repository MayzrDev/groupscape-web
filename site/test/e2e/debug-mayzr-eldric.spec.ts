import { test } from "@playwright/test";
import * as fs from "fs";

const MESH_PATH =
  "C:/Users/Liam/AppData/Local/Temp/claude/d--code-groupscape/c2f049e3-1d5f-4156-8962-c5e76b6ce927/scratchpad/mayzr_eldric.ply";

test("dump band data + screenshots for latest mesh", async ({ page }) => {
  const mesh = fs.readFileSync(MESH_PATH);
  await page.route("**/portrait/**", (route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: mesh })
  );
  await page.goto("/");
  await page.evaluate(() => {
    const bust = document.createElement("player-portrait");
    bust.setAttribute("mode", "bust");
    bust.setAttribute("player-name", "MayzrEldric");
    bust.style.width = "44px";
    bust.style.height = "60px";
    document.body.style.background = "#222";
    document.body.appendChild(bust);

    const full = document.createElement("player-portrait");
    full.setAttribute("mode", "full");
    full.setAttribute("player-name", "MayzrEldric");
    full.id = "full";
    full.style.width = "300px";
    full.style.height = "500px";
    full.style.position = "fixed";
    full.style.top = "0";
    full.style.left = "200px";
    document.body.appendChild(full);
  });
  const bust = page.locator("player-portrait[mode=bust]");
  const full = page.locator("#full");
  await page.waitForFunction(() => !!(document.querySelector("player-portrait[mode=bust]") as any).mesh);
  await page.waitForTimeout(300);
  await bust.locator("canvas").screenshot({ path: "test-results/mayzr-eldric-bust.png" });
  await full.locator("canvas").screenshot({ path: "test-results/mayzr-eldric-full.png" });

  const info = await bust.evaluate((el: any) => {
    const geometry = el.mesh.geometry;
    const box = geometry.boundingBox;
    const position = geometry.attributes.position;
    const BANDS = 24;
    const yMin = box.min.y;
    const totalHeight = box.max.y - yMin;
    const bandMinX = new Array(BANDS).fill(Infinity);
    const bandMaxX = new Array(BANDS).fill(-Infinity);
    const bandMinZ = new Array(BANDS).fill(Infinity);
    const bandMaxZ = new Array(BANDS).fill(-Infinity);
    for (let i = 0; i < position.count; i++) {
      const x = position.getX(i);
      const z = position.getZ(i);
      let band = Math.floor(((position.getY(i) - yMin) / totalHeight) * BANDS);
      if (band >= BANDS) band = BANDS - 1;
      if (band < 0) band = 0;
      if (x < bandMinX[band]) bandMinX[band] = x;
      if (x > bandMaxX[band]) bandMaxX[band] = x;
      if (z < bandMinZ[band]) bandMinZ[band] = z;
      if (z > bandMaxZ[band]) bandMaxZ[band] = z;
    }
    const girths = [];
    for (let b = 0; b < BANDS; b++) {
      const w = bandMaxX[b] - bandMinX[b];
      const d = bandMaxZ[b] - bandMinZ[b];
      girths.push(Number((isFinite(w) && isFinite(d) ? w + d : 0).toFixed(2)));
    }
    return {
      boxMin: { x: box.min.x, y: box.min.y, z: box.min.z },
      boxMax: { x: box.max.x, y: box.max.y, z: box.max.z },
      cameraPos: { x: el.camera.position.x, y: el.camera.position.y, z: el.camera.position.z },
      vertexCount: position.count,
      girths,
    };
  });
  console.log("ELDRIC MESH INFO", JSON.stringify(info, null, 2));
});

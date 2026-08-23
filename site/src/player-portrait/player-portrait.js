import * as THREE from "three";
import { PLYLoader } from "three/examples/jsm/loaders/PLYLoader.js";
import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { accountApi } from "../data/account-api";
import { pubsub } from "../data/pubsub";

const FOV_DEGREES = 30;
// Headroom above a tight bounding-sphere fit so the model doesn't touch the frame edges.
const FRAMING_PADDING = 1.22;
// Extra zoom-out applied only to "bust" mode (side panel) on top of FRAMING_PADDING, so the
// full-body "full" mode (/panels) framing is unaffected.
const BUST_ZOOM_OUT_FACTOR = 1.25;
// Fraction of the body height (feet to top of head, see findBodyTop) treated as "head and
// shoulders" for the bust crop.
const BUST_HEIGHT_FRACTION = 0.38;
// Minimum width+depth, as a fraction of the model's widest band, for a horizontal band to count
// as part of the body rather than a held weapon/staff.
const BODY_GIRTH_FRACTION = 0.35;
// How many horizontal bands the model is scanned in, for locating the top of the body. Coarse on
// purpose — OSRS models are low-poly, so a handful of vertices can land in an otherwise-empty
// band and make a finer scan read as noise (see BODY_GIRTH_LOOKBEHIND below).
const BODY_GIRTH_SCAN_BANDS = 24;
// When checking a band, also accept it if any of this many bands below it (further from the top)
// clears the threshold. Bridges single sparse bands — e.g. a gap between a hood and the
// shoulders below it — that would otherwise be mistaken for the top of a held weapon.
const BODY_GIRTH_LOOKBEHIND = 2;
// A band can only be credited via the look-behind bridge above if its OWN girth clears this
// (smaller) fraction of the model's max girth. Without this floor, a thin weapon held just 1-2
// bands above the shoulders (e.g. a one-handed sword raised at an angle, rather than a two-handed
// spear/staff held straight overhead) gets bridged the same way a genuinely sparse hood/hair band
// does, anchoring the crop on the weapon tip instead of the head. A real head/hood band — even a
// data-sparse one — still has a real, roughly life-sized cross-section; a weapon shaft/blade seen
// from above is a hairline sliver by comparison.
const BODY_GIRTH_LOOKBEHIND_MIN_FRACTION = 0.15;
// Full-body ("full" mode, /panels only) auto-rotate + drag-to-rotate tuning.
const AUTO_ROTATE_RADIANS_PER_SECOND = 0.3;
const DRAG_RADIANS_PER_PIXEL = 0.008;
const AUTO_ROTATE_RESUME_DELAY_MS = 3000;

export class PlayerPortrait extends BaseElement {
  html() {
    return `{{player-portrait.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.playerName = this.getAttribute("player-name");
    this.accountCharacterId = this.getAttribute("account-character-id");
    this.mode = this.getAttribute("mode") === "full" ? "full" : "bust";
    this.setAttribute("mode", this.mode);
    this.render();

    this.frame = this.querySelector(".player-portrait__frame");
    this.canvas = this.querySelector("canvas");

    this.renderer = new THREE.WebGLRenderer({ canvas: this.canvas, alpha: true });
    this.camera = new THREE.PerspectiveCamera(FOV_DEGREES, 1, 1, 10000);
    this.camera.position.set(0, 0, 220);
    this.scene = new THREE.Scene();

    this.eventListener(window, "resize", this.onResize.bind(this));

    // Size (bust: fixed width; full: width 100% + aspect-ratio) is pure CSS — this just keeps
    // the renderer's pixel size and camera aspect in sync as the frame's resolved box changes.
    this.squareObserver = new ResizeObserver(() => this.onResize());
    this.squareObserver.observe(this);
    this.onResize();

    // Snapshot whatever portrait-update timestamp is already known for this player before
    // subscribing, so the replay-most-recent below (see pubsub.subscribe) doesn't trigger a
    // redundant reload of the mesh we're about to fetch anyway.
    if (this.playerName) {
      const [mostRecentTimestamp] = pubsub.getMostRecent(`portraitUpdated:${this.playerName}`) || [];
      this.lastLoadedPortraitTimestamp = mostRecentTimestamp ?? null;
      this.subscribe(`portraitUpdated:${this.playerName}`, this.handlePortraitUpdated.bind(this));
    }

    this.loadPortrait();

    if (this.mode === "full") {
      this.setupRotation();
    }
  }

  // Fires whenever the group-data poll reports a newer character_mesh upload (e.g. the plugin
  // re-captured the portrait after a gear/appearance change) so an already-open panel picks up
  // the new mesh without the user having to refresh the page.
  handlePortraitUpdated(timestamp) {
    if (timestamp === this.lastLoadedPortraitTimestamp) return;
    this.lastLoadedPortraitTimestamp = timestamp;
    this.loadPortrait();
  }

  disconnectedCallback() {
    this.squareObserver?.disconnect();
    this.squareObserver = null;
    this.mesh?.geometry?.dispose();
    this.mesh?.material?.dispose();
    this.renderer?.dispose();
    this.renderer = null;
    if (this.rotationFrameId) {
      cancelAnimationFrame(this.rotationFrameId);
      this.rotationFrameId = null;
    }
    clearTimeout(this.resumeAutoRotateTimeout);
    super.disconnectedCallback();
  }

  // Slow auto-rotate, pausable by a horizontal drag that resumes after a short delay.
  // Only wired up for "full" mode (/panels) — bust portraits stay static.
  setupRotation() {
    this.dragging = false;
    this.autoRotatePaused = false;
    this.lastPointerX = 0;
    this.lastFrameTime = performance.now();

    this.eventListener(this.frame, "pointerdown", this.handlePointerDown.bind(this), { passive: false });
    this.eventListener(this.frame, "pointermove", this.handlePointerMove.bind(this), { passive: false });
    this.eventListener(this.frame, "pointerup", this.handlePointerUp.bind(this));
    this.eventListener(this.frame, "pointercancel", this.handlePointerUp.bind(this));

    const tick = (now) => {
      const deltaSeconds = (now - this.lastFrameTime) / 1000;
      this.lastFrameTime = now;

      if (this.mesh && !this.dragging && !this.autoRotatePaused) {
        this.mesh.rotation.y += AUTO_ROTATE_RADIANS_PER_SECOND * deltaSeconds;
        this.renderFrame();
      }

      this.rotationFrameId = requestAnimationFrame(tick);
    };
    this.rotationFrameId = requestAnimationFrame(tick);
  }

  handlePointerDown(event) {
    if (!this.mesh) return;
    this.dragging = true;
    this.autoRotatePaused = true;
    this.lastPointerX = event.clientX;
    clearTimeout(this.resumeAutoRotateTimeout);
    this.frame.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  handlePointerMove(event) {
    if (!this.dragging || !this.mesh) return;
    const deltaX = event.clientX - this.lastPointerX;
    this.lastPointerX = event.clientX;
    this.mesh.rotation.y += deltaX * DRAG_RADIANS_PER_PIXEL;
    this.renderFrame();
    event.preventDefault();
  }

  handlePointerUp(event) {
    if (!this.dragging) return;
    this.dragging = false;
    if (this.frame.hasPointerCapture(event.pointerId)) {
      this.frame.releasePointerCapture(event.pointerId);
    }
    clearTimeout(this.resumeAutoRotateTimeout);
    this.resumeAutoRotateTimeout = setTimeout(() => {
      this.autoRotatePaused = false;
    }, AUTO_ROTATE_RESUME_DELAY_MS);
  }

  onResize() {
    if (!this.renderer) return;
    const width = this.frame.clientWidth || 1;
    const height = this.frame.clientHeight || 1;
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    this.renderFrame();
  }

  async loadPortrait() {
    let buffer = null;
    try {
      buffer = this.accountCharacterId
        ? await accountApi.getCharacterPortrait(this.accountCharacterId)
        : await api.getPortrait(this.playerName);
    } catch (ex) {
      console.error(`failed to load portrait for ${this.playerName || this.accountCharacterId}`, ex);
    }
    if (!this.isConnected) return;

    if (!buffer) {
      this.classList.add("player-portrait--no-portrait");
      return;
    }

    this.classList.remove("player-portrait--no-portrait");
    this.buildMesh(buffer);
    this.renderFrame();
  }

  buildMesh(buffer) {
    if (this.mesh) {
      // Replacing an already-loaded portrait (see handlePortraitUpdated) - drop the old mesh
      // rather than leaving it in the scene alongside the new one.
      this.scene.remove(this.mesh);
      this.mesh.geometry.dispose();
      this.mesh.material.dispose();
      this.mesh = null;
    }

    const geometry = new PLYLoader().parse(buffer);
    // RuneLite model space is X east, Y down, Z north. Negating Y and Z (not just Y) is a
    // 180-degree rotation about X, flipping the model upright with its front facing the camera.
    geometry.scale(1, -1, -1);
    geometry.center();
    geometry.computeBoundingSphere();
    geometry.computeBoundingBox();

    const material = new THREE.MeshBasicMaterial({ vertexColors: true });
    this.mesh = new THREE.Mesh(geometry, material);
    this.scene.add(this.mesh);

    if (this.mode === "bust") {
      this.fitCameraToBust(geometry);
    } else {
      this.fitCameraToGeometry(geometry);
    }
  }

  fitCameraToGeometry(geometry) {
    const sphere = geometry.boundingSphere;
    if (!sphere || sphere.radius <= 0) return;
    const distance = PlayerPortrait.fitCameraDistance(sphere.radius, this.camera.fov);
    this.camera.position.set(0, 0, distance);
    this.camera.lookAt(0, 0, 0);
    this.camera.updateProjectionMatrix();
  }

  // Frames just the head and shoulders: the top BUST_HEIGHT_FRACTION slice of the model,
  // measured down from the top of the body rather than the raw bounding-box top — a raised
  // two-handed weapon (spear, staff) extends well above the head, and using box.max.y directly
  // would frame the weapon tip instead of the character.
  fitCameraToBust(geometry) {
    const box = geometry.boundingBox;
    if (!box) return;
    const bodyTop = PlayerPortrait.findBodyTop(geometry, box);
    const totalHeight = bodyTop - box.min.y;
    if (totalHeight <= 0) return;
    const bustRadius = (totalHeight * BUST_HEIGHT_FRACTION) / 2;
    const viewY = bodyTop - bustRadius;
    const distance = PlayerPortrait.fitCameraDistance(bustRadius, this.camera.fov) * BUST_ZOOM_OUT_FACTOR;
    // Center horizontally on what's actually visible in the crop, not the whole model — a
    // weapon held out to one side pulls the model's overall x-center away from the head.
    const viewX = PlayerPortrait.findHorizontalCenter(geometry, viewY, distance, this.camera.fov);
    this.camera.position.set(viewX, viewY, distance);
    this.camera.lookAt(viewX, viewY, 0);
    this.camera.updateProjectionMatrix();
  }

  // Averages the x-extent of vertices within the vertical slice the camera will actually show,
  // so a weapon held out to one side (which skews the whole model's bounding-box center) doesn't
  // pull the crop off the character.
  static findHorizontalCenter(geometry, viewY, distance, fovDegrees) {
    const fovRadians = (fovDegrees * Math.PI) / 180;
    const halfExtentY = distance * Math.tan(fovRadians / 2);
    const yLow = viewY - halfExtentY;
    const yHigh = viewY + halfExtentY;

    const position = geometry.attributes.position;
    let xMin = Infinity;
    let xMax = -Infinity;
    for (let i = 0; i < position.count; i++) {
      const y = position.getY(i);
      if (y < yLow || y > yHigh) continue;
      const x = position.getX(i);
      if (x < xMin) xMin = x;
      if (x > xMax) xMax = x;
    }
    return isFinite(xMin) && isFinite(xMax) ? (xMin + xMax) / 2 : 0;
  }

  // Scans the model in horizontal bands and, starting from the top, returns the y of the first
  // band that's still "wide" (width + depth past BODY_GIRTH_FRACTION of the model's max girth) —
  // allowing a look behind at the BODY_GIRTH_LOOKBEHIND bands below it. A held weapon or raised
  // staff is thin enough to fall below that threshold, so this finds the top of the head/
  // shoulders rather than the weapon tip. Falls back to the raw bounding-box top if nothing
  // qualifies.
  static findBodyTop(geometry, box) {
    const position = geometry.attributes.position;
    const yMin = box.min.y;
    const totalHeight = box.max.y - yMin;
    if (totalHeight <= 0) return box.max.y;

    const bandMinX = new Array(BODY_GIRTH_SCAN_BANDS).fill(Infinity);
    const bandMaxX = new Array(BODY_GIRTH_SCAN_BANDS).fill(-Infinity);
    const bandMinZ = new Array(BODY_GIRTH_SCAN_BANDS).fill(Infinity);
    const bandMaxZ = new Array(BODY_GIRTH_SCAN_BANDS).fill(-Infinity);
    for (let i = 0; i < position.count; i++) {
      const x = position.getX(i);
      const z = position.getZ(i);
      let band = Math.floor(((position.getY(i) - yMin) / totalHeight) * BODY_GIRTH_SCAN_BANDS);
      if (band >= BODY_GIRTH_SCAN_BANDS) band = BODY_GIRTH_SCAN_BANDS - 1;
      if (band < 0) band = 0;
      if (x < bandMinX[band]) bandMinX[band] = x;
      if (x > bandMaxX[band]) bandMaxX[band] = x;
      if (z < bandMinZ[band]) bandMinZ[band] = z;
      if (z > bandMaxZ[band]) bandMaxZ[band] = z;
    }

    const girth = (band) => {
      const width = bandMaxX[band] - bandMinX[band];
      const depth = bandMaxZ[band] - bandMinZ[band];
      return isFinite(width) && isFinite(depth) ? width + depth : 0;
    };
    let maxGirth = 0;
    for (let band = 0; band < BODY_GIRTH_SCAN_BANDS; band++) maxGirth = Math.max(maxGirth, girth(band));
    const threshold = maxGirth * BODY_GIRTH_FRACTION;

    const lookbehindFloor = maxGirth * BODY_GIRTH_LOOKBEHIND_MIN_FRACTION;
    for (let band = BODY_GIRTH_SCAN_BANDS - 1; band >= 0; band--) {
      let widestNearby = 0;
      for (let k = 0; k <= BODY_GIRTH_LOOKBEHIND; k++) {
        const nearbyBand = band - k;
        if (nearbyBand >= 0) widestNearby = Math.max(widestNearby, girth(nearbyBand));
      }
      if (widestNearby >= threshold && girth(band) >= lookbehindFloor) {
        return yMin + (band / BODY_GIRTH_SCAN_BANDS) * totalHeight;
      }
    }
    return box.max.y;
  }

  static fitCameraDistance(radius, fovDegrees) {
    const fovRadians = (fovDegrees * Math.PI) / 180;
    return (radius * FRAMING_PADDING) / Math.sin(fovRadians / 2);
  }

  renderFrame() {
    if (!this.renderer) return;
    this.renderer.render(this.scene, this.camera);
  }
}

customElements.define("player-portrait", PlayerPortrait);

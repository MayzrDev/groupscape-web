import * as THREE from "three";
import { PLYLoader } from "three/examples/jsm/loaders/PLYLoader.js";
import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";
import { accountApi } from "../data/account-api";

const FOV_DEGREES = 30;
// Headroom above a tight bounding-sphere fit so the model doesn't touch the frame edges.
const FRAMING_PADDING = 1.22;
// Fraction of the model's total height treated as "head and shoulders" for the bust crop.
const BUST_HEIGHT_FRACTION = 0.32;
// Shifts the bust view downward by this fraction of the bust radius, which pushes the head
// closer to the top of the frame instead of sitting dead-center.
const BUST_VERTICAL_SHIFT_FRACTION = 0.06;
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

    this.loadPortrait();

    if (this.mode === "full") {
      this.setupRotation();
    }
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

    this.buildMesh(buffer);
    this.renderFrame();
  }

  buildMesh(buffer) {
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

  // Frames just the head and shoulders: the top BUST_HEIGHT_FRACTION slice of the model's
  // (already-centered) bounding box, reusing the same vertical-FOV distance formula as the
  // full-body fit.
  fitCameraToBust(geometry) {
    const box = geometry.boundingBox;
    if (!box) return;
    const totalHeight = box.max.y - box.min.y;
    if (totalHeight <= 0) return;
    const bustRadius = (totalHeight * BUST_HEIGHT_FRACTION) / 2;
    const centerY = box.max.y - bustRadius;
    const viewY = centerY - bustRadius * BUST_VERTICAL_SHIFT_FRACTION;
    const distance = PlayerPortrait.fitCameraDistance(bustRadius, this.camera.fov);
    this.camera.position.set(0, viewY, distance);
    this.camera.lookAt(0, viewY, 0);
    this.camera.updateProjectionMatrix();
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

import * as THREE from "three";
import { PLYLoader } from "three/examples/jsm/loaders/PLYLoader.js";
import { BaseElement } from "../base-element/base-element";
import { api } from "../data/api";

const FOV_DEGREES = 30;
// Headroom above a tight bounding-sphere fit so the model doesn't touch the frame edges.
const FRAMING_PADDING = 1.22;
// Full-body framing: taller than wide, matching the approved player-panel placement.
const FRAME_ASPECT_RATIO = 3 / 5;

export class PlayerPortrait extends BaseElement {
  html() {
    return `{{player-portrait.html}}`;
  }

  connectedCallback() {
    super.connectedCallback();
    this.playerName = this.getAttribute("player-name");
    this.render();

    this.frame = this.querySelector(".player-portrait__frame");
    this.canvas = this.querySelector("canvas");

    this.renderer = new THREE.WebGLRenderer({ canvas: this.canvas, alpha: true });
    this.camera = new THREE.PerspectiveCamera(FOV_DEGREES, 1, 1, 10000);
    this.camera.position.set(0, 0, 220);
    this.scene = new THREE.Scene();

    this.eventListener(window, "resize", this.onResize.bind(this));

    // In a flex row (e.g. the player-panel header), `aspect-ratio` doesn't reliably derive
    // width from a stretch-resolved height, so the frame can end up taller than it is wide.
    // Correct it in JS once layout settles so the portrait keeps its full-body framing.
    this.squareObserver = new ResizeObserver(() => this.enforceAspectRatio());
    this.squareObserver.observe(this);
    this.enforceAspectRatio();

    this.onResize();

    this.loadPortrait();
  }

  disconnectedCallback() {
    this.squareObserver?.disconnect();
    this.squareObserver = null;
    this.mesh?.geometry?.dispose();
    this.mesh?.material?.dispose();
    this.renderer?.dispose();
    this.renderer = null;
    super.disconnectedCallback();
  }

  enforceAspectRatio() {
    const height = this.offsetHeight;
    const targetWidth = Math.round(height * FRAME_ASPECT_RATIO);
    if (height > 0 && this.offsetWidth !== targetWidth) {
      this.style.width = `${targetWidth}px`;
      this.onResize();
    }
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
      buffer = await api.getPortrait(this.playerName);
    } catch (ex) {
      console.error(`failed to load portrait for ${this.playerName}`, ex);
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

    const material = new THREE.MeshBasicMaterial({ vertexColors: true });
    this.mesh = new THREE.Mesh(geometry, material);
    this.scene.add(this.mesh);

    this.fitCameraToGeometry(geometry);
  }

  fitCameraToGeometry(geometry) {
    const sphere = geometry.boundingSphere;
    if (!sphere || sphere.radius <= 0) return;
    const distance = PlayerPortrait.fitCameraDistance(sphere.radius, this.camera.fov);
    this.camera.position.set(0, 0, distance);
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

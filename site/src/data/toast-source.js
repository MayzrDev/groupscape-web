import { api } from "./api";
import { pubsub } from "./pubsub";
import { utility } from "../utility";

const POLL_INTERVAL_MS = 15000;
const FETCH_LIMIT = 20;

// The server persists individual combat-achievement task completions under the `combat_task`
// event type, but the toast styling/icon for them has always lived under the `combat-achievement`
// key. Reuse it rather than defining a second purple.
const TOAST_TYPE_BY_EVENT_TYPE = {
  combat_task: "combat-achievement",
};

function toastTypeFor(eventType) {
  return TOAST_TYPE_BY_EVENT_TYPE[eventType] || eventType;
}

class ToastSource {
  constructor() {
    this.enabled = false;
    this.lastEventId = 0;
    this.sawFirstPoll = false;
  }

  enable() {
    if (this.enabled) return;
    this.enabled = true;
    this.lastEventId = 0;
    this.sawFirstPoll = false;
    this.interval = utility.callOnInterval(this.poll.bind(this), POLL_INTERVAL_MS, true);
  }

  disable() {
    this.enabled = false;
    if (this.interval) window.clearInterval(this.interval);
    this.interval = undefined;
  }

  async poll() {
    if (!this.enabled) return;

    const events = await api.getActivityEvents({ limit: FETCH_LIMIT });
    if (events.length === 0) return;

    const sorted = [...events].sort((a, b) => a.id - b.id);
    // The first poll after enabling establishes a baseline (matching group-data.js's `nextCheck`
    // pattern) so a page load doesn't replay every kill/death from before the session started.
    const newEvents = this.sawFirstPoll ? sorted.filter((event) => event.id > this.lastEventId) : [];
    this.lastEventId = sorted[sorted.length - 1].id;
    this.sawFirstPoll = true;

    for (const event of newEvents) {
      pubsub.publish("toast", { type: toastTypeFor(event.event_type), event });
    }
  }
}

const toastSource = new ToastSource();

export { toastSource };

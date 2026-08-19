import { api } from "./api";
import { pubsub } from "./pubsub";
import { utility } from "../utility";

const POLL_INTERVAL_MS = 15000;
const FETCH_LIMIT = 20;

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
      pubsub.publish("toast", { type: event.event_type, event });
    }
  }
}

const toastSource = new ToastSource();

export { toastSource };

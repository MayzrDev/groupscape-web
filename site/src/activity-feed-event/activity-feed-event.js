import { BaseElement } from "../base-element/base-element";
import { activityBadgeLabel, activityDisplayType, activityEventDescription } from "../data/activity-event-copy";
import { Item } from "../data/item";
import { groupData } from "../data/group-data";
import { api } from "../data/api";
import { REACTION_ICONS, REACTION_LABELS, RADIAL_REACTION_ORDER } from "./reaction-icons";

// Press-and-hold this long on the Like button opens the radial popup of the other 4 reaction
// types (§ product spec) - short enough to feel responsive, long enough that a quick tap never
// accidentally opens it.
const LONG_PRESS_MS = 350;

const RELATIVE_UNITS = [
  ["y", 31536000],
  ["mo", 2592000],
  ["d", 86400],
  ["h", 3600],
  ["m", 60],
];

function formatRelativeTime(date) {
  const seconds = Math.floor((Date.now() - date.getTime()) / 1000);
  if (seconds < 60) return "just now";

  for (const [suffix, unitSeconds] of RELATIVE_UNITS) {
    const value = Math.floor(seconds / unitSeconds);
    if (value >= 1) return `${value}${suffix} ago`;
  }
  return "just now";
}

export class ActivityFeedEvent extends BaseElement {
  constructor() {
    super();
    // Set by activity-feed-page.js before/after this row is created - whether the group has
    // likes/comments turned on at all. `reactionData`/`commentCount` start empty and are filled in
    // by the page's periodic `refreshReactions` poll (see activity-feed-page.js) - reactions never
    // come back from get-activity-events itself.
    this.reactionsEnabled = false;
    this.reactionData = { reactions: [], my_reaction: null };
    this.commentCount = 0;
    this.commentsOpen = false;
    this.comments = null;
    this.radialOpen = false;
  }

  html() {
    return `{{activity-feed-event.html}}`;
  }

  reactionIcon(kind) {
    return REACTION_ICONS[kind] || "";
  }

  // Overridden (rather than relying on BaseElement's plain innerHTML-replace) so that
  // activity-feed-page.js's mergeOrCreateRow, which calls `row.render()` directly on a repeat
  // kill/death to refresh the aggregated count, also gets the reaction DOM rebuilt and rebound -
  // otherwise a re-render would wipe the reaction/comment markup and silently lose its listeners.
  render() {
    super.render();
    this.bindReactionElements();
  }

  connectedCallback() {
    super.connectedCallback();
    if (this.displayType === "clue" && this.event.payload?.clueTier) {
      this.style.setProperty("--clue-tier-color", `var(--clue-${this.event.payload.clueTier})`);
    }
    this.render();
  }

  disconnectedCallback() {
    super.disconnectedCallback();
    if (this.longPressTimer) clearTimeout(this.longPressTimer);
  }

  // Re-run after every render() (kill/death merges call render() again on the same instance, see
  // activity-feed-page.js's mergeOrCreateRow) so the freshly rebuilt reaction DOM gets its
  // listeners back and reflects whatever reaction/comment state was already known before the
  // re-render wiped the markup.
  bindReactionElements() {
    if (!this.reactionsEnabled) return;

    this.likeButton = this.querySelector(".activity-feed-event__like-btn");
    this.radial = this.querySelector(".activity-feed-event__radial");
    this.pillsContainer = this.querySelector(".activity-feed-event__pills");
    this.commentButton = this.querySelector(".activity-feed-event__comment-btn");
    this.commentCountEl = this.querySelector(".activity-feed-event__comment-count");
    this.commentsPanel = this.querySelector(".activity-feed-event__comments-panel");
    this.commentsList = this.querySelector(".activity-feed-event__comments-list");
    this.commentInput = this.querySelector(".activity-feed-event__comment-input");
    this.commentSendButton = this.querySelector(".activity-feed-event__comment-send");
    this.commentsError = this.querySelector(".activity-feed-event__comments-error");
    this.commentsIndicator = this.querySelector(".activity-feed-event__comments-indicator");

    if (this.likeButton) {
      this.eventListener(this.likeButton, "pointerdown", this.handleLikePointerDown.bind(this), { passive: false });
      this.eventListener(this.likeButton, "pointermove", this.handleLikePointerMove.bind(this));
      this.eventListener(this.likeButton, "pointerup", this.handleLikePointerUp.bind(this));
      this.eventListener(this.likeButton, "pointercancel", this.handleLikePointerCancel.bind(this));
    }
    this.eventListener(this.commentButton, "click", this.toggleComments.bind(this));
    this.eventListener(this.commentSendButton, "click", this.submitComment.bind(this));
    this.eventListener(this.commentInput, "keydown", (e) => {
      if (e.key === "Enter") this.submitComment();
    });

    this.renderReactionSummary();
    this.updateCommentCount(this.commentCount);
    if (this.commentsOpen) this.openCommentsPanel({ skipToggleLoad: true });
  }

  // Called by activity-feed-page.js's refreshReactions poll with the latest server state for
  // this event - never derived locally, since reactions/comments can come from any group member.
  applyReactionSummary(summary) {
    if (!summary) return;
    this.reactionData = { reactions: summary.reactions || [], my_reaction: summary.my_reaction || null };
    if (typeof summary.comment_count === "number") this.commentCount = summary.comment_count;
    this.renderReactionSummary();
    this.updateCommentCount(this.commentCount);
  }

  renderReactionSummary() {
    if (!this.pillsContainer) return;
    this.pillsContainer.innerHTML = "";
    for (const { reaction, count } of this.reactionData.reactions) {
      if (count < 1) continue;
      const pill = document.createElement("span");
      pill.className = "activity-feed-event__pill";
      if (reaction === this.reactionData.my_reaction) pill.classList.add("activity-feed-event__pill--mine");
      pill.title = REACTION_LABELS[reaction] || reaction;
      pill.innerHTML = `${this.reactionIcon(reaction)}<span class="activity-feed-event__pill-count">${count}</span>`;
      this.pillsContainer.appendChild(pill);
    }
    if (this.likeButton) {
      this.likeButton.classList.toggle("activity-feed-event__like-btn--active", Boolean(this.reactionData.my_reaction));
    }
  }

  updateCommentCount(count) {
    this.commentCount = count;
    if (this.commentCountEl) this.commentCountEl.textContent = String(count);
    if (this.commentsIndicator) this.commentsIndicator.textContent = `${count} / 10 comments`;
    if (this.commentInput) this.commentInput.disabled = count >= 10;
    if (this.commentSendButton) this.commentSendButton.disabled = count >= 10;
  }

  handleLikePointerDown(e) {
    e.preventDefault();
    this.likeButton.setPointerCapture(e.pointerId);
    this.longPressFired = false;
    this.longPressTimer = setTimeout(() => {
      this.longPressFired = true;
      this.openRadial();
    }, LONG_PRESS_MS);
  }

  handleLikePointerMove(e) {
    if (!this.radialOpen) return;
    this.updateRadialHover(e.clientX, e.clientY);
  }

  handleLikePointerUp() {
    clearTimeout(this.longPressTimer);
    if (this.radialOpen) {
      const target = this.radialHoverTarget;
      this.closeRadial();
      if (target) this.sendReaction(target);
      return;
    }
    if (!this.longPressFired) {
      this.sendReaction("like");
    }
  }

  handleLikePointerCancel() {
    clearTimeout(this.longPressTimer);
    this.closeRadial();
  }

  openRadial() {
    if (!this.radial) return;
    this.radialOpen = true;
    this.radial.innerHTML = "";
    RADIAL_REACTION_ORDER.forEach((reaction, index) => {
      const option = document.createElement("div");
      option.className = `activity-feed-event__radial-option activity-feed-event__radial-option--${index}`;
      option.dataset.reaction = reaction;
      option.title = REACTION_LABELS[reaction];
      option.innerHTML = this.reactionIcon(reaction);
      this.radial.appendChild(option);
    });
    this.radial.hidden = false;
  }

  // Touch/mouse drag off the Like button keeps delivering pointermove/pointerup to it (pointer
  // capture, set in handleLikePointerDown) rather than the element the finger/cursor is actually
  // over, so hover detection has to hit-test manually via elementFromPoint instead of relying on
  // native pointerenter/pointerleave on the radial options.
  updateRadialHover(x, y) {
    const el = document.elementFromPoint(x, y);
    const option = el?.closest(".activity-feed-event__radial-option");
    const reaction = option?.dataset.reaction || null;
    if (reaction === this.radialHoverTarget) return;
    this.radialHoverTarget = reaction;
    for (const child of this.radial.children) {
      child.classList.toggle("activity-feed-event__radial-option--hovered", child.dataset.reaction === reaction);
    }
  }

  closeRadial() {
    this.radialOpen = false;
    this.radialHoverTarget = null;
    if (this.radial) {
      this.radial.hidden = true;
      this.radial.innerHTML = "";
    }
  }

  async sendReaction(reaction) {
    const response = await api.reactToActivityEvent(this.event.id, reaction);
    if (!response.ok) return;
    const summary = await response.json();
    this.applyReactionSummary({ ...summary, comment_count: this.commentCount });
  }

  toggleComments() {
    if (this.commentsOpen) {
      this.commentsOpen = false;
      if (this.commentsPanel) this.commentsPanel.hidden = true;
      return;
    }
    this.openCommentsPanel();
  }

  async openCommentsPanel({ skipToggleLoad = false } = {}) {
    this.commentsOpen = true;
    if (this.commentsPanel) this.commentsPanel.hidden = false;
    if (skipToggleLoad && this.comments) {
      this.renderComments();
      return;
    }
    await this.loadComments();
  }

  async loadComments() {
    const page = await api.getActivityComments(this.event.id);
    this.comments = page.comments || [];
    this.updateCommentCount(page.comment_count ?? this.comments.length);
    this.renderComments();
  }

  renderComments() {
    if (!this.commentsList) return;
    this.commentsList.innerHTML = "";
    for (const comment of this.comments || []) {
      const row = document.createElement("div");
      row.className = "activity-feed-event__comment";
      const nameHasIcon = groupData.members.has(comment.member_name);
      row.innerHTML = `
        <span class="activity-feed-event__comment-author">
          ${nameHasIcon ? `<player-icon player-name="${comment.member_name}"></player-icon>` : ""}${comment.member_name}
        </span>
        <span class="activity-feed-event__comment-text"></span>
      `;
      // Set as text, not innerHTML, so a comment can never inject markup into the feed.
      row.querySelector(".activity-feed-event__comment-text").textContent = comment.comment_text;
      this.commentsList.appendChild(row);
    }
  }

  async submitComment() {
    if (!this.commentInput || this.commentInput.disabled) return;
    const text = this.commentInput.value.trim();
    if (!text) return;
    if (this.commentsError) this.commentsError.textContent = "";

    const response = await api.addActivityComment(this.event.id, text);
    if (!response.ok) {
      const message = await response.text();
      if (this.commentsError) this.commentsError.textContent = message || "Failed to post comment";
      return;
    }
    const page = await response.json();
    this.comments = page.comments || [];
    this.commentInput.value = "";
    this.updateCommentCount(page.comment_count ?? this.comments.length);
    this.renderComments();
  }

  get occurredAt() {
    return new Date(this.event.occurred_at);
  }

  get relativeTime() {
    return formatRelativeTime(this.occurredAt);
  }

  get loot() {
    // A merged raid completion has no top-level `payload.loot` - each reporting member's share
    // lives under its own `participants[].loot` entry instead (see `RaidCompletionPayload`).
    const loot = ["cox", "tob", "toa"].includes(this.displayType)
      ? (this.event.payload?.participants || []).flatMap((p) => p.loot || [])
      : this.event.payload?.loot || [];
    // Same "unrecognized item" filter loot-log-group.js applies before rendering an item-box -
    // an item id with no local catalog entry (not yet added, or a bad drop report) would otherwise
    // throw inside item-box's Item.imageUrl() and take the whole row's render() down with it.
    return loot.filter((entry) => Item.exists(entry.item_id));
  }

  get displayType() {
    return activityDisplayType(this.event);
  }

  get badgeLabel() {
    return activityBadgeLabel(this.event);
  }

  descriptionHtml() {
    return activityEventDescription(this.event, {
      member: (name) => {
        // Merged raid completions wrap a joined name list ("A, B, and C") through this same
        // hook (see activityEventDescription's "cox"/"tob"/"toa" branch) - that's never a real
        // member key, so the icon only renders for the single-member case.
        const icon = groupData.members.has(name) ? `<player-icon player-name="${name}"></player-icon>` : "";
        return `${icon}<span class="activity-feed-event__member">${name}</span>`;
      },
      subject: (text, variant, wikiUrl, iconSrc, secondaryIconSrc) => {
        const cls = `activity-feed-event__subject${variant === "death" ? " activity-feed-event__subject--death" : ""}${
          variant === "clue" ? " activity-feed-event__subject--clue" : ""
        }`;
        // A second icon (currently only the collection-log item case) means the first icon leads
        // the name instead of trailing it - e.g. "<item icon> Fire element staff crown <log icon>".
        const leadingSrc = secondaryIconSrc ? iconSrc : null;
        const trailingSrc = secondaryIconSrc || iconSrc;
        const img = (src, modifier) =>
          src ? `<img class="activity-feed-event__subject-icon${modifier}" src="${src}" alt="" />` : "";
        const content = `${img(leadingSrc, " activity-feed-event__subject-icon--leading")}${text}${img(
          trailingSrc,
          ""
        )}`;
        return wikiUrl
          ? `<a href="${wikiUrl}" target="_blank" rel="noopener" class="${cls}">${content}</a>`
          : `<span class="${cls}">${content}</span>`;
      },
    });
  }
}

customElements.define("activity-feed-event", ActivityFeedEvent);

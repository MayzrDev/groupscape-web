// Small inline SVGs for the 5 activity-feed reaction types, styled to read as OSRS-flavored icons
// without needing real wiki sprite assets (see CLAUDE.md's OSRS icon-source note - none of these
// map to an existing wiki icon closely enough to be worth fetching one). Each is a flat 16x16
// viewBox so they drop straight into a button/pill at any size via CSS.
const REACTION_ICONS = {
  // Coins + a check mark - "liked/approved", coin stack being the game's universal positive icon.
  like: `<svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
    <ellipse cx="6" cy="10.5" rx="5" ry="2.5" fill="currentColor" opacity="0.35"/>
    <ellipse cx="6" cy="9" rx="5" ry="2.5" fill="currentColor"/>
    <path d="M10.5 5.5 12 7l3-3.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>
  </svg>`,
  // Laurel wreath around a star - "well played", borrowing the achievement-diary laurel motif.
  gg: `<svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
    <path d="M3 13c-1.2-2-1.4-5 .2-8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
    <path d="M13 13c1.2-2 1.4-5-.2-8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
    <path d="M3.6 6.2 2.4 5.4M4.4 8.6 3 8.3M5 11 3.7 11.4M12.4 6.2l1.2-.8M11.6 8.6l1.4-.3M11 11l1.3.4" stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>
    <path d="M8 2.3 9 5l2.8.2-2.2 1.8.7 2.7L8 8.2 5.7 9.7l.7-2.7L4.2 5.2 7 5z" fill="currentColor"/>
  </svg>`,
  // Laughing face - "funny", plain and legible at pill size.
  lol: `<svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
    <circle cx="8" cy="8" r="6.3" fill="currentColor" opacity="0.18"/>
    <circle cx="8" cy="8" r="6.3" stroke="currentColor" stroke-width="1.2"/>
    <path d="M5.4 6.4c.3-.5.8-.8 1.3-.8M9.3 6.4c.3-.5.8-.8 1.3-.8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
    <path d="M4.8 9c.6 1.6 2 2.6 3.2 2.6s2.6-1 3.2-2.6c.1-.3-.1-.6-.5-.6H5.3c-.4 0-.6.3-.5.6Z" fill="currentColor"/>
  </svg>`,
  // 4-point sparkle - "rare drop", matching the twinkle used on unique-drop item highlights.
  rare: `<svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
    <path d="M8 1.5c.3 2.4 1 4 2 5s2.6 1.7 5 2c-2.4.3-4 1-5 2s-1.7 2.6-2 5c-.3-2.4-1-4-2-5s-2.6-1.7-5-2c2.4-.3 4-1 5-2s1.7-2.6 2-5Z" fill="currentColor"/>
    <path d="M13 2.2c.1 1 .4 1.6.8 2 .4.4 1 .7 2 .8-1 .1-1.6.4-2 .8-.4.4-.7 1-.8 2-.1-1-.4-1.6-.8-2-.4-.4-1-.7-2-.8 1-.1 1.6-.4 2-.8.4-.4.7-1 .8-2Z" fill="currentColor" opacity="0.7"/>
  </svg>`,
  // Headstone - "f" (RIP), matching the death badge's tone elsewhere in the feed.
  f: `<svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
    <path d="M4 14V7.5C4 4.5 5.8 2.5 8 2.5s4 2 4 5V14" fill="currentColor" opacity="0.85"/>
    <path d="M4 14V7.5C4 4.5 5.8 2.5 8 2.5s4 2 4 5V14" stroke="currentColor" stroke-width="1"/>
    <path d="M7.1 5.6h1.8M8 4.7v1.8" stroke="var(--rsbackground, #3e3529)" stroke-width="1.1" stroke-linecap="round"/>
    <path d="M2.8 14h10.4" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
  </svg>`,
};

const REACTION_LABELS = {
  like: "Like",
  gg: "GG",
  lol: "LOL",
  rare: "Rare!",
  f: "F",
};

// Tap order for the radial popup - like sits on the trigger button itself, so only the other 4
// need positions around it.
const RADIAL_REACTION_ORDER = ["gg", "lol", "rare", "f"];

export { REACTION_ICONS, REACTION_LABELS, RADIAL_REACTION_ORDER };

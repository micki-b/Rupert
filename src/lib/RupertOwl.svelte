<script lang="ts" module>
  export type OwlMood = "idle" | "thinking" | "searching" | "studying" | "opening" | "happy" | "puzzled";
</script>

<script lang="ts">
  // Rupert, cut out of the app icon, acting out what the assistant does.
  // The artwork is the icon's own; only his eyes are drawn on top (over
  // the painted ones), so they can blink, look around and read. A small
  // badge names the task. Still under "reduce motion". In Spanish he wears
  // a sky-blue and white striped jersey (#19): the picture is then drawn in
  // an SVG with the jersey's area cut out, so nothing shows through where
  // both fade out at the bottom.
  import owlUrl from "$lib/assets/rupert-owl.png";
  import { currentLang, m } from "$lib/i18n";

  let { mood = "idle", size = 44 }: { mood?: OwlMood; size?: number } = $props();

  const uid = $props.id();
  /** The shirt: from the shoulders, along the bottom of his face, down to
   * the picture's faded edge (in the picture's 950 × 1000 coordinates). */
  const SHIRT =
    "M185 626 C260 662 385 702 475 722 C565 702 690 662 767 626 C800 628 838 636 868 660 C905 700 925 780 933 860 L936 1000 L14 1000 L17 860 C25 780 45 700 82 660 C112 636 150 628 185 626 Z";
  const outfit = $derived(currentLang() === "es" ? "jersey" : null);

</script>

<div class="owl mood-{mood}" style:width="{size}px" style:height="{size}px" role="img" aria-label={m.owl[mood]}>
  <div class="body">
    {#if outfit === "jersey"}
      <svg class="outfit" viewBox="0 0 950 1000" aria-hidden="true">
        <defs>
          <pattern id="stripes-{uid}" patternUnits="userSpaceOnUse" width="168" height="1000" x="391">
            <rect width="84" height="1000" fill="#fbfcfe" />
            <rect x="84" width="84" height="1000" fill="#74acdf" />
          </pattern>
          <linearGradient id="fade-{uid}" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0.87" stop-color="#fff" />
            <stop offset="0.995" stop-color="#fff" stop-opacity="0" />
          </linearGradient>
          <linearGradient id="volume-{uid}" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0" stop-color="#1b2a4a" stop-opacity="0.22" />
            <stop offset="0.2" stop-color="#1b2a4a" stop-opacity="0.03" />
            <stop offset="0.8" stop-color="#1b2a4a" stop-opacity="0.03" />
            <stop offset="1" stop-color="#1b2a4a" stop-opacity="0.22" />
          </linearGradient>
          <mask id="fade-mask-{uid}" maskUnits="userSpaceOnUse" x="0" y="0" width="950" height="1000">
            <rect width="950" height="1000" fill="url(#fade-{uid})" />
          </mask>
          <mask id="no-shirt-{uid}" maskUnits="userSpaceOnUse" x="0" y="0" width="950" height="1000">
            <rect width="950" height="1000" fill="#fff" />
            <path d={SHIRT} fill="#000" />
          </mask>
        </defs>
        <image href={owlUrl} width="950" height="1000" mask="url(#no-shirt-{uid})" />
        <g mask="url(#fade-mask-{uid})">
          <path d={SHIRT} fill="url(#stripes-{uid})" />
          <path d={SHIRT} fill="url(#volume-{uid})" />
          <!-- Sleeves over the wings: a little shade and a seam -->
          <path
            d="M205 612 C282 616 326 684 321 782 C317 862 300 934 282 1000 L14 1000 L17 860 C25 740 95 616 205 612 Z M745 612 C668 616 624 684 629 782 C633 862 650 934 668 1000 L936 1000 L933 860 C925 740 855 616 745 612 Z"
            fill="#1b2a4a"
            fill-opacity="0.08"
          />
          <path
            d="M236 622 C298 646 326 704 321 782 C317 862 300 934 282 1000 M714 622 C652 646 624 704 629 782 C633 862 650 934 668 1000"
            fill="none"
            stroke="#1b2a4a"
            stroke-opacity="0.22"
            stroke-width="5"
            stroke-linecap="round"
          />
        </g>
        <!-- The collar, along the bottom of his face -->
        <path
          d="M150 632 C200 628 230 640 262 660 C335 700 400 712 475 724 C550 712 615 700 688 660 C720 640 750 628 800 632"
          fill="none"
          stroke="#1f3561"
          stroke-width="20"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </svg>
    {:else}
      <img src={owlUrl} alt="" draggable="false" />
    {/if}
    <!-- Eyes, in the cut-out image's own coordinates (950 × 1000) -->
    <svg class="eyes" viewBox="0 0 950 1000" aria-hidden="true">
      <defs>
        <radialGradient id="pupil-{uid}" cx="40%" cy="35%" r="70%">
          <stop offset="0" stop-color="#5a3322" />
          <stop offset="1" stop-color="#2a150c" />
        </radialGradient>
        <radialGradient id="face-{uid}">
          <stop offset="0.8" stop-color="#efcea5" />
          <stop offset="1" stop-color="#efcea5" stop-opacity="0" />
        </radialGradient>
      </defs>
      <!-- Paint over the drawn pupils, then draw movable ones. -->
      <circle cx="335.9" cy="454" r="88" fill="url(#face-{uid})" />
      <circle cx="615.9" cy="454.7" r="88" fill="url(#face-{uid})" />
      <g class="pupils">
        <circle cx="335.9" cy="454" r="70" fill="url(#pupil-{uid})" />
        <circle cx="615.9" cy="454.7" r="70" fill="url(#pupil-{uid})" />
        <circle cx="360" cy="426" r="16" fill="#fff" />
        <circle cx="640" cy="426" r="16" fill="#fff" />
      </g>
      <path
        class="closed"
        d="M272 474 Q336 396 400 474 M552 474 Q616 396 680 474"
        fill="none"
        stroke="#3a1d0e"
        stroke-width="18"
        stroke-linecap="round"
      />
    </svg>
  </div>

  <span class="badge" aria-hidden="true">
    <svg viewBox="0 0 24 24" fill="none" stroke-linecap="round" stroke-linejoin="round">
      {#if mood === "thinking"}
        <circle class="dot" cx="6" cy="12" r="2.3" />
        <circle class="dot" cx="12" cy="12" r="2.3" />
        <circle class="dot" cx="18" cy="12" r="2.3" />
      {:else if mood === "searching"}
        <circle cx="10" cy="10" r="6" stroke-width="2.4" />
        <path d="M14.5 14.5 L20 20" stroke-width="2.6" />
      {:else if mood === "studying"}
        <path d="M3 6 Q7.5 4 12 6.5 Q16.5 4 21 6 V18.5 Q16.5 16.5 12 19 Q7.5 16.5 3 18.5 Z M12 6.5 V19" stroke-width="2" />
      {:else if mood === "opening"}
        <path d="M3 7 a1 1 0 0 1 1-1 h5 l2 2 h9 a1 1 0 0 1 1 1 v9 a1 1 0 0 1-1 1 H4 a1 1 0 0 1-1-1 Z" stroke-width="2" />
      {:else if mood === "happy"}
        <path class="ok" d="M5 12.5 L10 17 L19 7" stroke-width="2.8" />
      {:else if mood === "puzzled"}
        <path class="warn" d="M8.5 9 Q8.5 5 12 5 Q15.5 5 15.5 8.5 Q15.5 11 12 12.5 V14.5" stroke-width="2.6" />
        <circle class="warn-dot" cx="12" cy="19" r="1.2" />
      {/if}
    </svg>
  </span>
</div>

<style>
  .owl {
    position: relative;
    flex: none;
    animation: arrive 0.35s ease-out;
  }
  .body {
    position: absolute;
    inset: 0;
    transform-origin: 50% 95%;
  }
  .body img,
  .outfit,
  .eyes {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }
  .body img {
    object-fit: contain;
    user-select: none;
    -webkit-user-drag: none;
  }
  .eyes {
    overflow: visible;
  }
  .eyes * {
    transform-box: fill-box;
    transform-origin: center;
  }
  .pupils,
  .closed {
    transition:
      opacity 0.2s,
      transform 0.3s;
  }
  .closed {
    opacity: 0;
  }
  .mood-happy .pupils {
    opacity: 0;
  }
  .mood-happy .closed {
    opacity: 1;
  }

  /* The task badge */
  .badge {
    position: absolute;
    right: -6%;
    bottom: 0;
    width: 42%;
    height: 42%;
    border-radius: 50%;
    background: var(--bg);
    border: 1.5px solid var(--accent);
    display: grid;
    place-items: center;
    opacity: 0;
    transform: scale(0.6);
    transition:
      opacity 0.25s,
      transform 0.25s;
  }
  .owl:not(.mood-idle) .badge {
    opacity: 1;
    transform: scale(1);
  }
  .badge svg {
    width: 64%;
    height: 64%;
    stroke: var(--accent);
  }
  .badge .dot {
    fill: var(--accent);
    stroke: none;
    animation: pulse 1.5s ease-in-out infinite;
  }
  .badge .dot:nth-child(2) {
    animation-delay: 0.25s;
  }
  .badge .dot:nth-child(3) {
    animation-delay: 0.5s;
  }
  .badge .ok {
    stroke: #4f9a5a;
  }
  .badge .warn {
    stroke: #c26f17;
  }
  .badge .warn-dot {
    fill: #c26f17;
    stroke: none;
  }

  /* Body language */
  .mood-idle .body {
    animation: sway 5s ease-in-out infinite;
  }
  .mood-thinking .body {
    animation: ponder 3.2s ease-in-out infinite;
  }
  .mood-searching .body {
    animation: breathe 4s ease-in-out infinite;
  }
  .mood-studying .body {
    animation: nod 1.3s ease-in-out infinite;
  }
  .mood-opening .body {
    animation: turn 1.2s ease-in-out infinite;
  }
  .mood-happy .body {
    animation: squash 0.8s ease-out infinite;
  }
  .mood-puzzled .body {
    animation: puzzle 2.4s ease-in-out infinite;
  }

  /* The eyes */
  .mood-idle .pupils {
    animation: blink 5s ease-in-out 1s infinite;
  }
  .mood-thinking .pupils {
    animation: look-up 2.4s ease-in-out infinite;
  }
  .mood-searching .pupils {
    animation: scan 3.2s infinite;
  }
  .mood-studying .pupils {
    animation: read 2.4s steps(1) infinite;
  }
  .mood-opening .pupils {
    transform: translate(22px, 24px);
  }
  .mood-puzzled .pupils {
    transform: translate(-20px, -22px);
  }

  @keyframes arrive {
    from {
      opacity: 0;
      transform: scale(0.85);
    }
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 0.25;
    }
    50% {
      opacity: 1;
    }
  }
  @keyframes sway {
    0%,
    100% {
      transform: rotate(-1.5deg);
    }
    50% {
      transform: rotate(1.5deg) scale(1.01);
    }
  }
  @keyframes breathe {
    50% {
      transform: scale(1.02);
    }
  }
  @keyframes ponder {
    0%,
    100% {
      transform: rotate(0);
    }
    40%,
    60% {
      transform: rotate(-6deg) translateY(-1%);
    }
  }
  @keyframes nod {
    0%,
    100% {
      transform: scale(1.05) translateY(2%) rotate(0);
    }
    50% {
      transform: scale(1.05) translateY(4%) rotate(2deg);
    }
  }
  @keyframes turn {
    0%,
    50%,
    100% {
      transform: scaleX(1) translateY(0);
    }
    25% {
      transform: scaleX(0.92) translateY(-4%);
    }
  }
  @keyframes squash {
    0%,
    100% {
      transform: scale(1, 1);
    }
    20% {
      transform: scale(1.08, 0.9);
    }
    45% {
      transform: scale(0.94, 1.08) translateY(-8%);
    }
    70% {
      transform: scale(1.03, 0.97);
    }
  }
  @keyframes puzzle {
    0%,
    100% {
      transform: rotate(9deg);
    }
    50% {
      transform: rotate(6deg);
    }
  }
  @keyframes blink {
    0%,
    94%,
    100% {
      transform: scaleY(1);
    }
    97% {
      transform: scaleY(0.12);
    }
  }
  @keyframes look-up {
    0%,
    100% {
      transform: translate(0, 0);
    }
    30%,
    70% {
      transform: translate(22px, -26px);
    }
  }
  @keyframes read {
    0% {
      transform: translate(-26px, 14px);
    }
    25% {
      transform: translate(0, 14px);
    }
    50% {
      transform: translate(26px, 14px);
    }
    75% {
      transform: translate(-26px, 28px);
    }
  }
  /* Scanning a shelf: quick jumps, short holds, a blink. */
  @keyframes scan {
    0%,
    14% {
      transform: translate(-30px, -8px);
    }
    17%,
    30% {
      transform: translate(28px, -14px);
    }
    33%,
    50% {
      transform: translate(-22px, 18px) scaleY(1);
    }
    51.5% {
      transform: translate(-22px, 18px) scaleY(0.12);
    }
    53%,
    62% {
      transform: translate(30px, 12px) scaleY(1);
    }
    65%,
    80% {
      transform: translate(4px, -26px);
    }
    83%,
    100% {
      transform: translate(-30px, -8px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .owl,
    .owl * {
      animation: none !important;
      transition: none !important;
    }
  }
</style>

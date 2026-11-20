<script lang="ts" module>
  export type OwlMood = "idle" | "thinking" | "searching" | "studying" | "opening" | "happy" | "puzzled";
</script>

<script lang="ts">
  // Rupert, cut out of the app icon, acting out what the assistant does.
  // The artwork is the icon's own; only his eyes are drawn on top (over
  // the painted ones), so they can blink, look around and read. A small
  // badge names the task. Still under "reduce motion". In Spanish he wears
  // a sky-blue and white striped jersey with a 10 (#19), in German
  // lederhosen (#18): the picture is then drawn in
  // an SVG with the outfit's area cut out, so nothing shows through where
  // both fade out at the bottom.
  import owlUrl from "$lib/assets/rupert-owl.png";
  import { currentLang, m } from "$lib/i18n";

  let { mood = "idle", size = 44 }: { mood?: OwlMood; size?: number } = $props();

  const uid = $props.id();
  /** The shirt: from the shoulders, along the bottom of his face into a
   * V-neck, down to the picture's faded edge (in the picture's 950 × 1000
   * coordinates). */
  const SHIRT =
    "M185 626 C225 645 270 664 320 680 L475 815 L630 680 C680 664 725 645 767 626 C800 628 838 636 868 660 C905 700 925 780 933 860 L936 1000 L14 1000 L17 860 C25 780 45 700 82 660 C112 636 150 628 185 626 Z";
  /** The traditional shirt under the lederhosen: along the bottom of his
   * face, without a V. */
  const TRACHT_SHIRT =
    "M185 626 C260 662 385 702 475 722 C565 702 690 662 767 626 C800 628 838 636 868 660 C905 700 925 780 933 860 L936 1000 L14 1000 L17 860 C25 780 45 700 82 660 C112 636 150 628 185 626 Z";
  const outfit = $derived(({ es: "jersey", de: "lederhosen" } as Record<string, string>)[currentLang()] ?? null);

</script>

<div class="owl mood-{mood}" style:width="{size}px" style:height="{size}px" role="img" aria-label={m.owl[mood]}>
  <div class="body">
    {#if outfit === "jersey"}
      <svg class="outfit" viewBox="0 0 950 1000" aria-hidden="true">
        <defs>
          <!-- A sky-blue stripe, lighter at its edges like the real shirt's -->
          <linearGradient id="stripe-{uid}" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0" stop-color="#9cc7ec" />
            <stop offset="0.22" stop-color="#74acdf" />
            <stop offset="0.78" stop-color="#74acdf" />
            <stop offset="1" stop-color="#9cc7ec" />
          </linearGradient>
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
          <clipPath id="shirt-{uid}"><path d={SHIRT} /></clipPath>
        </defs>
        <image href={owlUrl} width="950" height="1000" mask="url(#no-shirt-{uid})" />
        <g mask="url(#fade-mask-{uid})">
          <!-- White shirt, three sky-blue stripes: the middle one under the V -->
          <path d={SHIRT} fill="#fbfcfe" />
          <g clip-path="url(#shirt-{uid})">
            <rect x="90" y="600" width="170" height="400" fill="url(#stripe-{uid})" />
            <rect x="390" y="600" width="170" height="400" fill="url(#stripe-{uid})" />
            <rect x="690" y="600" width="170" height="400" fill="url(#stripe-{uid})" />
          </g>
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
          <!-- The number 10, in the style of the 2022 shirt's lettering: tall,
               condensed digits, a rounded flag on the 1, a narrow rounded 0,
               and a small V notch low on each. It sits on the middle stripe
               and its lower third fades out with the picture. -->
          <path
            transform="translate(0 25)"
            d="M429 990 H459 V912 L451 905 L459 898 V832 C459 812 446 800 426 800 H410 C397 800 389 809 389 822 V836 C389 842 393 846 399 846 H413 C421 846 429 838 429 830 Z M511 800 H521 A40 40 0 0 1 561 840 V950 A40 40 0 0 1 521 990 H511 A40 40 0 0 1 471 950 V840 A40 40 0 0 1 511 800 Z M513 830 H519 A12 12 0 0 1 531 842 V948 A12 12 0 0 1 519 960 H513 A12 12 0 0 1 501 948 V912 L493 905 L501 898 V842 A12 12 0 0 1 513 830 Z"
            fill="#1c2340"
            fill-rule="evenodd"
            stroke="#a9d0f0"
            stroke-width="4"
            stroke-linejoin="round"
            paint-order="stroke"
          />
        </g>
        <!-- Navy bands over the shoulders, and the V-neck collar -->
        <path
          d="M185 626 C150 629 112 638 84 660 C62 680 46 706 36 738 M767 626 C802 629 840 638 868 660 C890 680 906 706 916 738"
          fill="none"
          stroke="#1f3561"
          stroke-width="28"
          stroke-linecap="round"
        />
        <path
          d="M150 632 C200 628 260 658 320 680 L475 815 L630 680 C690 658 750 628 800 632"
          fill="none"
          stroke="#1f3561"
          stroke-width="22"
          stroke-linecap="round"
          stroke-linejoin="miter"
          stroke-miterlimit="8"
        />
      </svg>
    {:else if outfit === "lederhosen"}
      <!-- Lederhosen (#18) over a white German football shirt in the style of
           the 1990 home shirt (a black-red-gold chevron from the shoulders to
           a point mid-chest, a black collar with a red edge; no crest or
           logo): leather
           braces with the chest strap (Brustlatz) in green trim and an
           embroidered edelweiss, and the shorts' waistband where the picture
           fades. -->
      <svg class="outfit" viewBox="0 0 950 1000" aria-hidden="true">
        <defs>
          <linearGradient id="leather-{uid}" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0" stop-color="#5a3620" />
            <stop offset="0.5" stop-color="#7a4b2b" />
            <stop offset="1" stop-color="#5a3620" />
          </linearGradient>
          <linearGradient id="fade-{uid}" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0.87" stop-color="#fff" />
            <stop offset="0.995" stop-color="#fff" stop-opacity="0" />
          </linearGradient>
          <linearGradient id="volume-{uid}" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0" stop-color="#2a1a10" stop-opacity="0.25" />
            <stop offset="0.2" stop-color="#2a1a10" stop-opacity="0.03" />
            <stop offset="0.8" stop-color="#2a1a10" stop-opacity="0.03" />
            <stop offset="1" stop-color="#2a1a10" stop-opacity="0.25" />
          </linearGradient>
          <mask id="fade-mask-{uid}" maskUnits="userSpaceOnUse" x="0" y="0" width="950" height="1000">
            <rect width="950" height="1000" fill="url(#fade-{uid})" />
          </mask>
          <mask id="no-shirt-{uid}" maskUnits="userSpaceOnUse" x="0" y="0" width="950" height="1000">
            <rect width="950" height="1000" fill="#fff" />
            <path d={TRACHT_SHIRT} fill="#000" />
          </mask>
          <clipPath id="shirt-{uid}"><path d={TRACHT_SHIRT} /></clipPath>
        </defs>
        <image href={owlUrl} width="950" height="1000" mask="url(#no-shirt-{uid})" />
        <g mask="url(#fade-mask-{uid})">
          <path d={TRACHT_SHIRT} fill="#fbfcfe" />
          <g clip-path="url(#shirt-{uid})">
            <path d="M0 652 L475 835 L950 652 L950 694 L475 877 L0 694 Z" fill="#1a1a1a" />
            <path d="M0 694 L475 877 L950 694 L950 736 L475 919 L0 736 Z" fill="#c8161d" />
            <path d="M0 736 L475 919 L950 736 L950 778 L475 961 L0 778 Z" fill="#f5b81c" />
          </g>
          <path d={TRACHT_SHIRT} fill="url(#volume-{uid})" />
          <path
            d="M205 612 C282 616 326 684 321 782 C317 862 300 934 282 1000 L14 1000 L17 860 C25 740 95 616 205 612 Z M745 612 C668 616 624 684 629 782 C633 862 650 934 668 1000 L936 1000 L933 860 C925 740 855 616 745 612 Z"
            fill="#2a1a10"
            fill-opacity="0.1"
          />
          <g clip-path="url(#shirt-{uid})">
            <rect x="0" y="948" width="950" height="60" fill="url(#leather-{uid})" />
            <path d="M262 640 L330 640 L360 1000 L292 1000 Z M688 640 L620 640 L590 1000 L658 1000 Z" fill="url(#leather-{uid})" />
            <path
              d="M271 650 L300 1000 M321 650 L350 1000 M679 650 L650 1000 M629 650 L600 1000"
              stroke="#efe3c8"
              stroke-width="3"
              stroke-dasharray="10 8"
              fill="none"
              opacity="0.8"
            />
            <rect x="318" y="775" width="314" height="86" rx="10" fill="url(#leather-{uid})" />
            <rect x="318" y="775" width="314" height="86" rx="10" fill="none" stroke="#3d7a3f" stroke-width="8" />
            <rect x="334" y="789" width="282" height="58" rx="6" fill="none" stroke="#efe3c8" stroke-width="3" stroke-dasharray="9 7" opacity="0.85" />
            <path
              d="M475 818 Q462 798 475 774 Q488 798 475 818Z M475 818 Q479 794 503 784 Q498 810 475 818Z M475 818 Q493 802 518 810 Q497 827 475 818Z M475 818 Q499 817 513 840 Q486 839 475 818Z M475 818 Q494 833 490 859 Q470 842 475 818Z M475 818 Q480 842 460 859 Q456 833 475 818Z M475 818 Q464 839 437 840 Q451 817 475 818Z M475 818 Q453 827 432 810 Q457 802 475 818Z M475 818 Q452 810 447 784 Q471 794 475 818Z"
              fill="#f4f1ea"
              stroke="#d8d0c0"
              stroke-width="2"
            />
            <circle cx="475" cy="818" r="11" fill="#e8b830" />
            <circle cx="392" cy="818" r="9" fill="#3d7a3f" />
            <circle cx="558" cy="818" r="9" fill="#3d7a3f" />
            <circle cx="326" cy="972" r="11" fill="#e9dcc2" />
            <circle cx="624" cy="972" r="11" fill="#e9dcc2" />
          </g>
        </g>
        <!-- The shirt's black collar with its red edge, along the bottom of his face -->
        <path
          d="M160 640 C210 638 260 672 320 692 C380 712 430 724 475 732 C520 724 570 712 630 692 C690 672 740 638 790 640"
          fill="none"
          stroke="#c8161d"
          stroke-width="8"
          stroke-linecap="round"
        />
        <path
          d="M160 632 C210 630 260 664 320 684 C380 704 430 716 475 724 C520 716 570 704 630 684 C690 664 740 630 790 632"
          fill="none"
          stroke="#1a1a1a"
          stroke-width="16"
          stroke-linecap="round"
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

<script lang="ts" module>
  export type OwlMood = "idle" | "thinking" | "searching" | "studying" | "opening" | "happy" | "puzzled";
</script>

<script lang="ts">
  // Rupert, drawn in lines after the app icon (ear tufts, folded-paper
  // brows, round bronze wire glasses, amber beak). He draws himself in when
  // shown, and acts out what the assistant is doing. With "reduce motion"
  // he stays still.
  import { onMount } from "svelte";

  let { mood = "idle", size = 44 }: { mood?: OwlMood; size?: number } = $props();

  const eyesOpen = $derived(mood !== "happy");
  /** After the first drawing, parts that come back (eyes) show at once. */
  let drawn = $state(false);
  onMount(() => {
    const t = setTimeout(() => (drawn = true), 1500);
    return () => clearTimeout(t);
  });
</script>

<svg
  class="owl mood-{mood}"
  class:drawn
  width={size}
  height={size}
  viewBox="0 0 120 120"
  fill="none"
  stroke="currentColor"
  stroke-width="2.6"
  stroke-linecap="round"
  stroke-linejoin="round"
  role="img"
  aria-label="Rupert is {mood === 'idle' ? 'listening' : mood}"
>
  <g class="body">
    <g class="head">
      <!-- Head with ear tufts, down to the wings -->
      <path
        class="draw"
        style="--d: 0s"
        pathLength="1"
        d="M17 67 Q12 50 15 38 Q16 31 20 26 L24 8 L41 16 Q60 12 79 16 L96 8 L100 26 Q104 31 105 38 Q108 50 103 67"
      />
      <!-- Folded-paper brows, dipping to the middle -->
      <path class="draw brow-l" style="--d: 0.3s" pathLength="1" d="M20 31 L31 22 Q47 25 60 33 L57 38 Q45 32 33 30 L23 38 Z" />
      <path class="draw brow-r" style="--d: 0.35s" pathLength="1" d="M100 31 L89 22 Q73 25 60 33 L63 38 Q75 32 87 30 L97 38 Z" />
      <!-- Facial disc, coming to a point below the beak -->
      <path class="draw" style="--d: 0.45s" pathLength="1" d="M23 38 Q17 50 22 62 Q30 74 60 80 Q90 74 98 62 Q103 50 97 38" />
      <!-- Round wire glasses -->
      <g class="glasses">
        <circle class="draw" style="--d: 0.55s" pathLength="1" cx="44.4" cy="51.2" r="14.4" />
        <circle class="draw" style="--d: 0.6s" pathLength="1" cx="75.6" cy="51.2" r="14.4" />
        <path class="draw" style="--d: 0.7s" pathLength="1" d="M58.8 49.5 Q60 46.8 61.2 49.5 M30 50 L14 46.5 M90 50 L106 46.5" />
        {#if mood === "studying"}
          <path class="glint" d="M36 44 L41 40 M67.2 44 L72.2 40" />
        {/if}
      </g>
      <!-- Eyes: big dark pupils with a highlight -->
      {#if eyesOpen}
        <g class="eyes">
          <g class="pupils">
            <circle class="pupil" cx="44" cy="51" r="8" />
            <circle class="pupil" cx="76" cy="51" r="8" />
            <circle class="shine" cx="46.6" cy="47.6" r="2.1" />
            <circle class="shine" cx="78.6" cy="47.6" r="2.1" />
          </g>
        </g>
      {:else}
        <path class="draw happy-eyes" style="--d: 0s" pathLength="1" d="M36 54 Q44 44 52 54 M68 54 Q76 44 84 54" />
      {/if}
      <!-- Beak -->
      <path class="beak" d="M56 56 L64 56 L60 66 Z" />
    </g>
    <!-- Wings, and the belly between them -->
    <path class="draw wing-l" style="--d: 0.8s" pathLength="1" d="M10 111 Q6 82 22 69 Q36 64 43 82 Q46 96 44 111" />
    <path class="draw wing-r" style="--d: 0.85s" pathLength="1" d="M110 111 Q114 82 98 69 Q84 64 77 82 Q74 96 76 111" />
    <path class="draw" style="--d: 0.95s" pathLength="1" d="M44 111 Q60 114 76 111" />
  </g>

  <!-- What he's doing -->
  {#if mood === "thinking"}
    <g class="thought">
      <circle class="dot d1" cx="104" cy="15" r="2.4" />
      <circle class="dot d2" cx="110" cy="8" r="3" />
      <circle class="dot d3" cx="117" cy="1" r="3.6" />
    </g>
  {:else if mood === "searching"}
    <g class="magnifier">
      <circle class="draw prop" style="--d: 0s" pathLength="1" cx="92" cy="88" r="9" />
      <path class="draw prop" style="--d: 0.2s" pathLength="1" d="M98.5 94.5 L108 104" />
    </g>
  {:else if mood === "studying"}
    <g class="book">
      <path
        class="draw prop"
        style="--d: 0s"
        pathLength="1"
        d="M42 88 Q51 83 60 88 Q69 83 78 88 L78 101 Q69 96 60 101 Q51 96 42 101 Z M60 88 L60 101"
      />
      <path class="draw prop lines" style="--d: 0.3s" pathLength="1" d="M46 91 L56 89.5 M46 95 L56 93.5 M64 89.5 L74 91 M64 93.5 L74 95" />
    </g>
  {:else if mood === "opening"}
    <g class="folder">
      <path class="draw prop" style="--d: 0s" pathLength="1" d="M88 90 L97 90 L100 93 L114 93 L114 110 L88 110 Z" />
      <path class="draw prop flap" style="--d: 0.25s" pathLength="1" d="M88 97 L114 97" />
    </g>
  {:else if mood === "puzzled"}
    <g class="question">
      <path class="draw prop" style="--d: 0s" pathLength="1" d="M105 16 Q105 6 112 6 Q119 6 119 12 Q119 17 112 19 L112 24" />
      <circle class="fill" cx="112" cy="30" r="1.8" />
    </g>
  {/if}
</svg>

<style>
  .owl {
    overflow: visible;
    flex: none;
  }
  .glasses {
    stroke: var(--folder);
  }
  .fill {
    fill: currentColor;
    stroke: none;
  }
  .pupil {
    fill: #3d2316;
    stroke-width: 1.6;
  }
  .shine {
    fill: #fff;
    stroke: none;
  }
  .beak {
    fill: #e08a2c;
    stroke: #c26f17;
    stroke-width: 1.4;
  }
  .prop {
    stroke: var(--accent);
  }
  .glint {
    stroke: var(--bg);
    stroke-width: 2;
    opacity: 0;
    animation: glint 2.4s ease-in-out infinite;
  }
  .dot {
    fill: var(--accent);
    stroke: none;
    opacity: 0.2;
    animation: pulse 1.5s ease-in-out infinite;
  }
  .d2 {
    animation-delay: 0.25s;
  }
  .d3 {
    animation-delay: 0.5s;
  }
  /* Groups move around their own centres. */
  .head,
  .body,
  .eyes,
  .pupils,
  .magnifier,
  .flap,
  .wing-r,
  .brow-r {
    transform-box: fill-box;
    transform-origin: center;
  }

  /* Drawn in, line by line. */
  .draw {
    stroke-dasharray: 1;
    stroke-dashoffset: 1;
    animation: draw 0.9s ease-out forwards;
    animation-delay: var(--d, 0s);
  }
  .fill,
  .pupils,
  .beak {
    animation: appear 0.4s ease-out 1s both;
  }
  .prop.draw {
    animation-duration: 0.6s;
  }

  /* Idle: breathing and the odd blink. */
  .mood-idle .body {
    animation: breathe 4s ease-in-out infinite;
  }
  .mood-idle .eyes {
    animation: blink 5s ease-in-out 1.5s infinite;
  }
  /* Thinking: looking up and away. */
  .mood-thinking .pupils {
    animation: look-up 2.4s ease-in-out infinite;
  }
  /* Searching: the glass sweeps, the eyes follow. */
  .mood-searching .magnifier {
    animation: sweep 1.6s ease-in-out infinite;
  }
  .mood-searching .pupils {
    animation: follow 1.6s ease-in-out infinite;
  }
  /* Studying: reading line by line. */
  .mood-studying .pupils {
    animation: read 2.4s steps(1) infinite;
  }
  .drawn .fill,
  .drawn .beak,
  .drawn .pupils {
    animation: none;
  }
  .happy-eyes {
    animation-duration: 0.35s;
  }
  /* Opening: the flap swings, a wing lifts. */
  .mood-opening .flap {
    transform-origin: left center;
    animation: flap 1.4s ease-in-out infinite;
  }
  .mood-opening .wing-r {
    transform-origin: bottom center;
    animation: wave 1.4s ease-in-out infinite;
  }
  /* Happy: a little hop. */
  .mood-happy .body {
    animation: hop 0.6s ease-out 2;
  }
  /* Puzzled: head tilt, one brow up. */
  .mood-puzzled .head {
    transform: rotate(8deg);
    transition: transform 0.4s;
  }
  .mood-puzzled .brow-r {
    transform: translateY(-3px) rotate(-6deg);
  }

  @keyframes draw {
    to {
      stroke-dashoffset: 0;
    }
  }
  @keyframes appear {
    from {
      opacity: 0;
    }
  }
  @keyframes breathe {
    50% {
      transform: scale(1.025);
    }
  }
  @keyframes blink {
    0%,
    94%,
    100% {
      transform: scaleY(1);
    }
    97% {
      transform: scaleY(0.1);
    }
  }
  @keyframes look-up {
    0%,
    100% {
      transform: translate(0, 0);
    }
    30%,
    70% {
      transform: translate(2.5px, -3px);
    }
  }
  @keyframes sweep {
    0%,
    100% {
      transform: translate(0, 0) rotate(0);
    }
    50% {
      transform: translate(-44px, -36px) rotate(-12deg);
    }
  }
  @keyframes follow {
    0%,
    100% {
      transform: translate(2.5px, 1px);
    }
    50% {
      transform: translate(-2.5px, 0);
    }
  }
  @keyframes read {
    0% {
      transform: translate(-2.5px, 1px);
    }
    25% {
      transform: translate(0, 1px);
    }
    50% {
      transform: translate(2.5px, 1px);
    }
    75% {
      transform: translate(-2.5px, 3px);
    }
  }
  @keyframes glint {
    0%,
    60%,
    100% {
      opacity: 0;
    }
    75% {
      opacity: 0.9;
    }
  }
  @keyframes pulse {
    50% {
      opacity: 1;
    }
  }
  @keyframes flap {
    0%,
    100% {
      transform: rotate(0);
    }
    50% {
      transform: rotate(-32deg);
    }
  }
  @keyframes wave {
    0%,
    100% {
      transform: rotate(0);
    }
    50% {
      transform: rotate(-14deg);
    }
  }
  @keyframes hop {
    40% {
      transform: translateY(-5px);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .owl *,
    .owl {
      animation: none !important;
      transition: none !important;
    }
    .draw {
      stroke-dashoffset: 0;
    }
    .dot {
      opacity: 0.8;
    }
  }
</style>

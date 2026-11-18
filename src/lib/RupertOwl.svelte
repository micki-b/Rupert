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
      <!-- Outline with ear tufts -->
      <path
        class="draw"
        style="--d: 0s"
        pathLength="1"
        d="M38 30 L30 14 L47 25 Q60 21 73 25 L90 14 L82 30 Q96 44 94 66 Q92 92 76 104 Q60 112 44 104 Q28 92 26 66 Q24 44 38 30 Z"
      />
      <!-- Folded-paper brows -->
      <path class="draw brow-l" style="--d: 0.35s" pathLength="1" d="M36 45 Q46 35 58 45 L52 46 Q45 41 38 47 Z" />
      <path class="draw brow-r" style="--d: 0.4s" pathLength="1" d="M84 45 Q74 35 62 45 L68 46 Q75 41 82 47 Z" />
      <!-- Round wire glasses -->
      <g class="glasses">
        <circle class="draw" style="--d: 0.55s" pathLength="1" cx="48" cy="57" r="11" />
        <circle class="draw" style="--d: 0.6s" pathLength="1" cx="72" cy="57" r="11" />
        <path class="draw" style="--d: 0.7s" pathLength="1" d="M59 56 Q60 53 61 56 M37 56 L29 52 M83 56 L91 52" />
        {#if mood === "studying"}
          <path class="glint" d="M42 51 L46 48 M66 51 L70 48" />
        {/if}
      </g>
      <!-- Eyes -->
      {#if eyesOpen}
        <g class="eyes">
          <g class="pupils">
            <circle class="fill" cx="48" cy="58" r="3.6" />
            <circle class="fill" cx="72" cy="58" r="3.6" />
          </g>
        </g>
      {:else}
        <path class="draw happy-eyes" style="--d: 0s" pathLength="1" d="M43 59 Q48 52 53 59 M67 59 Q72 52 77 59" />
      {/if}
      <!-- Beak -->
      <path class="beak" d="M55.5 66 L64.5 66 L60 74 Z" />
    </g>
    <!-- Wings and chest -->
    <path class="draw wing-l" style="--d: 0.85s" pathLength="1" d="M31 70 Q22 88 36 101" />
    <path class="draw wing-r" style="--d: 0.9s" pathLength="1" d="M89 70 Q98 88 84 101" />
    <path class="draw" style="--d: 1s" pathLength="1" d="M47 84 L60 93 L73 84" />
  </g>

  <!-- What he's doing -->
  {#if mood === "thinking"}
    <g class="thought">
      <circle class="dot d1" cx="97" cy="20" r="2.4" />
      <circle class="dot d2" cx="104" cy="12" r="3" />
      <circle class="dot d3" cx="112" cy="5" r="3.6" />
    </g>
  {:else if mood === "searching"}
    <g class="magnifier">
      <circle class="draw prop" style="--d: 0s" pathLength="1" cx="86" cy="78" r="9" />
      <path class="draw prop" style="--d: 0.2s" pathLength="1" d="M92.5 84.5 L102 94" />
    </g>
  {:else if mood === "studying"}
    <g class="book">
      <path
        class="draw prop"
        style="--d: 0s"
        pathLength="1"
        d="M38 92 Q49 86 60 92 Q71 86 82 92 L82 106 Q71 100 60 106 Q49 100 38 106 Z M60 92 L60 106"
      />
      <path class="draw prop lines" style="--d: 0.3s" pathLength="1" d="M43 95 L54 93 M43 99 L54 97 M66 93 L77 95 M66 97 L77 99" />
    </g>
  {:else if mood === "opening"}
    <g class="folder">
      <path class="draw prop" style="--d: 0s" pathLength="1" d="M84 86 L93 86 L96 89 L110 89 L110 106 L84 106 Z" />
      <path class="draw prop flap" style="--d: 0.25s" pathLength="1" d="M84 93 L110 93" />
    </g>
  {:else if mood === "puzzled"}
    <g class="question">
      <path class="draw prop" style="--d: 0s" pathLength="1" d="M99 22 Q99 12 106 12 Q113 12 113 18 Q113 23 106 25 L106 30" />
      <circle class="fill" cx="106" cy="36" r="1.8" />
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
  .drawn .beak {
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
    transform-origin: top center;
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
      transform: translate(-34px, -14px) rotate(-12deg);
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

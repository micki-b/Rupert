// rupertlibrarian.com home page: the light/dark handle, the Ask Rupert
// languages, the platform frames, the download button for this computer,
// and the scan's flight. Everything works without it too.

const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;
/** The page's language: en, es or de. */
const pageLang = ["es", "de"].includes(document.documentElement.lang) ? document.documentElement.lang : "en";

// MARK: - Light and dark

(() => {
  const box = document.querySelector(".daynight");
  if (!box) return;
  const handle = box.querySelector(".handle");
  const set = (pct) => box.style.setProperty("--split", `${Math.max(4, Math.min(96, pct))}%`);
  const fromEvent = (e) => {
    const r = box.getBoundingClientRect();
    set(((e.clientX - r.left) / r.width) * 100);
  };
  let dragging = false;
  let touched = false;
  box.addEventListener("pointerdown", (e) => {
    dragging = touched = true;
    box.setPointerCapture(e.pointerId);
    box.classList.add("dragging");
    fromEvent(e);
  });
  box.addEventListener("pointermove", (e) => dragging && fromEvent(e));
  const stop = () => {
    dragging = false;
    box.classList.remove("dragging");
  };
  box.addEventListener("pointerup", stop);
  box.addEventListener("pointercancel", stop);
  handle.addEventListener("keydown", (e) => {
    const now = parseFloat(getComputedStyle(box).getPropertyValue("--split")) || 30;
    if (e.key === "ArrowLeft") set(now - 5);
    if (e.key === "ArrowRight") set(now + 5);
    touched = true;
  });

  // A slow sweep when it first comes into view: to the middle, then back.
  if (reduceMotion) return;
  const sweep = (from, to, ms) =>
    new Promise((done) => {
      const start = performance.now();
      const step = (t) => {
        if (touched) return done();
        const k = Math.min(1, (t - start) / ms);
        const ease = k < 0.5 ? 2 * k * k : 1 - Math.pow(-2 * k + 2, 2) / 2;
        set(from + (to - from) * ease);
        k < 1 ? requestAnimationFrame(step) : done();
      };
      requestAnimationFrame(step);
    });
  new IntersectionObserver((entries, obs) => {
    if (!entries[0].isIntersecting) return;
    obs.disconnect();
    sweep(30, 50, 1800).then(() => sweep(50, 30, 1800));
  }, { threshold: 0.5 }).observe(box);
})();

// MARK: - Ask Rupert, in three languages

(() => {
  const chat = document.querySelector(".chat");
  if (!chat) return;
  const text = {
    en: {
      title: "Ask Rupert",
      q: "What's the deductible on my car insurance?",
      steps: "Searching for “car insurance” · Reading Insurance/Car insurance renewal.pdf",
      a: "Your Acme Mutual car policy, renewed March 2, 2026, has a $500 collision deductible. The yearly premium is $1,184 and roadside assistance is included. Shall I open it?",
      placeholder: "Ask about your documents…",
    },
    es: {
      title: "Preguntale a Rupert",
      q: "¿Cuál es la franquicia del seguro del auto?",
      steps: "Buscando «seguro auto» · Leyendo Insurance/Car insurance renewal.pdf",
      a: "Tu póliza de auto de Acme Mutual, renovada el 2 de marzo de 2026, tiene una franquicia de USD 500 por choque. La prima anual es de USD 1.184 e incluye auxilio mecánico. ¿Querés que la abra?",
      placeholder: "Preguntá sobre tus documentos…",
    },
    de: {
      title: "Rupert fragen",
      q: "Wie hoch ist die Selbstbeteiligung meiner Autoversicherung?",
      steps: "Suche nach „Autoversicherung“ · Insurance/Car insurance renewal.pdf wird gelesen",
      a: "Ihre Kfz-Police bei Acme Mutual, verlängert am 2. März 2026, hat eine Selbstbeteiligung von 500 USD bei Kollisionen. Die Jahresprämie beträgt 1.184 USD, Pannenhilfe ist inklusive. Soll ich sie öffnen?",
      placeholder: "Fragen Sie zu Ihren Dokumenten …",
    },
  };
  // The tour starts in the page's language.
  const order = ["en", "es", "de"];
  while (order[0] !== pageLang) order.push(order.shift());
  const tabs = [...document.querySelectorAll(".langs [data-lang]")];
  const field = (k) => chat.querySelector(`[data-t="${k}"]`);
  let current = "en";
  let timer;
  let typing;

  function show(lang, animate = true) {
    current = lang;
    const t = text[lang];
    chat.setAttribute("lang", lang);
    chat.dataset.lang = lang;
    for (const b of tabs) b.setAttribute("aria-selected", String(b.dataset.lang === lang));
    for (const k of ["title", "q", "steps", "placeholder"]) field(k).textContent = t[k];
    const answer = field("a");
    clearInterval(typing);
    if (!animate || reduceMotion) {
      answer.textContent = t.a;
      return;
    }
    chat.classList.remove("in");
    void chat.offsetWidth;
    chat.classList.add("in");
    answer.textContent = "";
    const words = t.a.split(" ");
    let i = 0;
    setTimeout(() => {
      typing = setInterval(() => {
        answer.textContent = words.slice(0, ++i).join(" ");
        if (i >= words.length) clearInterval(typing);
      }, 55);
    }, 900);
  }

  const next = () => show(order[(order.indexOf(current) + 1) % order.length]);
  const cycle = () => {
    clearInterval(timer);
    timer = setInterval(next, 9000);
  };
  for (const b of tabs)
    b.addEventListener("click", () => {
      show(b.dataset.lang);
      clearInterval(timer); // the visitor chose; stop the tour
    });
  show(pageLang, false);
  if (!reduceMotion) {
    new IntersectionObserver((entries, obs) => {
      if (!entries[0].isIntersecting) return;
      obs.disconnect();
      show(pageLang);
      cycle();
    }, { threshold: 0.4 }).observe(chat);
  }
})();

// MARK: - One library, every screen

(() => {
  const device = document.querySelector(".device-stage");
  if (!device) return;
  const tabs = [...document.querySelectorAll(".platforms [data-platform]")];
  const notes = [...document.querySelectorAll(".platform-note")];
  const order = tabs.map((b) => b.dataset.platform);
  let timer;
  function show(p) {
    device.dataset.platform = p;
    for (const b of tabs) b.setAttribute("aria-selected", String(b.dataset.platform === p));
    for (const n of notes) n.hidden = n.dataset.note !== p;
  }
  for (const b of tabs)
    b.addEventListener("click", () => {
      clearInterval(timer);
      show(b.dataset.platform);
    });
  if (reduceMotion) return;
  new IntersectionObserver((entries, obs) => {
    if (!entries[0].isIntersecting) return;
    obs.disconnect();
    timer = setInterval(() => show(order[(order.indexOf(device.dataset.platform) + 1) % order.length]), 3200);
  }, { threshold: 0.4 }).observe(device);
})();

// MARK: - The scan's flight

(() => {
  const story = document.querySelector(".scan-story");
  if (!story || reduceMotion) return;
  new IntersectionObserver((entries) => {
    for (const e of entries) story.classList.toggle("go", e.isIntersecting);
  }, { threshold: 0.35 }).observe(story);
})();

// MARK: - Download for this computer

(() => {
  const ua = navigator.userAgent;
  const os = /iPhone|iPad|iPod/.test(ua)
    ? "ios"
    : /Mac/.test(ua)
      ? "mac"
      : /Windows/.test(ua)
        ? "win"
        : /Android/.test(ua)
          ? null
          : /Linux/.test(ua)
            ? "linux"
            : null;
  if (!os) return;
  const tile = document.querySelector(`.pf[data-os="${os}"]`);
  if (!tile) return;
  tile.classList.add("yours");
  const words = {
    en: {
      mac: ["Download for Mac", "Universal: Apple silicon and Intel. macOS 13 or later."],
      win: ["Download for Windows", "For Windows 10 and 11. A signed installer."],
      linux: ["Download for Linux", "AppImage, .deb and .rpm on the releases page."],
      ios: ["Get it on the App Store", "The computer version reads, files and searches; the iPhone scans and browses."],
    },
    es: {
      mac: ["Descargar para Mac", "Universal: Apple silicon e Intel. macOS 13 o posterior."],
      win: ["Descargar para Windows", "Para Windows 10 y 11. Un instalador firmado."],
      linux: ["Descargar para Linux", "AppImage, .deb y .rpm en la página de versiones."],
      ios: ["Bajala del App Store", "La versión de computadora lee, archiva y busca; el iPhone escanea y recorre."],
    },
    de: {
      mac: ["Für Mac laden", "Universal: Apple-Chip und Intel. macOS 13 oder neuer."],
      win: ["Für Windows laden", "Für Windows 10 und 11. Ein signiertes Installationsprogramm."],
      linux: ["Für Linux laden", "AppImage, .deb und .rpm auf der Release-Seite."],
      ios: ["Im App Store laden", "Die Computer-Version liest, ordnet und sucht; das iPhone scannt und stöbert."],
    },
  };
  const [label, note] = words[pageLang][os];
  for (const a of document.querySelectorAll("[data-download-primary]")) {
    if (a.closest(".hero2")) {
      a.textContent = label;
      a.href = "#download";
    } else {
      a.href = tile.href;
      a.querySelector("[data-download-label]").textContent = label;
    }
  }
  const n = document.querySelector("[data-download-note]");
  if (n) n.textContent = note;
})();

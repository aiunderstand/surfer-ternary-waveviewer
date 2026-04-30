// BroadcastChannel bridge between MRCS Studio (Tab A) and surfer-ternary-waveviewer (Tab B).
// Loaded from surfer/index.html after integration.js. Both apps share an origin (MRCS Studio at /surfer/).
(function () {
  "use strict";

  const CHANNEL = "mrcs-surfer-v1";
  const POLL_MS = 100;
  const WASM_TIMEOUT_MS = 30000;
  const MAX_QUEUED = 4;

  let bc = null;
  let titleSet = false;
  let firstHeaderApplied = false;

  // Startup queue: holds message objects received before inject_message is available.
  const queue = [];
  let queueDrained = false;
  let pollTimer = null;

  function isInjectReady() {
    return typeof inject_message === "function";
  }

  function safeInject(json) {
    try {
      inject_message(json);
    } catch (e) {
      console.error("[mrcs-surfer-bridge] inject_message threw:", e);
    }
  }

  // FR-5: hide the side panel and top menu the first time MRCS Studio sends a header,
  // so the freshly opened Surfer tab presents a clean waveform-only view.
  function applyDefaultLayout() {
    if (firstHeaderApplied) return;
    firstHeaderApplied = true;
    safeInject(JSON.stringify({ SetSidePanelVisible: false }));
    safeInject(JSON.stringify({ SetMenuVisible: false }));
  }

  function applyMsg(msg) {
    switch (msg.type) {
      case "ping":
        bc.postMessage({ type: "pong", sessionId: msg.sessionId });
        break;

      case "header":
        // Initial / probe-set-change reload: clear existing waveform state, then add scopes.
        if (!titleSet) {
          document.title = "Surfer – MRCS Live";
          titleSet = true;
        }
        safeInject(JSON.stringify({ LoadFromData: [msg.data, "Clear"] }));
        applyDefaultLayout();
        if (msg.scopeCommands) {
          const bytes = Array.from(new TextEncoder().encode(msg.scopeCommands));
          safeInject(JSON.stringify({ LoadCommandFromData: bytes }));
        }
        break;

      case "frame":
        // Live update reload: KeepAvailable preserves displayed signals across reloads,
        // so we don't re-issue scope_add and we don't get duplicate signal entries.
        safeInject(JSON.stringify({ LoadFromData: [msg.data, "KeepAvailable"] }));
        break;

      case "reset":
        safeInject(JSON.stringify("RemovePlaceholders"));
        safeInject(JSON.stringify({ ZoomToFit: { viewport_idx: 0 } }));
        break;

      default:
        console.warn("[mrcs-surfer-bridge] Unknown message type:", msg.type);
    }
  }

  function drain() {
    queueDrained = true;
    clearTimeout(pollTimer);
    const pending = queue.splice(0);
    for (const m of pending) applyMsg(m);
  }

  function schedulePoll() {
    const deadline = Date.now() + WASM_TIMEOUT_MS;
    function tick() {
      if (isInjectReady()) {
        drain();
      } else if (Date.now() >= deadline) {
        console.error(
          "[mrcs-surfer-bridge] Timed out after 30 s waiting for inject_message. " +
          "Discarding " + queue.length + " queued message(s)."
        );
        queueDrained = true;
        queue.length = 0;
      } else {
        pollTimer = setTimeout(tick, POLL_MS);
      }
    }
    pollTimer = setTimeout(tick, POLL_MS);
  }

  function dispatch(msg) {
    if (!msg || typeof msg.type !== "string") {
      console.warn("[mrcs-surfer-bridge] Ignoring malformed message (no string 'type'):", msg);
      return;
    }

    if (msg.type === "ping" || msg.type === "reset") {
      applyMsg(msg);
      return;
    }

    if (queueDrained || (isInjectReady() && queue.length === 0)) {
      applyMsg(msg);
      return;
    }

    if (queue.length < MAX_QUEUED) {
      const wasEmpty = queue.length === 0;
      queue.push(msg);
      if (wasEmpty) schedulePoll();
    } else {
      // Drop oldest non-header to keep latency bounded.
      const idx = queue.findIndex((m) => m.type === "frame");
      if (idx >= 0) queue.splice(idx, 1);
      queue.push(msg);
    }
  }

  if (typeof BroadcastChannel === "undefined") {
    console.warn("[mrcs-surfer-bridge] BroadcastChannel is not available in this browser. Live streaming disabled.");
    return;
  }

  bc = new BroadcastChannel(CHANNEL);
  bc.onmessage = function (ev) {
    try {
      dispatch(ev.data);
    } catch (e) {
      console.error("[mrcs-surfer-bridge] Uncaught error in onmessage handler:", e);
    }
  };

  console.log("[mrcs-surfer-bridge] Listening on BroadcastChannel '" + CHANNEL + "'");
})();

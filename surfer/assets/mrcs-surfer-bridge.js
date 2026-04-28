// window.postMessage bridge between MRCS Studio (Tab A) and surfer-ternary-waveviewer (Tab B).
// Loaded from surfer/index.html after integration.js.
// Works cross-origin: MRCS Studio posts to the surfer window reference it obtained from window.open();
// this script listens on window and replies to the sender via event.source.postMessage().
(function () {
  "use strict";

  const POLL_MS = 100;
  const WASM_TIMEOUT_MS = 30000;
  const MAX_QUEUED = 2; // header + t=0 frame

  let titleSet = false;

  // Startup queue: holds messages received before inject_message is available (FR-4.3).
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

  // Apply a decoded message object, calling inject_message as needed.
  // ping is handled upstream (needs event.source) and never reaches here.
  function applyMsg(msg) {
    switch (msg.type) {
      case "header":
        // msg.data is an array of integers (0-255): complete MRCS-GHW file (full-reload, FR-3.9).
        if (!titleSet) {
          document.title = "Surfer – MRCS Live"; // FR-4.8
          titleSet = true;
        }
        safeInject(JSON.stringify({ LoadFromData: [msg.data, "Clear"] }));
        // FR-4.7: auto-add all probed scopes after the waveform loads.
        if (msg.scopeCommands) {
          const bytes = Array.from(new TextEncoder().encode(msg.scopeCommands));
          safeInject(JSON.stringify({ LoadCommandFromData: bytes }));
        }
        break;

      case "frame":
        // v2: AppendWaveformFrame when wellen gains incremental-append support. No-op scaffold for v1.
        break;

      case "reset":
        // FR-2.7
        safeInject(JSON.stringify("RemovePlaceholders"));
        safeInject(JSON.stringify({ ZoomToFit: { viewport_idx: 0 } }));
        break;

      default:
        console.warn("[mrcs-surfer-bridge] Unknown message type:", msg.type);
    }
  }

  // Drain the startup queue once inject_message becomes available.
  function drain() {
    queueDrained = true;
    clearTimeout(pollTimer);
    const pending = queue.splice(0);
    for (const m of pending) {
      applyMsg(m);
    }
  }

  // Poll for inject_message readiness up to WASM_TIMEOUT_MS (FR-4.3).
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

  // Route an incoming message: apply immediately or enqueue (if WASM not ready).
  function dispatch(msg) {
    if (!msg || typeof msg.type !== "string") {
      console.warn("[mrcs-surfer-bridge] Ignoring malformed message (no string 'type'):", msg);
      return;
    }

    // reset does not call inject_message — handle unconditionally.
    if (msg.type === "reset") {
      applyMsg(msg);
      return;
    }

    // header and frame need inject_message.
    if (queueDrained || (isInjectReady() && queue.length === 0)) {
      applyMsg(msg);
      return;
    }

    // WASM not ready yet: enqueue with cap (FR-4.3).
    if (queue.length < MAX_QUEUED) {
      const wasEmpty = queue.length === 0;
      queue.push(msg);
      if (wasEmpty) schedulePoll();
    } else {
      console.warn("[mrcs-surfer-bridge] Startup queue full (max " + MAX_QUEUED + "); dropping " + msg.type + " message.");
    }
  }

  window.addEventListener("message", function (event) {
    try {
      const msg = event.data;
      if (!msg || typeof msg.type !== "string") {
        console.warn("[mrcs-surfer-bridge] Ignoring malformed message:", msg);
        return;
      }

      // ping must reply to the specific sender window (cross-origin compatible).
      if (msg.type === "ping") {
        console.log("[mrcs-surfer-bridge] ping received from", event.origin, "source:", event.source);
        try {
          event.source.postMessage({ type: "pong", sessionId: msg.sessionId }, event.origin);
          console.log("[mrcs-surfer-bridge] pong sent to", event.origin);
        } catch (e) {
          console.error("[mrcs-surfer-bridge] Failed to send pong:", e);
        }
        return;
      }

      dispatch(msg);
    } catch (e) {
      console.error("[mrcs-surfer-bridge] Uncaught error in message handler:", e);
    }
  });

  console.log("[mrcs-surfer-bridge] Ready – listening for cross-origin postMessage from MRCS Studio");
})();

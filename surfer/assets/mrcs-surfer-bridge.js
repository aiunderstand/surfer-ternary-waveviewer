// BroadcastChannel bridge between MRCS Studio (Tab A) and surfer-ternary-waveviewer (Tab B).
// Loaded from surfer/index.html after integration.js.
// Both apps are served from the same origin (MRCS Studio at /surfer/).
(function () {
  "use strict";

  const CHANNEL = "mrcs-surfer-v1";
  const POLL_MS = 100;
  const WASM_TIMEOUT_MS = 30000;
  const MAX_QUEUED = 2; // FR-4.3: header + t=0 frame

  let bc = null;
  let titleSet = false;

  // Startup queue: holds message objects received before inject_message is available (FR-4.3).
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
  function applyMsg(msg) {
    switch (msg.type) {
      case "ping":
        // Respond with pong on the same channel (FR-2.7 / FR-2.1 handshake)
        bc.postMessage({ type: "pong", sessionId: msg.sessionId });
        break;

      case "header":
        // msg.data is an array of integers (0-255) representing the complete MRCS-GHW file
        // (header sections + all accumulated SNP/CYC frames — full-reload approach, FR-3.9).
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

  // Route an incoming BroadcastChannel message: apply immediately or enqueue.
  function dispatch(msg) {
    if (!msg || typeof msg.type !== "string") {
      console.warn("[mrcs-surfer-bridge] Ignoring malformed message (no string 'type'):", msg);
      return;
    }

    // ping and reset do not call inject_message — handle unconditionally.
    if (msg.type === "ping" || msg.type === "reset") {
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

  // Bail out gracefully if BroadcastChannel is unavailable (NFR-2.1).
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

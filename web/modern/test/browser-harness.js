(() => {
  const realFetch = window.fetch.bind(window);
  const requestedSkin = new URLSearchParams(window.location.search).get("skin");
  const allowedSkins = new Set(["MMD3.wal", "WinampModern566.wal", "CornerAmp_Redux.wal"]);
  const sampleName = allowedSkins.has(requestedSkin) ? requestedSkin : "MMD3.wal";
  const sampleSkin = new URL(
    `../../../native/webamp/packages/webamp-modern/assets/skins/${sampleName}`,
    window.location.href,
  );
  window.fetch = (input, options) => {
    const url = String(input instanceof Request ? input.url : input);
    return realFetch(
      url.startsWith("kogskin://current/skin.wal") ? sampleSkin : input,
      options,
    );
  };

  const listeners = [];
  const bridge = {
    skinUrl: "kogskin://current/skin.wal",
    stateJson: JSON.stringify({
      playback: "paused",
      position: 12,
      duration: 185,
      volume: 0.65,
      currentIndex: 0,
      revision: 1,
      tracks: [
        {
          id: "one",
          title: "Markup <b>must remain text</b>",
          artist: "Kog",
          album: "Browser smoke test",
          duration: 185,
        },
      ],
      shuffle: "off",
      repeat: "off",
      eq: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
      eqEnabled: true,
      eqPreamp: 0,
      songInfo: "FLAC • 44.1 kHz • stereo • 921 kbps",
      visualization: { wave: [0, 0.25, -0.25], spectrum: [0.1, 0.5, 0.9] },
    }),
    stateJsonChanged: { connect(callback) { listeners.push(callback); } },
    request(command, payloadJson) {
      document.documentElement.dataset.lastCommand = command;
      document.documentElement.dataset.lastPayload = payloadJson;
      if (command === "ready") {
        document.documentElement.dataset.kogReady = "true";
        queueMicrotask(() => {
          const blocked = window.kogModern.commands.send("clear");
          document.documentElement.dataset.untrustedClearBlocked = String(!blocked);
          document.querySelector("#play")?.click();
        });
      }
      if (command === "error") document.documentElement.dataset.kogError = payloadJson;
    },
  };

  window.qt = { webChannelTransport: {} };
  window.QWebChannel = class QWebChannel {
    constructor(_transport, callback) {
      callback({ objects: { kog: bridge } });
    }
  };
})();

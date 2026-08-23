import { describe, expect, it as bunIt } from "bun:test";

const {
  AudioPassthrough,
  AudioPlayer,
  AudioRecorder,
  getInputDevices,
} = await import("../../index.js");
import {
  isAudioSystemAvailable,
  REQUIRE_AUDIO_HARDWARE,
} from "../utils/test-helpers.js";

function makeSilenceWav(durationMs: number, sampleRate = 44_100): number[] {
  const frameCount = Math.floor((sampleRate * durationMs) / 1000);
  const dataSize = frameCount * 2;
  const bytes = new Uint8Array(44 + dataSize);
  const view = new DataView(bytes.buffer);
  const writeAscii = (offset: number, value: string) => {
    for (let index = 0; index < value.length; index++) {
      bytes[offset + index] = value.charCodeAt(index);
    }
  };

  writeAscii(0, "RIFF");
  view.setUint32(4, 36 + dataSize, true);
  writeAscii(8, "WAVE");
  writeAscii(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  writeAscii(36, "data");
  view.setUint32(40, dataSize, true);
  return Array.from(bytes);
}

const outputIt = bunIt.skipIf(!isAudioSystemAvailable());
const inputDevices = (() => {
  try {
    return getInputDevices();
  } catch {
    return [];
  }
})();
const inputAvailable = inputDevices.length > 0;
const inputDeviceId = inputDevices.find((device) => device.isDefault)?.id ?? inputDevices[0]?.id;
if (REQUIRE_AUDIO_HARDWARE && !inputAvailable) {
  throw new Error("Required audio input hardware is unavailable.");
}
const inputIt = bunIt.skipIf(!inputAvailable && !REQUIRE_AUDIO_HARDWARE);

describe("hardware audio paths", () => {
  outputIt("opens the native output path and advances playback", async () => {
    const player = new AudioPlayer();
    try {
      player.loadBuffer(makeSilenceWav(250));
      player.play();
      await Bun.sleep(80);
      expect(player.getCurrentTime()).toBeGreaterThan(0);
    } finally {
      player.stop();
    }
  });

  inputIt("opens and closes the native input path", async () => {
    const recorder = new AudioRecorder();
    try {
      recorder.start(inputDeviceId);
      await Bun.sleep(80);
      expect(recorder.isRecording()).toBe(true);
    } finally {
      recorder.stop();
    }
    expect(recorder.isRecording()).toBe(false);
  });

  inputIt("delivers recorder data callbacks without blocking capture", async () => {
    const recorder = new AudioRecorder();
    let callbackCount = 0;
    let sampleCount = 0;
    recorder.setOnData((_error, data) => {
      callbackCount += 1;
      sampleCount += data.length;
    });

    try {
      recorder.start(inputDeviceId);
      await Bun.sleep(120);
    } finally {
      recorder.stop();
    }

    await Bun.sleep(20);
    expect(callbackCount).toBeGreaterThan(0);
    expect(sampleCount).toBeGreaterThan(0);
  });

  inputIt("opens and closes the native passthrough path", async () => {
    const passthrough = new AudioPassthrough();
    try {
      passthrough.start(inputDeviceId, undefined, 20);
      await Bun.sleep(80);
      expect(passthrough.isRunning()).toBe(true);
    } finally {
      passthrough.stop();
    }
    expect(passthrough.isRunning()).toBe(false);
  });
});

if (!isAudioSystemAvailable() || !inputAvailable) {
  const mode = REQUIRE_AUDIO_HARDWARE ? "required" : "optional";
  console.warn(`Hardware audio tests are ${mode}; unavailable paths are skipped.`);
}

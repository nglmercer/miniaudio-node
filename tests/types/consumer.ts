import type { StreamOutputConfig, SupportedStreamConfig } from "../../index.mjs";

const sampleRate = 48_000;
const channels = 2;

const requested: StreamOutputConfig = {
  sampleRate,
  channels,
  bufferSize: 512,
};

const supported: SupportedStreamConfig = {
  sampleRate,
  channelCount: channels,
  sampleWidth: 16,
};

export function verifyConsumerDeclarations(): [StreamOutputConfig, SupportedStreamConfig] {
  return [requested, supported];
}

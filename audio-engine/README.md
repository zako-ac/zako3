# Zako3 Audio Engine (AE)

Zako3 Audio Engine, also known as AE, is a standalone audio server that does basically:
1. Receive audios from TapHub.
2. Mix them together.
3. Send the mixed audio to a DiscordS channel.

Also note that AE does **NOT** handle actual connection to Taps. It only handles audio mixing and sending to Discord.

## Architecture

### Transport
AE uses gRPC as the transport layer. Protobuf schema used in gRPC can be found in `/protos/audio_engine.proto` file. (It's in the root directory of this repository.)



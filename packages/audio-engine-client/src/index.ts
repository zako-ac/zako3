import { ChannelCredentials, createChannel, Client, createClient, Channel } from 'nice-grpc';
import {
  AudioEngineDefinition,
  JoinRequest,
  LeaveRequest,
  PlayRequest,
  SetVolumeRequest,
  StopRequest,
  StopManyRequest,
  NextMusicRequest,
  GetSessionStateRequest,
  AudioStopFilter,
  SessionState,
  Error as ProtoError,
} from './gen/audio_engine';

export * from './gen/audio_engine';

export class AudioEngineError extends Error {
  constructor(public message: string, public isInternal: boolean) {
    super(message);
    this.name = 'AudioEngineError';
  }
}

export type AudioEngineConfig = {
  address: string;
  credentials?: ChannelCredentials;
};

export class AudioEngine {
  private client: Client<typeof AudioEngineDefinition>;
  private channel: Channel;

  constructor(config: AudioEngineConfig | string) {
    const address = typeof config === 'string' ? config : config.address;
    const credentials =
      typeof config === 'object' && config.credentials
        ? config.credentials
        : ChannelCredentials.createInsecure();

    this.channel = createChannel(address, credentials);
    this.client = createClient(AudioEngineDefinition, this.channel);
  }

  private handleError(error?: ProtoError) {
    if (error) {
      throw new AudioEngineError(error.message, error.isInternal);
    }
  }

  /**
   * Joins a voice channel in a guild.
   * @param guildId The ID of the guild.
   * @param channelId The ID of the voice channel.
   * @throws {AudioEngineError} If the operation fails.
   */
  public async join(guildId: number, channelId: number): Promise<void> {
    const request: JoinRequest = { guildId, channelId };
    const response = await this.client.join(request);
    this.handleError(response.error);
  }

  /**
   * Leaves the voice channel in a guild.
   * @param guildId The ID of the guild.
   * @throws {AudioEngineError} If the operation fails.
   */
  public async leave(guildId: number): Promise<void> {
    const request: LeaveRequest = { guildId };
    const response = await this.client.leave(request);
    this.handleError(response.error);
  }

  /**
   * Requests to play audio.
   * @returns The track ID of the queued audio.
   * @throws {AudioEngineError} If the operation fails.
   */
  public async play(
    guildId: number,
    queueName: string,
    tapName: string,
    audioRequestString: string,
    volume: number = 1.0
  ): Promise<number> {
    const request: PlayRequest = {
      guildId,
      queueName,
      tapName,
      audioRequestString,
      volume,
    };
    const response = await this.client.play(request);
    this.handleError(response.error);
    return response.trackId!;
  }

  /**
   * Sets the volume of a specific track.
   * @param volume Float value (e.g., 1.0 for 100%).
   * @throws {AudioEngineError} If the operation fails.
   */
  public async setVolume(
    guildId: number,
    trackId: number,
    volume: number
  ): Promise<void> {
    const request: SetVolumeRequest = { guildId, trackId, volume };
    const response = await this.client.setVolume(request);
    this.handleError(response.error);
  }

  /**
   * Stops a specific track.
   * @throws {AudioEngineError} If the operation fails.
   */
  public async stop(guildId: number, trackId: string): Promise<void> {
    const request: StopRequest = { guildId, trackId };
    const response = await this.client.stop(request);
    this.handleError(response.error);
  }

  /**
   * Stops multiple tracks based on a filter.
   * @throws {AudioEngineError} If the operation fails.
   */
  public async stopMany(
    guildId: number,
    filter: AudioStopFilter
  ): Promise<void> {
    const request: StopManyRequest = { guildId, filter };
    const response = await this.client.stopMany(request);
    this.handleError(response.error);
  }

  /**
   * Skips to the next music track.
   * @throws {AudioEngineError} If the operation fails.
   */
  public async nextMusic(guildId: number): Promise<void> {
    const request: NextMusicRequest = { guildId };
    const response = await this.client.nextMusic(request);
    this.handleError(response.error);
  }

  /**
   * Retrieves the current session state for a guild.
   * @returns The session state containing channel info and tracks.
   * @throws {AudioEngineError} If the operation fails.
   */
  public async getSessionState(
    guildId: number
  ): Promise<SessionState | undefined> {
    const request: GetSessionStateRequest = { guildId };
    const response = await this.client.getSessionState(request);
    this.handleError(response.error);
    return response.state;
  }

  /**
   * Closes the underlying gRPC channel.
   */
  public close() {
    this.channel.close();
  }
}

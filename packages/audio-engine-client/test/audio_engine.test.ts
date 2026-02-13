import { AudioEngine } from '../src';
import { ChannelCredentials, createChannel, createClient } from 'nice-grpc';
import { AudioEngineDefinition } from '../src/gen/audio_engine';

// Mock nice-grpc functions
jest.mock('nice-grpc', () => {
  const originalModule = jest.requireActual('nice-grpc');
  return {
    ...originalModule,
    createChannel: jest.fn(),
    createClient: jest.fn(),
  };
});

describe('AudioEngine', () => {
  let audioEngine: AudioEngine;
  let mockClient: any;
  let mockChannel: any;

  beforeEach(() => {
    // Reset mocks
    jest.clearAllMocks();

    // Setup basic mock client methods
    mockClient = {
      join: jest.fn(),
      leave: jest.fn(),
      play: jest.fn(),
      setVolume: jest.fn(),
      stop: jest.fn(),
      stopMany: jest.fn(),
      nextMusic: jest.fn(),
      getSessionState: jest.fn(),
    };

    mockChannel = {
      close: jest.fn(),
    };

    (createChannel as jest.Mock).mockReturnValue(mockChannel);
    (createClient as jest.Mock).mockReturnValue(mockClient);

    audioEngine = new AudioEngine('localhost:50051');
  });

  describe('constructor', () => {
    it('should create client with string address', () => {
      expect(createChannel).toHaveBeenCalledWith(
        'localhost:50051',
        expect.anything()
      );
      expect(createClient).toHaveBeenCalledWith(
        AudioEngineDefinition,
        mockChannel
      );
    });

    it('should create client with config object', () => {
      const creds = ChannelCredentials.createInsecure();
      new AudioEngine({
        address: 'localhost:50051',
        credentials: creds,
      });

      expect(createChannel).toHaveBeenCalledWith('localhost:50051', creds);
    });
  });

  describe('join', () => {
    it('should call client.join and handle success', async () => {
      mockClient.join.mockResolvedValue({ success: true });

      await audioEngine.join(123, 456);

      expect(mockClient.join).toHaveBeenCalledWith({
        guildId: 123,
        channelId: 456,
      });
    });

    it('should throw error on failure', async () => {
      mockClient.join.mockResolvedValue({
        error: { message: 'Failed to join', isInternal: false },
      });

      await expect(audioEngine.join(123, 456)).rejects.toThrow('Failed to join');
    });
  });

  describe('play', () => {
    it('should call client.play and return trackId', async () => {
      mockClient.play.mockResolvedValue({ trackId: 999 });

      const trackId = await audioEngine.play(
        1,
        'default',
        'mic',
        'song',
        0.5
      );

      expect(mockClient.play).toHaveBeenCalledWith({
        guildId: 1,
        queueName: 'default',
        tapName: 'mic',
        audioRequestString: 'song',
        volume: 0.5,
      });
      expect(trackId).toBe(999);
    });
  });

  describe('stopMany', () => {
    it('should correctly pass filter', async () => {
      mockClient.stopMany.mockResolvedValue({ success: true });

      await audioEngine.stopMany(1, { all: {} });

      expect(mockClient.stopMany).toHaveBeenCalledWith({
        guildId: 1,
        filter: { all: {} },
      });
    });
  });

  describe('getSessionState', () => {
    it('should return session state', async () => {
      const mockState = {
        guildId: 1,
        channelId: 2,
        tracks: [],
      };
      mockClient.getSessionState.mockResolvedValue({ state: mockState });

      const state = await audioEngine.getSessionState(1);

      expect(state).toEqual(mockState);
    });
  });

  describe('close', () => {
    it('should close the channel', () => {
      audioEngine.close();
      expect(mockChannel.close).toHaveBeenCalled();
    });
  });
});

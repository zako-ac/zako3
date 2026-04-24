import { http, HttpResponse, delay } from 'msw'
import { faker } from '@faker-js/faker'
import { API_BASE } from './base'
import type {
  GuildPlaybackStateDto,
  PlaybackActionDto,
  StopTrackDto,
  SkipDto,
  PauseTrackDto,
  ResumeTrackDto,
  EditQueueDto,
  TrackDto,
} from '@zako-ac/zako3-data'

const mockPlaybackHistory: PlaybackActionDto[] = []

const createMockTrack = (): TrackDto => ({
  trackId: faker.string.uuid(),
  queueName: 'main',
  metadata: [
    { type: 'artist', value: faker.music.artist() },
    { type: 'title', value: faker.music.songName() },
  ],
  tapId: 'spotify_tap_id',
  audioRequestString: 'https://example.com/audio.mp3',
  requestedBy: 'test_user',
  volume: 100,
  paused: false,
})

const mockPlaybackStates: Record<string, GuildPlaybackStateDto> = {
  guild_1: {
    guildId: 'guild_1',
    guildName: 'Development Server',
    guildIconUrl: 'https://cdn.discordapp.com/icons/1234567890/a_1234567890abcdef1234567890abcdef.png',
    channelId: 'channel_1',
    channelName: 'voice-general',
    queues: {
      main: [createMockTrack(), createMockTrack()],
    },
    queueMeta: {
      main: {
        user: {
          id: 'user_1',
          name: 'test_user',
          avatarUrl: 'https://example.com/avatar.png',
        },
      },
    },
  },
  guild_2: {
    guildId: 'guild_2',
    guildName: 'Test Community',
    channelId: 'channel_2',
    channelName: 'music',
    queues: {},
    queueMeta: {},
  },
}

const createAction = (actionType: string, guildId: string, channelId: string): PlaybackActionDto => ({
  id: faker.string.uuid(),
  actionType,
  guildId,
  channelId,
  actorDiscordUserId: 'current-user-id',
  createdAt: new Date().toISOString(),
  undoneAt: null,
  undoneByDiscordUserId: null,
})

export const playbackHandlers = [
  http.get(`${API_BASE}/playback/state`, async () => {
    await delay(100)
    return HttpResponse.json(Object.values(mockPlaybackStates))
  }),

  http.post(`${API_BASE}/playback/stop`, async ({ request }) => {
    await delay(150)
    const body = (await request.json()) as StopTrackDto
    const state = mockPlaybackStates[body.guildId]

    if (!state) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Guild not found' },
        { status: 404 }
      )
    }

    const action = createAction('stop', body.guildId, body.channelId)
    mockPlaybackHistory.push(action)
    return HttpResponse.json(action)
  }),

  http.post(`${API_BASE}/playback/skip`, async ({ request }) => {
    await delay(150)
    const body = (await request.json()) as SkipDto
    const state = mockPlaybackStates[body.guildId]

    if (!state) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Guild not found' },
        { status: 404 }
      )
    }

    const action = createAction('skip', body.guildId, body.channelId)
    mockPlaybackHistory.push(action)
    return HttpResponse.json(action)
  }),

  http.post(`${API_BASE}/playback/pause`, async ({ request }) => {
    await delay(150)
    const body = (await request.json()) as PauseTrackDto
    const state = mockPlaybackStates[body.guildId]

    if (!state) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Guild not found' },
        { status: 404 }
      )
    }

    const action = createAction('pause', body.guildId, body.channelId)
    mockPlaybackHistory.push(action)
    return HttpResponse.json(action)
  }),

  http.post(`${API_BASE}/playback/resume`, async ({ request }) => {
    await delay(150)
    const body = (await request.json()) as ResumeTrackDto
    const state = mockPlaybackStates[body.guildId]

    if (!state) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Guild not found' },
        { status: 404 }
      )
    }

    const action = createAction('resume', body.guildId, body.channelId)
    mockPlaybackHistory.push(action)
    return HttpResponse.json(action)
  }),

  http.patch(`${API_BASE}/playback/queue`, async ({ request }) => {
    await delay(150)
    const body = (await request.json()) as EditQueueDto
    const state = mockPlaybackStates[body.guildId]

    if (!state) {
      return HttpResponse.json(
        { code: 'NOT_FOUND', message: 'Guild not found' },
        { status: 404 }
      )
    }

    const action = createAction('queue_edit', body.guildId, body.channelId)
    mockPlaybackHistory.push(action)
    return HttpResponse.json(action)
  }),

  http.post(`${API_BASE}/playback/undo/:actionId`, async ({ params }) => {
    await delay(150)
    const { actionId } = params

    const action = createAction('undo', 'guild_1', 'channel_1')
    action.id = actionId as string
    mockPlaybackHistory.push(action)
    return HttpResponse.json(action)
  }),

  http.get(`${API_BASE}/playback/history`, async () => {
    await delay(100)
    return HttpResponse.json(mockPlaybackHistory.slice(-20))
  }),
]

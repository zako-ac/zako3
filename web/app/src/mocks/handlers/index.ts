import { authHandlers } from './auth'
import { userHandlers } from './users'
import { tapHandlers } from './taps'
import { notificationHandlers } from './notifications'
import { adminHandlers } from './admin'
import { settingsHandlers } from './settings'
import { guildHandlers } from './guild'
import { playbackHandlers } from './playback'
import { mappersHandlers } from './mappers'
import { statsHandlers } from './stats'

export const handlers = [
  ...authHandlers,
  ...userHandlers,
  ...tapHandlers,
  ...notificationHandlers,
  ...adminHandlers,
  ...settingsHandlers,
  ...guildHandlers,
  ...playbackHandlers,
  ...mappersHandlers,
  ...statsHandlers,
]

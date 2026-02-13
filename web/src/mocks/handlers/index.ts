import { authHandlers } from './auth'
import { userHandlers } from './users'
import { tapHandlers } from './taps'
import { notificationHandlers } from './notifications'
import { adminHandlers } from './admin'

export const handlers = [
  ...authHandlers,
  ...userHandlers,
  ...tapHandlers,
  ...notificationHandlers,
  ...adminHandlers,
]

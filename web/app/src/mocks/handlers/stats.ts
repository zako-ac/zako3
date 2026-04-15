import { http, HttpResponse } from 'msw'
import { API_BASE } from './base'

export const statsHandlers = [
  http.get(`${API_BASE}/stats/sse`, () => {
    // Create a simple SSE stream that emits events
    const encoder = new TextEncoder()

    const stream = new ReadableStream({
      start(controller) {
        let eventCount = 0
        const maxEvents = 5

        const sendEvent = () => {
          if (eventCount >= maxEvents) {
            controller.close()
            return
          }

          const event = `data: {"timestamp": "${new Date().toISOString()}", "type": "stats_update"}\n\n`
          controller.enqueue(encoder.encode(event))

          eventCount++
          // Send next event after 1 second
          setTimeout(sendEvent, 1000)
        }

        sendEvent()
      },
    })

    return new HttpResponse(stream, {
      headers: {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache',
        'Connection': 'keep-alive',
      },
    })
  }),
]

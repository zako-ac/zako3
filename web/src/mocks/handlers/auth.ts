import { http, HttpResponse, delay } from 'msw'
import { mockCurrentUser, mockAdminUser } from '../data/users'
import type { AuthUser, LoginResponse, AuthCallbackResponse } from '@zako-ac/zako3-data'

const API_BASE = '/api'

let currentUser: AuthUser | null = null
let useAdminUser = false

export const setMockUser = (isAdmin: boolean) => {
    useAdminUser = isAdmin
    const user = isAdmin ? mockAdminUser : mockCurrentUser
    currentUser = {
        id: user.id,
        discordId: user.discordId,
        username: user.username,
        avatar: user.avatar,
        email: user.email,
        isAdmin: user.isAdmin,
    }
}

setMockUser(true)

export const authHandlers = [
    http.get(`${API_BASE}/auth/login`, async () => {
        await delay(100)
        const response: LoginResponse = {
            redirectUrl: '/auth/callback?code=mock_auth_code',
        }
        return HttpResponse.json(response)
    }),

    http.get(`${API_BASE}/auth/callback`, async ({ request }) => {
        await delay(300)
        const url = new URL(request.url)
        const code = url.searchParams.get('code')

        if (!code) {
            return HttpResponse.json(
                { code: 'INVALID_CODE', message: 'Missing authorization code' },
                { status: 400 }
            )
        }

        const user = useAdminUser ? mockAdminUser : mockCurrentUser
        currentUser = {
            id: user.id,
            discordId: user.discordId,
            username: user.username,
            avatar: user.avatar,
            email: user.email,
            isAdmin: user.isAdmin,
        }

        const response: AuthCallbackResponse = {
            token: 'mock_jwt_token_' + Date.now(),
            user: currentUser,
        }

        return HttpResponse.json(response)
    }),

    http.post(`${API_BASE}/auth/callback`, async ({ request }) => {
        await delay(300)
        const body = (await request.json()) as { code?: string }
        const code = body?.code

        if (!code) {
            return HttpResponse.json(
                { code: 'INVALID_CODE', message: 'Missing authorization code' },
                { status: 400 }
            )
        }

        const user = useAdminUser ? mockAdminUser : mockCurrentUser
        currentUser = {
            id: user.id,
            discordId: user.discordId,
            username: user.username,
            avatar: user.avatar,
            email: user.email,
            isAdmin: user.isAdmin,
        }

        const response: AuthCallbackResponse = {
            token: 'mock_jwt_token_' + Date.now(),
            user: currentUser,
        }

        return HttpResponse.json(response)
    }),

    http.post(`${API_BASE}/auth/logout`, async () => {
        await delay(100)
        currentUser = null
        return new HttpResponse(null, { status: 204 })
    }),

    http.get(`${API_BASE}/auth/refresh`, async ({ request }) => {
        await delay(100)
        const authHeader = request.headers.get('Authorization')

        if (!authHeader || !authHeader.startsWith('Bearer ')) {
            return HttpResponse.json(
                { code: 'UNAUTHORIZED', message: 'Invalid token' },
                { status: 401 }
            )
        }

        return HttpResponse.json({
            token: 'mock_jwt_token_refreshed_' + Date.now(),
        })
    }),

    http.get(`${API_BASE}/users/me`, async ({ request }) => {
        await delay(100)
        const authHeader = request.headers.get('Authorization')

        if (!authHeader || !authHeader.startsWith('Bearer ')) {
            return HttpResponse.json(
                { code: 'UNAUTHORIZED', message: 'Not authenticated' },
                { status: 401 }
            )
        }

        if (!currentUser) {
            const user = useAdminUser ? mockAdminUser : mockCurrentUser
            currentUser = {
                id: user.id,
                discordId: user.discordId,
                username: user.username,
                avatar: user.avatar,
                email: user.email,
                isAdmin: user.isAdmin,
            }
        }

        return HttpResponse.json(currentUser)
    }),
]

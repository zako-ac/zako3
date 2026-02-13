import React from 'react'
import ReactDOM from 'react-dom/client'
import { App } from './App'
import '@/styles/index.css'
import '@/i18n/config'

const enableMocking = async () => {
    if (import.meta.env.DEV) {
        const { worker } = await import('@/mocks/browser')
        return worker.start({
            onUnhandledRequest: 'bypass',
        })
    }
    return Promise.resolve()
}

enableMocking().then(() => {
    ReactDOM.createRoot(document.getElementById('root')!).render(
        <React.StrictMode>
            <App />
        </React.StrictMode>
    )
})

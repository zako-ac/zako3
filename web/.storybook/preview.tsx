import type { Preview } from '@storybook/react'
import { initialize, mswLoader } from 'msw-storybook-addon'
import '../src/styles/index.css'

initialize()

const preview: Preview = {
    parameters: {
        controls: {
            matchers: {
                color: /(background|color)$/i,
                date: /Date$/i,
            },
        },
        backgrounds: {
            default: 'dark',
            values: [
                { name: 'dark', value: 'oklch(0.12 0.01 270)' },
                { name: 'light', value: 'oklch(0.98 0 0)' },
            ],
        },
    },
    loaders: [mswLoader],
    decorators: [
        (Story, context) => {
            const isDark = context.globals.theme !== 'light'
            return (
                <div className={isDark ? 'dark' : ''}>
                    <div className="bg-background text-foreground min-h-screen p-4">
                        <Story />
                    </div>
                </div>
            )
        },
    ],
    globalTypes: {
        theme: {
            name: 'Theme',
            description: 'Global theme for components',
            defaultValue: 'dark',
            toolbar: {
                icon: 'circlehollow',
                items: ['light', 'dark'],
                showName: true,
            },
        },
    },
}

export default preview

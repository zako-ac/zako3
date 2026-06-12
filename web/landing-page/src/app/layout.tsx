import type { Metadata } from "next"
import { Geist, Geist_Mono } from "next/font/google"
import { ThemeProvider } from "next-themes"
import "./globals.css"

const geistSans = Geist({
    variable: "--font-geist-sans",
    subsets: ["latin"],
})

const geistMono = Geist_Mono({
    variable: "--font-geist-mono",
    subsets: ["latin"],
})

export const metadata: Metadata = {
    title: "ZAKO",
    description: "자코는 여러 목소리를 가진 디스코드 TTS 및 음악 봇입니다. 유저들이 TTS와 음악 소스를 직접 개발할 수 있습니다.",
    alternates: {
        canonical: "https://zako.ac",
    },
    openGraph: {
        title: "ZAKO",
        description: "자코는 여러 목소리를 가진 디스코드 TTS 및 음악 봇입니다. 유저들이 TTS와 음악 소스를 직접 개발할 수 있습니다.",
        url: "https://zako.ac",
        siteName: "ZAKO",
        images: {
            url: "https://zako.ac/assets/zakopsa.png",
        }
    },
    themeColor: "#eb3489",
}

export default function RootLayout({
    children,
}: Readonly<{
    children: React.ReactNode
}>) {
    return (
        <html lang="ko" className={`${geistSans.variable} ${geistMono.variable} h-full antialiased`} suppressHydrationWarning>
            <body className="min-h-full flex flex-col">
                <ThemeProvider attribute="class" defaultTheme="dark" enableSystem>
                    {children}
                </ThemeProvider>
            </body>
        </html>
    )
}

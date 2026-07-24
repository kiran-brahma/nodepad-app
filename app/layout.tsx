import type { Metadata } from 'next'
import { Geist, Geist_Mono, Vazirmatn } from 'next/font/google'
import { ThemeProvider } from 'next-themes'
import { MobileWall } from '@/components/mobile-wall'
import './globals.css'

const _geist = Geist({ subsets: ["latin"] });
const _geistMono = Geist_Mono({ subsets: ["latin"] });
const vazirmatn = Vazirmatn({
  subsets: ["arabic"],
  variable: "--font-vazirmatn",
  display: "swap",
});

export const metadata: Metadata = {
  title: 'nodepad',
  description: 'A spatial research tool where AI augments your thinking — not replaces it.',
  icons: {
    icon: [{ url: '/icon.svg', type: 'image/svg+xml' }],
    apple: '/apple-icon.png',
  },
  openGraph: {
    title: 'nodepad',
    description: 'A spatial research tool where AI augments your thinking — not replaces it.',
    url: 'https://nodepad.space',
    siteName: 'nodepad',
    locale: 'en_US',
    type: 'website',
  },
  twitter: {
    card: 'summary_large_image',
    title: 'nodepad',
    description: 'A spatial research tool where AI augments your thinking — not replaces it.',
  },
}

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode
}>) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className={`font-sans antialiased ${vazirmatn.variable}`} suppressHydrationWarning>
        <ThemeProvider attribute="class" defaultTheme="dark" enableSystem={false}>
          <MobileWall />
          {children}
        </ThemeProvider>
      </body>
    </html>
  )
}

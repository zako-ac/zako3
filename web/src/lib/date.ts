import { format, formatDistanceToNow, Locale, parseISO } from 'date-fns'
import { enUS, ko } from 'date-fns/locale'

const locales: Record<string, Locale> = {
    en: enUS,
    ko: ko,
}

const getLocale = (lang: string = 'en'): Locale => locales[lang] || enUS

export const formatDate = (
    date: string | Date,
    formatStr: string = 'PPP',
    lang: string = 'en'
): string => {
    const d = typeof date === 'string' ? parseISO(date) : date
    return format(d, formatStr, { locale: getLocale(lang) })
}

export const formatDateTime = (
    date: string | Date,
    lang: string = 'en'
): string => {
    return formatDate(date, 'PPp', lang)
}

export const formatRelativeTime = (
    date: string | Date,
    lang: string = 'en'
): string => {
    const d = typeof date === 'string' ? parseISO(date) : date
    return formatDistanceToNow(d, { addSuffix: true, locale: getLocale(lang) })
}

export const formatShortDate = (
    date: string | Date,
    lang: string = 'en'
): string => {
    return formatDate(date, 'MMM d, yyyy', lang)
}

export const formatChartDate = (date: string | Date): string => {
    return formatDate(date, 'MMM d')
}

export const formatChartTime = (date: string | Date): string => {
    return formatDate(date, 'HH:mm')
}

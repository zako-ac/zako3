import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Cpu, Trash2, Pencil, ChevronDown, ChevronUp, FlaskConical } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader } from '@/components/ui/card'
import type { WasmMapperDto, MapperInputDataKey } from '../api'
import { MapperEditForm } from './MapperEditForm'
import { MapperTestPanel } from './MapperTestPanel'

interface MapperCardProps {
    mapper: WasmMapperDto
    onDelete: (id: string) => void
    isDeleting?: boolean
}

export const MapperCard = ({ mapper, onDelete, isDeleting }: MapperCardProps) => {
    const { t } = useTranslation()
    const [editing, setEditing] = useState(false)
    const [testing, setTesting] = useState(false)
    const [showHash, setShowHash] = useState(false)

    // Explicit translations for extraction
    const inputDataLabels: Record<MapperInputDataKey, string> = {
        mapping_list: t('admin.mappers.inputDataBadges.mappingList'),
        discord_info: t('admin.mappers.inputDataBadges.discordInfo'),
        caller_info: t('admin.mappers.inputDataBadges.callerInfo'),
        mapper_list: t('admin.mappers.inputDataBadges.mapperList'),
    }

    if (editing) {
        return (
            <MapperEditForm
                mapper={mapper}
                onCancel={() => setEditing(false)}
                onSuccess={() => setEditing(false)}
            />
        )
    }

    return (
        <Card>
            <CardHeader className="flex flex-row items-start justify-between pb-2">
                <div className="flex items-center gap-2">
                    <Cpu className="text-muted-foreground h-4 w-4" />
                    <div>
                        <div className="font-medium">{mapper.name}</div>
                        <div className="text-muted-foreground font-mono text-xs">{mapper.id}</div>
                    </div>
                </div>
                <div className="flex gap-1">
                    <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => setTesting((v) => !v)}
                        className="h-8 w-8"
                        title={t('admin.mappers.testMapper')}
                    >
                        <FlaskConical className="h-3.5 w-3.5" />
                    </Button>
                    <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => setEditing(true)}
                        className="h-8 w-8"
                    >
                        <Pencil className="h-3.5 w-3.5" />
                    </Button>
                    <Button
                        variant="ghost"
                        size="icon"
                        onClick={() => onDelete(mapper.id)}
                        disabled={isDeleting}
                        className="text-destructive hover:text-destructive h-8 w-8"
                    >
                        <Trash2 className="h-3.5 w-3.5" />
                    </Button>
                </div>
            </CardHeader>
            <CardContent className="space-y-2">
                <div className="flex flex-wrap gap-1">
                    {mapper.input_data.length === 0 ? (
                        <span className="text-muted-foreground text-xs">{t('admin.mappers.noInputData')}</span>
                    ) : (
                        mapper.input_data.map((key) => (
                            <Badge key={key} variant="secondary" className="text-xs">
                                {inputDataLabels[key] ?? key}
                            </Badge>
                        ))
                    )}
                </div>
                <div className="text-muted-foreground text-xs">
                    <span className="font-medium">{t('admin.mappers.fileLabel')}:</span> {mapper.wasm_filename}
                </div>
                <button
                    onClick={() => setShowHash((v) => !v)}
                    className="text-muted-foreground flex items-center gap-1 text-xs hover:underline"
                >
                    SHA-256
                    {showHash ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
                </button>
                {showHash && (
                    <div className="bg-muted rounded p-1 font-mono text-xs break-all">
                        {mapper.sha256_hash}
                    </div>
                )}
            </CardContent>
            {testing && <MapperTestPanel mapper={mapper} />}
        </Card>
    )
}

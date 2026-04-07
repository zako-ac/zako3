import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Label } from '@/components/ui/label'
import { Input } from '@/components/ui/input'
import { Checkbox } from '@/components/ui/checkbox'
import { UploadCloud } from 'lucide-react'
import type { MapperInputDataKey } from '../api'

const INPUT_DATA_OPTIONS: { key: MapperInputDataKey }[] = [
    { key: 'mapping_list' },
    { key: 'discord_info' },
    { key: 'caller_info' },
    { key: 'mapper_list' },
]

interface MapperFormFieldsProps {
    id?: string
    setId?: (v: string) => void
    idReadonly?: boolean
    name: string
    setName: (v: string) => void
    inputData: MapperInputDataKey[]
    setInputData: (v: MapperInputDataKey[]) => void
    file: File | null
    setFile: (f: File | null) => void
    fileRequired?: boolean
}

export const MapperFormFields = ({
    id,
    setId,
    idReadonly,
    name,
    setName,
    inputData,
    setInputData,
    file,
    setFile,
    fileRequired = false,
}: MapperFormFieldsProps) => {
    const { t } = useTranslation()
    const inputRef = useRef<HTMLInputElement>(null)
    const [isDragging, setIsDragging] = useState(false)

    // Explicit translations for extraction
    const inputDataLabels = {
        mapping_list: t('admin.mappers.inputDataOptions.mappingList'),
        discord_info: t('admin.mappers.inputDataOptions.discordInfo'),
        caller_info: t('admin.mappers.inputDataOptions.callerInfo'),
        mapper_list: t('admin.mappers.inputDataOptions.mapperList'),
    }

    const inputDataDescriptions = {
        mapping_list: t('admin.mappers.inputDataOptions.mappingListDescription'),
        discord_info: t('admin.mappers.inputDataOptions.discordInfoDescription'),
        caller_info: t('admin.mappers.inputDataOptions.callerInfoDescription'),
        mapper_list: t('admin.mappers.inputDataOptions.mapperListDescription'),
    }

    const toggleInput = (key: MapperInputDataKey) => {
        setInputData(
            inputData.includes(key) ? inputData.filter((k) => k !== key) : [...inputData, key]
        )
    }

    const handleDragOver = (e: React.DragEvent<HTMLDivElement>) => {
        e.preventDefault()
        setIsDragging(true)
    }

    const handleDragLeave = () => {
        setIsDragging(false)
    }

    const handleDrop = (e: React.DragEvent<HTMLDivElement>) => {
        e.preventDefault()
        setIsDragging(false)
        const files = e.dataTransfer.files
        if (files.length > 0) {
            setFile(files[0])
        }
    }

    const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        if (e.target.files?.[0]) {
            setFile(e.target.files[0])
        }
    }

    return (
        <div className="space-y-4">
            {setId !== undefined && (
                <div className="space-y-1">
                    <Label htmlFor="mapper-id">{t('admin.mappers.id')}</Label>
                    <Input
                        id="mapper-id"
                        value={id ?? ''}
                        onChange={(e) => setId(e.target.value)}
                        readOnly={idReadonly}
                        placeholder={t('admin.mappers.idPlaceholder')}
                        className={idReadonly ? 'bg-muted' : ''}
                    />
                    {!idReadonly && (
                        <p className="text-muted-foreground text-xs">
                            {t('admin.mappers.idDescription', { id: id || 'id' })}
                        </p>
                    )}
                </div>
            )}

            <div className="space-y-1">
                <Label htmlFor="mapper-name">{t('admin.mappers.name')}</Label>
                <Input
                    id="mapper-name"
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    placeholder={t('admin.mappers.namePlaceholder')}
                />
            </div>

            <div className="space-y-2">
                <Label>{t('admin.mappers.inputData')}</Label>
                <div className="space-y-2">
                    {INPUT_DATA_OPTIONS.map(({ key }) => (
                        <label key={key} className="flex cursor-pointer items-start gap-2">
                            <Checkbox
                                checked={inputData.includes(key)}
                                onCheckedChange={() => toggleInput(key)}
                                className="mt-0.5"
                            />
                            <div>
                                <div className="text-sm font-medium">{inputDataLabels[key]}</div>
                                <div className="text-muted-foreground text-xs">{inputDataDescriptions[key]}</div>
                            </div>
                        </label>
                    ))}
                </div>
            </div>

            <div className="space-y-1">
                <Label>
                    {t('admin.mappers.wasmFile')} {fileRequired ? <span className="text-destructive">*</span> : t('admin.mappers.wasmFileOptional')}
                </Label>
                <input
                    ref={inputRef}
                    type="file"
                    accept=".wasm"
                    onChange={handleInputChange}
                    className="hidden"
                />
                <div
                    onClick={() => inputRef.current?.click()}
                    onDragOver={handleDragOver}
                    onDragLeave={handleDragLeave}
                    onDrop={handleDrop}
                    className={`border-2 border-dashed rounded-lg p-8 text-center cursor-pointer transition-colors ${
                        isDragging
                            ? 'border-primary/60 bg-primary/15'
                            : 'border-primary/30 bg-primary/5 hover:border-primary/50 hover:bg-primary/10'
                    }`}
                >
                    {file ? (
                        <div className="space-y-1">
                            <p className="text-sm font-medium">{file.name}</p>
                            <p className="text-xs text-muted-foreground">
                                {t('admin.mappers.dropzone.selected', { name: file.name, size: Math.round(file.size / 1024) })}
                            </p>
                        </div>
                    ) : (
                        <div className="space-y-2">
                            <UploadCloud className="mx-auto h-8 w-8 text-muted-foreground" />
                            <div className="space-y-1">
                                <p className="text-sm font-medium">{t('admin.mappers.dropzone.label')}</p>
                                <p className="text-xs text-muted-foreground">{t('admin.mappers.dropzone.sublabel')}</p>
                            </div>
                            <p className="text-xs text-muted-foreground">{t('admin.mappers.dropzone.hint')}</p>
                        </div>
                    )}
                </div>
            </div>
        </div>
    )
}

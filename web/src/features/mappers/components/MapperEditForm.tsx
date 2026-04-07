import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from '@/components/ui/card'
import type { WasmMapperDto, MapperInputDataKey } from '../api'
import { useUpdateMapper } from '../hooks'
import { MapperFormFields } from './MapperFormFields'

interface MapperEditFormProps {
    mapper: WasmMapperDto
    onSuccess: () => void
    onCancel: () => void
}

export const MapperEditForm = ({ mapper, onSuccess, onCancel }: MapperEditFormProps) => {
    const { t } = useTranslation()
    const [name, setName] = useState(mapper.name)
    const [inputData, setInputData] = useState<MapperInputDataKey[]>(mapper.input_data)
    const [file, setFile] = useState<File | null>(null)

    const { mutateAsync: updateMapper, isPending } = useUpdateMapper(mapper.id)

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault()
        if (!name.trim()) return

        const formData = new FormData()
        formData.append('name', name.trim())
        formData.append('input_data', JSON.stringify(inputData))
        if (file) formData.append('file', file)

        try {
            await updateMapper(formData)
            toast.success(t('admin.mappers.updateSuccess'))
            onSuccess()
        } catch {
            toast.error(t('admin.mappers.updateError'))
        }
    }

    return (
        <Card>
            <form onSubmit={handleSubmit}>
                <CardHeader>
                    <CardTitle className="text-base">{t('admin.mappers.editFormTitle', { id: mapper.id })}</CardTitle>
                </CardHeader>
                <CardContent>
                    <MapperFormFields
                        id={mapper.id}
                        idReadonly
                        name={name}
                        setName={setName}
                        inputData={inputData}
                        setInputData={setInputData}
                        file={file}
                        setFile={setFile}
                    />
                </CardContent>
                <CardFooter className="flex justify-end gap-2 mt-4">
                    <Button type="button" variant="ghost" onClick={onCancel} disabled={isPending}>
                        {t('admin.mappers.cancel')}
                    </Button>
                    <Button type="submit" disabled={isPending || !name}>
                        {isPending ? t('admin.mappers.saving') : t('admin.mappers.save')}
                    </Button>
                </CardFooter>
            </form>
        </Card>
    )
}

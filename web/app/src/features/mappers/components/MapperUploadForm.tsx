import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from '@/components/ui/card'
import type { MapperInputDataKey } from '../api'
import { useCreateMapper } from '../hooks'
import { MapperFormFields } from './MapperFormFields'

interface MapperUploadFormProps {
    onSuccess: () => void
    onCancel: () => void
}

export const MapperUploadForm = ({ onSuccess, onCancel }: MapperUploadFormProps) => {
    const { t } = useTranslation()
    const [id, setId] = useState('')
    const [name, setName] = useState('')
    const [inputData, setInputData] = useState<MapperInputDataKey[]>([])
    const [file, setFile] = useState<File | null>(null)

    const { mutateAsync: createMapper, isPending } = useCreateMapper()

    const handleSubmit = async (e: React.FormEvent) => {
        e.preventDefault()
        if (!id.trim() || !name.trim() || !file) return

        const formData = new FormData()
        formData.append('id', id.trim())
        formData.append('name', name.trim())
        formData.append('input_data', JSON.stringify(inputData))
        formData.append('file', file)

        try {
            await createMapper(formData)
            toast.success(t('admin.mappers.createSuccess'))
            onSuccess()
        } catch (e) {
            toast.error(e instanceof Error ? e.message : t('admin.mappers.createError'))
        }
    }

    return (
        <Card>
            <form onSubmit={handleSubmit}>
                <CardHeader>
                    <CardTitle className="text-base">{t('admin.mappers.uploadFormTitle')}</CardTitle>
                </CardHeader>
                <CardContent>
                    <MapperFormFields
                        id={id}
                        setId={setId}
                        name={name}
                        setName={setName}
                        inputData={inputData}
                        setInputData={setInputData}
                        file={file}
                        setFile={setFile}
                        fileRequired
                    />
                </CardContent>
                <CardFooter className="flex justify-end gap-2 mt-4">
                    <Button type="button" variant="ghost" onClick={onCancel} disabled={isPending}>
                        {t('admin.mappers.cancel')}
                    </Button>
                    <Button type="submit" disabled={isPending || !id || !name || !file}>
                        {isPending ? t('admin.mappers.uploading') : t('admin.mappers.upload')}
                    </Button>
                </CardFooter>
            </form>
        </Card>
    )
}

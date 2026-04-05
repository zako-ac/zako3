import {
    Select,
    SelectContent,
    SelectItem,
    SelectTrigger,
    SelectValue,
} from '@/components/ui/select'
import { TAP_OCCUPATIONS } from '@zako-ac/zako3-data'
import type { TapOccupation } from '@zako-ac/zako3-data'
import { OccupationBadge } from './occupation-badge'

interface OccupationSelectProps {
    value: TapOccupation
    onChange: (value: TapOccupation) => void
    disabled?: boolean
}

export const OccupationSelect = ({
    value,
    onChange,
    disabled,
}: OccupationSelectProps) => {
    return (
        <Select
            value={value}
            onValueChange={(v) => onChange(v as TapOccupation)}
            disabled={disabled}
        >
            <SelectTrigger className="w-[180px]">
                <SelectValue>
                    <div className="flex items-center gap-2">
                        <OccupationBadge occupation={value} />
                    </div>
                </SelectValue>
            </SelectTrigger>
            <SelectContent>
                {TAP_OCCUPATIONS.map((occ) => (
                    <SelectItem key={occ} value={occ}>
                        <div className="flex items-center gap-2">
                            <OccupationBadge occupation={occ} />
                        </div>
                    </SelectItem>
                ))}
            </SelectContent>
        </Select>
    )
}

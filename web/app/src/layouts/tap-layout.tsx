import { Outlet } from 'react-router-dom'

export const TapLayout = () => {
    return (
        <div className="mx-auto max-w-4xl">
            <Outlet />
        </div>
    )
}

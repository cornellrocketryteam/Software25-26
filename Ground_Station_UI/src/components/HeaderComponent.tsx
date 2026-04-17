import { useNavigate } from "react-router-dom";
import { usePropulsion } from "../PropulsionPage";

export default function Header({ pageTitle }: { pageTitle: string }) {
    const navigate = useNavigate();
    const {currFlightMode} = usePropulsion();

    const isPropulsionPage = pageTitle === "Propulsion Page";
    const isRecoveryPage = pageTitle === "Recovery and Payload";

    return (
        <header className="bg-[#F9F9EB] px-8 py-4">
            <div className="relative flex items-center justify-between">
                {/* LEFT: Logo + Page Name */}
                <div className="flex items-center gap-4">
                    <button onClick={() => navigate("/")} className="focus:outline-none" aria-label="Go to home">
                        <img src="/src/assets/CRT_LOGO.png" alt="CRT Logo" className="h-16"
                        />
                    </button>

                    <h1 className="text-4xl font-inter">
                        {pageTitle}
                    </h1>
                </div>

                {/* CENTER: Rocket State */}
                <div className="absolute left-1/2 -translate-x-1/2">
                    <div className="rounded-xl bg-white px-6 py-2 shadow text-lg font-semibold">
                        Curr Rocket State: {currFlightMode}
                    </div>
                </div>

                {/* RIGHT: Navigation Button */}
                <div className="flex items-center gap-4">
                    {isPropulsionPage && (
                        <button onClick={() => navigate("/recovery")} className="bg-white border-[6px] border-black rounded-3xl px-6 py-2 text-lg font-inter hover:bg-gray-50 transition-colors">
                            Recovery & Payload
                        </button>
                    )}
                    {isRecoveryPage && (
                        <button onClick={() => navigate("/propulsion")} className="bg-white border-[6px] border-black rounded-3xl px-6 py-2 text-lg font-inter hover:bg-gray-50 transition-colors">
                            Propulsion Page
                        </button>
                    )}
                </div>
            </div>
        </header>
    );
}
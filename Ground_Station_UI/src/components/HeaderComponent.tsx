import { useNavigate } from "react-router-dom";
import type { interactionType } from "../PropulsionPage";

interface HeaderProps {
    pageTitle: string;          // required, always needed
    currFlightMode?: string;    // optional, only passed from PropulsionPage
    buttonInteractionState?: string;  // optional, only passed from PropulsionPage
    canInteractRef?: React.RefObject<string>;  // optional, only passed from PropulsionPage
    fillUIActive?: boolean;     // optional, only passed from PropulsionPage
    setButtonInteractionState?: (allowInteraction: interactionType) => void // optional, only passed from PropulsionPage
}



export default function Header({ 
        pageTitle,
        currFlightMode,
        buttonInteractionState,
        canInteractRef,
        fillUIActive,
        setButtonInteractionState
    }: HeaderProps) {
    const navigate = useNavigate();


    const isPropulsionPage = pageTitle === "Propulsion Page";
    const isRecoveryPage = pageTitle === "Recovery & Payload Page";

    return (
        <header className="bg-[#F9F9EB] px-8 py-4">
            <div className="flex items-center justify-between">
                {/* LEFT: Logo + Page Name */}
                <div className="flex items-center gap-4 flex-1">
                    <button onClick={() => navigate("/")} className="focus:outline-none" aria-label="Go to home">
                        <img src="/src/assets/CRT_LOGO.png" alt="CRT Logo" className="h-16"/>
                    </button>
                    <h1 className="text-4xl font-inter">{pageTitle}</h1>
                </div>

                {/* CENTER: Rocket State + Display Toggle */}
                <div className="flex flex-row items-center gap-4 justify-center">
                    <div className="rounded-xl bg-white border-[3px] border-black px-6 py-2 shadow text-lg font-semibold">
                        Curr Rocket State: {currFlightMode}
                    </div>
                    <button 
                        disabled = {fillUIActive} // Disable interaction toggle when fill UI is active
                        onClick={() => {
                            if(canInteractRef?.current === "DISABLED"){
                                setButtonInteractionState?.("ENABLED");
                                canInteractRef.current = "ENABLED";
                            } else if(canInteractRef?.current === "ENABLED"){
                                setButtonInteractionState?.("DISABLED");
                                canInteractRef.current = "DISABLED";
                            }
                            
                        }}
                        className="bg-white border-[3px] border-black rounded-3xl px-6 py-2 text-lg font-inter hover:bg-gray-50 transition-colors text-lg font-semibold disabled:opacity-30 disabled:cursor-not-allowed"
                    >
                        Button Interaction State: {buttonInteractionState}
                    </button>
                </div>

                {/* RIGHT: Navigation Button */}
                <div className="flex items-center gap-4 flex-1 justify-end">
                    {isPropulsionPage && (
                        <button onClick={
                            () => {
                                navigate("/recovery")

                            }
                        } className="bg-white border-[4px] border-black rounded-3xl px-6 py-2 text-lg font-inter hover:bg-gray-50 transition-colors">
                            Recovery & Payload
                        </button>
                    )}
                    {isRecoveryPage && (
                        <button onClick={
                            () => {
                                navigate("/propulsion")
                                
                                }
                            } className="bg-white border-[4px] border-black rounded-3xl px-6 py-2 text-lg font-inter hover:bg-gray-50 transition-colors">
                            Propulsion Page
                        </button>
                    )}
                </div>
            </div>
        </header>
    );
}